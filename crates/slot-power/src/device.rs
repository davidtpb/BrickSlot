use std::ffi::{c_int, c_ulong, c_void};
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Battery, Charge, CpuProfile, LedState, Lights, LightsEffect, LightsMode, Platform};

const TOP_STEP: u32 = 9;

const DEV_INPUT: &str = "/dev/input";

const RUN_DIR: &str = "/run";

pub fn uptime_seconds(s: &str) -> Option<String> {
    let first = s.split_whitespace().next()?;
    if !first.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if !first.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(first.to_string())
}

pub fn record_first_frame(run: &Path, uptime: &str) {
    let marker = run.join("boot-first-frame");
    let again = marker.exists();
    if !again {
        let _ = fs::write(&marker, format!("{uptime}\n"));
    }
    let label = if again {
        "first-frame-again"
    } else {
        "first-frame"
    };
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(run.join("boot-trace"))
    {
        let _ = writeln!(f, "{uptime} {label}");
    }
}

pub fn trace_first_frame() {
    let Ok(raw) = fs::read_to_string("/proc/uptime") else {
        return;
    };
    let Some(up) = uptime_seconds(&raw) else {
        return;
    };
    record_first_frame(Path::new(RUN_DIR), &up);
}

const EV_FF: u16 = 0x15;
const FF_RUMBLE: u16 = 0x50;
const EVIOCSFF: c_ulong = 0x4030_4580;

const DISPDBG_MAX: u32 = 255;

const DISP_MAX: u32 = 255;
const DISP_WAKE: u32 = 8;
const DISP_LCD_SET_BRIGHTNESS: c_ulong = 0x102;

const TRIMUI_MOTOR_GPIO: &str = "class/gpio/gpio227/value";
const TRIMUI_MOTOR_VOLTAGE: &str = "class/motor/voltage";
const TRIMUI_LED_ANIM: &str = "class/led_anim";

pub const BOARD_VAR: &str = "SLOT_DEVICE";

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Board {
    #[default]
    Anbernic,
    TrimUiBrick,
    TrimUiBrickPro,
    TrimUiSmartPro,
}

impl Board {
    pub fn detect() -> Board {
        let device = std::env::var(BOARD_VAR).ok();
        let cpuinfo = fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
        Board::from_hints(device.as_deref(), &cpuinfo)
    }

    pub fn from_hints(device: Option<&str>, cpuinfo: &str) -> Board {
        let named = device.map(|d| d.trim().to_ascii_lowercase());
        match named.as_deref() {
            Some("brickpro" | "brick-pro" | "tg4040") => return Board::TrimUiBrickPro,
            Some("brick" | "tg3040") => return Board::TrimUiBrick,
            Some("smartpro" | "smart-pro" | "tg5040") => return Board::TrimUiSmartPro,
            Some("anbernic" | "h700") => return Board::Anbernic,
            _ => {}
        }
        if cpuinfo.contains("TG4040") {
            Board::TrimUiBrickPro
        } else if cpuinfo.contains("TG3040") {
            Board::TrimUiBrick
        } else if cpuinfo.contains("TG5040") {
            Board::TrimUiSmartPro
        } else {
            Board::Anbernic
        }
    }

    pub fn is_trimui(self) -> bool {
        self != Board::Anbernic
    }

    fn motor_volts(self) -> u32 {
        match self {
            Board::TrimUiBrickPro => 2_500_000,
            _ => 3_300_000,
        }
    }
}

enum Backlight {
    Class(PathBuf),
    Dispdbg(PathBuf),
    Disp(PathBuf),
}

/// A light that is only on or off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// A `class/leds` LED: its directory and full brightness.
    Leds { dir: PathBuf, max: u32 },
    /// A file taking 1 for on and 0 for off.
    File(PathBuf),
}

impl Switch {
    fn set(&self, on: bool) {
        let (path, value) = match self {
            Switch::Leds { dir, max } => (dir.join("brightness"), if on { *max } else { 0 }),
            Switch::File(path) => (path.clone(), u32::from(on)),
        };
        let _ = fs::write(path, format!("{value}\n"));
    }
}

/// LEDs a kernel lists that are not a power light: the card reader's
/// activity, keyboard locks, radios.
fn not_a_power_light(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "mmc",
        "capslock",
        "numlock",
        "scrolllock",
        "kana",
        "compose",
        "rfkill",
        "phy",
        "usb",
        "disk",
        "hci",
        "wlan",
        "bluetooth",
        "mute",
    ]
    .iter()
    .any(|n| name.contains(n))
}

/// The H700 boards' lights: the first `class/leds` LED that may be the
/// power light, every other such LED, and any PMIC `work_led` switch.
pub fn h700_lights(sysfs: &Path) -> (Option<PathBuf>, Vec<Switch>) {
    let mut dirs: Vec<PathBuf> = fs::read_dir(sysfs.join("class/leds"))
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    dirs.sort();
    let mut lights = dirs.into_iter().filter(|d| {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned());
        name.is_some_and(|n| !not_a_power_light(&n))
            && (d.join("multi_intensity").is_file() || d.join("brightness").is_file())
    });
    let first = lights.next();
    let mut extra: Vec<Switch> = lights
        .map(|dir| {
            let max = read_number(&dir.join("max_brightness")).unwrap_or(255);
            Switch::Leds { dir, max }
        })
        .collect();
    if let Ok(supplies) = fs::read_dir(sysfs.join("class/power_supply")) {
        let mut files: Vec<PathBuf> = supplies
            .filter_map(|e| e.ok())
            .map(|e| e.path().join("work_led"))
            .filter(|f| f.is_file())
            .collect();
        files.sort();
        extra.extend(files.into_iter().map(Switch::File));
    }
    (first, extra)
}

/// Writes what lights there are to `slot-led.log` on the card, so a light
/// that will not go out can be tracked down.
fn log_lights(root: &Path, sysfs: &Path, first: &Option<PathBuf>, extra: &[Switch]) {
    use std::io::Write;
    let mut text = String::from("class/leds:\n");
    if let Ok(d) = fs::read_dir(sysfs.join("class/leds")) {
        let mut names: Vec<String> = d
            .filter_map(|e| e.ok())
            .map(|e| {
                let p = e.path();
                let b = fs::read_to_string(p.join("brightness")).unwrap_or_default();
                let m = fs::read_to_string(p.join("max_brightness")).unwrap_or_default();
                let t = fs::read_to_string(p.join("trigger")).unwrap_or_default();
                format!(
                    "  {} brightness={} max={} trigger={}",
                    e.file_name().to_string_lossy(),
                    b.trim(),
                    m.trim(),
                    t.trim()
                )
            })
            .collect();
        names.sort();
        for n in names {
            text.push_str(&n);
            text.push('\n');
        }
    }
    text.push_str(&format!("chosen: {first:?}\nalso: {extra:?}\n"));
    if let Ok(mut f) = fs::File::create(root.join("slot-led.log")) {
        let _ = f.write_all(text.as_bytes());
    }
}

enum Led {
    Multi(PathBuf),
    Mono { dir: PathBuf, max: u32 },
    Anim(PathBuf),
}

pub struct DevicePlatform {
    root: PathBuf,
    sysfs: PathBuf,
    backlight: Option<Backlight>,
    max_brightness: u32,
    battery: Option<PathBuf>,
    rtc: Option<PathBuf>,
    motor: Option<Motor>,
    led: Option<Led>,
    /// More lights switched on and off with `led`: the H700 boards' other
    /// LEDs and the PMIC's own LED switch, where there are such.
    led_extra: Vec<Switch>,
    board: Board,
    lights: Lights,
    led_state: Option<LedState>,
    disp_dark: bool,
    rainbow: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    cpu_stock: Option<CpuSettings>,
}

impl DevicePlatform {
    pub fn new(root: PathBuf) -> Self {
        DevicePlatform::probe_board(Path::new("/sys"), Path::new("/dev"), root, Board::detect())
    }

    pub fn probe(sysfs: &Path, root: PathBuf) -> Self {
        DevicePlatform::probe_board(sysfs, Path::new("/dev"), root, Board::Anbernic)
    }

    pub fn probe_board(sysfs: &Path, dev: &Path, root: PathBuf, board: Board) -> Self {
        if board.is_trimui() {
            return DevicePlatform::probe_trimui(sysfs, dev, root, board);
        }
        let class = first_dir(&sysfs.join("class/backlight"), |d| {
            d.join("brightness").is_file()
        });
        let max_brightness = class
            .as_ref()
            .and_then(|d| read_number(&d.join("max_brightness")))
            .unwrap_or(DISPDBG_MAX);
        let backlight = match class {
            Some(dir) => Some(Backlight::Class(dir)),
            None => {
                let dbg = sysfs.join("kernel/debug/dispdbg");
                dbg.join("param")
                    .is_file()
                    .then_some(Backlight::Dispdbg(dbg))
            }
        };
        let battery = first_dir(&sysfs.join("class/power_supply"), is_battery);
        let rtc = first_dir(&sysfs.join("class/rtc"), |_| true);
        let motor = Motor::open(sysfs);
        let (first, led_extra) = h700_lights(sysfs);
        log_lights(&root, sysfs, &first, &led_extra);
        let led = first.map(|dir| {
            if dir.join("multi_intensity").is_file() {
                Led::Multi(dir)
            } else {
                let max = read_number(&dir.join("max_brightness")).unwrap_or(255);
                Led::Mono { dir, max }
            }
        });
        DevicePlatform {
            root,
            sysfs: sysfs.to_path_buf(),
            backlight,
            max_brightness,
            battery,
            rtc,
            motor,
            led,
            led_extra,
            board,
            lights: Lights::default(),
            led_state: None,
            disp_dark: false,
            rainbow: None,
            cpu_stock: None,
        }
    }

    fn probe_trimui(sysfs: &Path, dev: &Path, root: PathBuf, board: Board) -> Self {
        let disp = dev.join("disp");
        let backlight = if disp.exists() {
            Some(Backlight::Disp(disp))
        } else {
            first_dir(&sysfs.join("class/backlight"), |d| {
                d.join("brightness").is_file()
            })
            .map(Backlight::Class)
        };
        let max_brightness = match &backlight {
            Some(Backlight::Class(dir)) => {
                read_number(&dir.join("max_brightness")).unwrap_or(DISP_MAX)
            }
            _ => DISP_MAX,
        };
        let motor = Motor::open(sysfs).or_else(|| Motor::gpio(sysfs, board));
        let anim = sysfs.join(TRIMUI_LED_ANIM);
        let led = anim.join("max_scale").is_file().then_some(Led::Anim(anim));
        DevicePlatform {
            root,
            sysfs: sysfs.to_path_buf(),
            backlight,
            max_brightness,
            battery: first_dir(&sysfs.join("class/power_supply"), is_battery),
            rtc: first_dir(&sysfs.join("class/rtc"), |_| true),
            motor,
            led,
            led_extra: Vec::new(),
            board,
            lights: Lights::default(),
            led_state: None,
            disp_dark: false,
            rainbow: None,
            cpu_stock: None,
        }
    }

    pub fn board(&self) -> Board {
        self.board
    }

    fn breadcrumb(&self, line: &str) {
        use std::io::Write;
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("ags-shutdown-trace.log"))
        {
            let _ = writeln!(f, "{} {line}", self.now());
            let _ = f.sync_all();
        }
    }

    pub fn trace_boot(&self) {
        self.breadcrumb("boot: platform up, trace working");
    }

    pub fn report(&self) -> String {
        let leaf = |p: &Path| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.display().to_string())
        };
        let name = |p: &Option<PathBuf>, absent: &str| match p {
            Some(p) => leaf(p),
            None => absent.to_string(),
        };
        let backlight = match &self.backlight {
            Some(Backlight::Class(dir)) => leaf(dir),
            Some(Backlight::Dispdbg(_)) => "dispdbg".to_string(),
            Some(Backlight::Disp(_)) => "disp".to_string(),
            None => "no backlight".to_string(),
        };
        let motor = match &self.motor {
            Some(m) => format!("motor {}", m.name),
            None => "no motor".to_string(),
        };
        format!(
            "{:?}: backlight {} (0 to {}), battery {}, {}, {motor}",
            self.board,
            backlight,
            self.max_brightness,
            name(&self.battery, "no battery"),
            name(&self.rtc, "no rtc"),
        )
    }
}

fn ran(what: &str, result: std::io::Result<std::process::ExitStatus>) {
    match result {
        Ok(status) if status.success() => eprintln!("slot: {what} ok"),
        Ok(status) => eprintln!("slot: {what} {status}"),
        Err(e) => eprintln!("slot: {what}: {e}"),
    }
}

pub fn has_bit(mask: &str, bit: u16) -> bool {
    let words: Vec<&str> = mask.split_whitespace().collect();
    let from_end = usize::from(bit) / 64;
    let Some(word) = words.len().checked_sub(from_end + 1).map(|i| words[i]) else {
        return false;
    };
    u64::from_str_radix(word, 16).is_ok_and(|w| w >> (bit % 64) & 1 == 1)
}

pub fn rumble_node(sysfs: &Path) -> Option<String> {
    let dir = first_dir(&sysfs.join("class/input"), |node| {
        fs::read_to_string(node.join("device/capabilities/ff"))
            .is_ok_and(|ff| has_bit(&ff, FF_RUMBLE))
    })?;
    Some(dir.file_name()?.to_string_lossy().into_owned())
}

pub fn motor_change(strength: u16, running: bool) -> Option<bool> {
    match (strength > 0, running) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

struct Motor {
    drive: Drive,
    name: String,
    running: bool,
}

enum Drive {
    Ff {
        node: fs::File,
        id: i16,
    },
    Gpio {
        value: PathBuf,
        voltage: PathBuf,
        volts: u32,
    },
}

#[repr(C)]
#[derive(Default)]
struct FfTrigger {
    button: u16,
    interval: u16,
}

#[repr(C)]
#[derive(Default)]
struct FfReplay {
    length: u16,
    delay: u16,
}

#[repr(C)]
#[derive(Default)]
struct FfRumble {
    strong: u16,
    weak: u16,
}

#[repr(C)]
struct FfEffect {
    kind: u16,
    id: i16,
    direction: u16,
    trigger: FfTrigger,
    replay: FfReplay,
    _align: u16,
    rumble: FfRumble,
    _tail: [u8; 28],
}

#[repr(C)]
struct FfEvent {
    sec: i64,
    usec: i64,
    kind: u16,
    code: u16,
    value: i32,
}

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

fn upload(node: &fs::File, id: i16) -> Option<i16> {
    let mut effect = FfEffect {
        kind: FF_RUMBLE,
        id,
        direction: 0,
        trigger: FfTrigger::default(),
        replay: FfReplay::default(),
        _align: 0,
        rumble: FfRumble {
            strong: u16::MAX,
            weak: u16::MAX,
        },
        _tail: [0; 28],
    };
    let rc = unsafe {
        ioctl(
            node.as_raw_fd(),
            EVIOCSFF,
            &mut effect as *mut FfEffect as *mut c_void,
        )
    };
    (rc >= 0).then_some(effect.id)
}

impl Motor {
    fn open(sysfs: &Path) -> Option<Motor> {
        let name = rumble_node(sysfs)?;
        let node = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(Path::new(DEV_INPUT).join(&name))
            .map_err(|e| eprintln!("slot: rumble {name}: {e}"))
            .ok()?;
        let Some(id) = upload(&node, -1) else {
            eprintln!("slot: rumble {name}: {}", std::io::Error::last_os_error());
            return None;
        };
        Some(Motor {
            drive: Drive::Ff { node, id },
            name,
            running: false,
        })
    }

    fn gpio(sysfs: &Path, board: Board) -> Option<Motor> {
        let value = sysfs.join(TRIMUI_MOTOR_GPIO);
        if !value.is_file() {
            return None;
        }
        Some(Motor {
            drive: Drive::Gpio {
                value,
                voltage: sysfs.join(TRIMUI_MOTOR_VOLTAGE),
                volts: board.motor_volts(),
            },
            name: "gpio227".to_string(),
            running: false,
        })
    }

    fn play(&mut self, on: bool) {
        match &mut self.drive {
            Drive::Ff { node, id } => {
                if on {
                    match upload(node, *id).or_else(|| upload(node, -1)) {
                        Some(new) => *id = new,
                        None => eprintln!(
                            "slot: rumble {}: {}",
                            self.name,
                            std::io::Error::last_os_error()
                        ),
                    }
                }
                let ev = FfEvent {
                    sec: 0,
                    usec: 0,
                    kind: EV_FF,
                    code: *id as u16,
                    value: i32::from(on),
                };
                let bytes = unsafe {
                    std::slice::from_raw_parts(
                        &ev as *const FfEvent as *const u8,
                        size_of::<FfEvent>(),
                    )
                };
                match (&*node).write_all(bytes) {
                    Ok(()) => self.running = on,
                    Err(e) => eprintln!("slot: rumble: {e}"),
                }
            }
            Drive::Gpio {
                value,
                voltage,
                volts,
            } => {
                if on && voltage.is_file() {
                    let _ = fs::write(&*voltage, volts.to_string());
                }
                match fs::write(&*value, if on { "1" } else { "0" }) {
                    Ok(()) => self.running = on,
                    Err(e) => eprintln!("slot: rumble {}: {e}", value.display()),
                }
            }
        }
    }
}

impl Drop for Motor {
    fn drop(&mut self) {
        if self.running {
            self.play(false);
        }
    }
}

const ANIM_ZONES: [(&str, &str); 4] = [
    ("m", "max_scale"),
    ("lr", "max_scale_lr"),
    ("f1", "max_scale_f1f2"),
    ("rear", "max_scale_rear"),
];

const ANIM_BATTERY_MAX: u8 = 30;

fn battery_colour(state: LedState) -> Option<u32> {
    match state {
        LedState::Off | LedState::Running => None,
        LedState::Low => Some(0xFF0000),
        LedState::Charging => Some(0xFF8C00),
        LedState::Charged => Some(0x00FF00),
    }
}

const ANIMATION_TICK: Duration = Duration::from_millis(80);

/// Plays an effect the light driver cannot: every tick each lit zone gets
/// the colour `effect_colour` gives it, until the flag handed back is set.
fn spawn_animation(
    dir: PathBuf,
    lit: usize,
    effect: LightsEffect,
    base: u32,
) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let spawned = std::thread::Builder::new()
        .name("slot-lights".into())
        .spawn(move || {
            let began = std::time::Instant::now();
            let mut seed = 0x2545_F491_u32;
            let mut shown = [u32::MAX; ANIM_ZONES.len()];
            while !flag.load(Ordering::Relaxed) {
                let ms = began.elapsed().as_millis() as u64;
                for (i, (zone, _)) in ANIM_ZONES[..lit].iter().enumerate() {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    let rgb = effect_colour(effect, base, ms, i, seed);
                    if shown[i] == rgb {
                        continue;
                    }
                    shown[i] = rgb;
                    let zones: &[&str] = if *zone == "f1" {
                        &["f1", "f2"]
                    } else {
                        &[zone]
                    };
                    for z in zones {
                        let _ = fs::write(
                            dir.join(format!("effect_rgb_hex_{z}")),
                            format!("{rgb:06X}\n"),
                        );
                        let _ = fs::write(dir.join(format!("effect_{z}")), "4\n");
                    }
                }
                std::thread::sleep(ANIMATION_TICK);
            }
        });
    if let Err(e) = spawned {
        eprintln!("slot: lights: {e}");
    }
    stop
}

const OCEAN: [u32; 3] = [0x0030FF, 0x00C8FF, 0x00FFB0];
const AURORA: [u32; 3] = [0x00FF60, 0x00D8FF, 0x9B30FF];
const SUNSET: [u32; 3] = [0xFF6A00, 0xFF2D6F, 0x8E2DE2];
const CANDY: [u32; 3] = [0xFF4FB8, 0x3FE0FF, 0xFFE14F];

/// The colour of lit zone `zone` (0 top bar, 1 sides, 2 FN keys, 3 rear)
/// `ms` into `effect`. `noise` is fresh randomness for flickering effects.
pub fn effect_colour(effect: LightsEffect, base: u32, ms: u64, zone: usize, noise: u32) -> u32 {
    let zone_f = zone as f32;
    match effect {
        LightsEffect::Rainbow => hue_rgb(ms as f32 * 0.06),
        LightsEffect::RainbowWave => hue_rgb(ms as f32 * 0.06 + zone_f * 90.0),
        LightsEffect::Ocean => palette(&OCEAN, 6000.0, ms as f32 + zone_f * 900.0),
        LightsEffect::Aurora => palette(&AURORA, 7000.0, ms as f32 + zone_f * 1200.0),
        LightsEffect::Sunset => palette(&SUNSET, 6000.0, ms as f32 + zone_f * 900.0),
        LightsEffect::Candy => palette(&CANDY, 4500.0, ms as f32 + zone_f * 700.0),
        LightsEffect::Fire => {
            let heat = (noise & 0xFF) as f32 / 255.0;
            let glow = 0.55 + 0.45 * ((noise >> 8) & 0xFF) as f32 / 255.0;
            scale(mix(0xFF1800, 0xFFB000, heat), glow)
        }
        LightsEffect::Police => {
            let half = (ms / 400).is_multiple_of(2);
            let flash = (ms % 400) < 300 && (ms % 100) < 70;
            match (flash, half == zone.is_multiple_of(2)) {
                (false, _) => 0,
                (true, true) => 0xFF0000,
                (true, false) => 0x0030FF,
            }
        }
        LightsEffect::Heartbeat => {
            let t = (ms % 1200) as f32;
            let beat = |at: f32| {
                let d = (t - at) / 90.0;
                (-d * d).exp()
            };
            scale(base, 0.12 + 0.88 * beat(120.0).max(0.8 * beat(380.0)))
        }
        LightsEffect::Static | LightsEffect::Breathe | LightsEffect::Blink => base,
    }
}

/// Smoothly round a looping list of colours, one full lap per `period` ms.
fn palette(colours: &[u32], period: f32, ms: f32) -> u32 {
    let n = colours.len();
    let pos = (ms.rem_euclid(period) / period) * n as f32;
    let i = pos as usize % n;
    let t = pos.fract();
    let t = t * t * (3.0 - 2.0 * t);
    mix(colours[i], colours[(i + 1) % n], t)
}

fn mix(a: u32, b: u32, t: f32) -> u32 {
    let ch = |c: u32, shift: u32| ((c >> shift) & 0xFF) as f32;
    let one = |shift: u32| {
        let v = ch(a, shift) + (ch(b, shift) - ch(a, shift)) * t.clamp(0.0, 1.0);
        (v.round() as u32).min(255) << shift
    };
    one(16) | one(8) | one(0)
}

fn scale(c: u32, by: f32) -> u32 {
    mix(0, c, by)
}

/// A fully saturated, full brightness colour at `hue` degrees, as 0xRRGGBB.
pub fn hue_rgb(hue: f32) -> u32 {
    let h = hue.rem_euclid(360.0) / 60.0;
    let x = 1.0 - (h % 2.0 - 1.0).abs();
    let (r, g, b) = match h as u32 {
        0 => (1.0, x, 0.0),
        1 => (x, 1.0, 0.0),
        2 => (0.0, 1.0, x),
        3 => (0.0, x, 1.0),
        4 => (x, 0.0, 1.0),
        _ => (1.0, 0.0, x),
    };
    let byte = |v: f32| (v * 255.0).round() as u32;
    byte(r) << 16 | byte(g) << 8 | byte(b)
}

const CPUFREQ: &str = "devices/system/cpu/cpu0/cpufreq";

/// Battery Saver keeps the clock at or under this.
const SAVER_MAX_KHZ: u32 = 1_200_000;

#[derive(Clone, PartialEq, Eq, Debug)]
struct CpuSettings {
    governor: String,
    min: u32,
    max: u32,
}

impl CpuSettings {
    fn read(dir: &Path) -> Option<CpuSettings> {
        Some(CpuSettings {
            governor: fs::read_to_string(dir.join("scaling_governor"))
                .ok()?
                .trim()
                .to_string(),
            min: read_number(&dir.join("scaling_min_freq"))?,
            max: read_number(&dir.join("scaling_max_freq"))?,
        })
    }

    fn frequencies(dir: &Path) -> Vec<u32> {
        let mut f: Vec<u32> = fs::read_to_string(dir.join("scaling_available_frequencies"))
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|v| v.parse().ok())
            .collect();
        f.sort_unstable();
        f
    }

    fn governor_of(dir: &Path, wanted: &[&str], fallback: &str) -> String {
        let have = fs::read_to_string(dir.join("scaling_available_governors")).unwrap_or_default();
        wanted
            .iter()
            .find(|g| have.split_whitespace().any(|h| h == **g))
            .map_or(fallback.to_string(), |g| g.to_string())
    }

    fn saver(&self, dir: &Path) -> CpuSettings {
        let f = CpuSettings::frequencies(dir);
        let lowest = f.first().copied().unwrap_or(self.min);
        let cap = f
            .iter()
            .copied()
            .filter(|&k| k <= SAVER_MAX_KHZ)
            .max()
            .unwrap_or(self.max.min(SAVER_MAX_KHZ));
        CpuSettings {
            governor: CpuSettings::governor_of(
                dir,
                &["conservative", "ondemand", "schedutil"],
                &self.governor,
            ),
            min: lowest,
            max: cap.max(lowest),
        }
    }

    fn performance(&self, dir: &Path) -> CpuSettings {
        let f = CpuSettings::frequencies(dir);
        let top = f.last().copied().unwrap_or(self.max);
        CpuSettings {
            governor: CpuSettings::governor_of(dir, &["performance"], &self.governor),
            min: top,
            max: top,
        }
    }

    /// Opens the range out first, so the min never sits above the max on
    /// the way, then narrows it to what is wanted.
    fn write(&self, dir: &Path) {
        let put = |name: &str, v: String| {
            if let Err(e) = fs::write(dir.join(name), v) {
                eprintln!("slot: cpu {name}: {e}");
            }
        };
        let f = CpuSettings::frequencies(dir);
        if let (Some(lo), Some(hi)) = (f.first(), f.last()) {
            put("scaling_min_freq", lo.to_string());
            put("scaling_max_freq", hi.to_string());
        }
        put("scaling_governor", self.governor.clone());
        put("scaling_max_freq", self.max.to_string());
        put("scaling_min_freq", self.min.to_string());
    }
}

const SUSPEND_TRIES: u32 = 5;

/// Writes "mem" to /sys/power/state, which returns once the handheld wakes.
/// As NextUI's suspend script found on the Brick, the write can report a
/// failure after a real sleep, so anything longer than a few seconds counts
/// as slept; a quick failure is retried a few times.
fn suspend(state: &Path) -> bool {
    for attempt in 1..=SUSPEND_TRIES {
        let began = std::time::Instant::now();
        match fs::write(state, "mem") {
            Ok(()) => return true,
            Err(_) if began.elapsed() > Duration::from_secs(5) => return true,
            Err(e) => {
                eprintln!("slot: suspend attempt {attempt}: {e}");
                std::thread::sleep(Duration::from_secs(2));
            }
        }
    }
    false
}

fn anim_zone(dir: &Path, zone: &str, rgb: u32, effect: LightsEffect) {
    let (id, ms) = match effect {
        LightsEffect::Static => (4, 1000),
        LightsEffect::Breathe => (2, 2000),
        LightsEffect::Blink => (5, 1000),
        // The Brick's driver has none of these; spawn_animation draws them
        // over a static light instead.
        _ => (4, 1000),
    };
    let write = |name: String, body: String| {
        let _ = fs::write(dir.join(name), body);
    };
    write(format!("effect_rgb_hex_{zone}"), format!("{rgb:06X}\n"));
    write(format!("effect_duration_{zone}"), format!("{ms}\n"));
    write(format!("effect_cycles_{zone}"), "-1\n".to_string());
    write(format!("effect_{zone}"), format!("{id}\n"));
}

fn disp_brightness(node: &Path, value: u32) -> std::io::Result<()> {
    let disp = fs::OpenOptions::new().read(true).write(true).open(node)?;
    let mut param: [c_ulong; 4] = [0, c_ulong::from(value), 0, 0];
    let rc = unsafe {
        ioctl(
            disp.as_raw_fd(),
            DISP_LCD_SET_BRIGHTNESS,
            param.as_mut_ptr() as *mut c_void,
        )
    };
    if rc < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn first_dir(parent: &Path, keep: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut names: Vec<PathBuf> = fs::read_dir(parent)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    names.sort();
    names.into_iter().find(|p| keep(p))
}

fn is_battery(dir: &Path) -> bool {
    fs::read_to_string(dir.join("type")).is_ok_and(|t| t.trim() == "Battery")
        && dir.join("capacity").is_file()
}

fn read_number(path: &Path) -> Option<u32> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn charge_at(dir: &Path) -> Charge {
    let Ok(text) = fs::read_to_string(dir.join("status")) else {
        return Charge::Unknown;
    };
    match text.trim() {
        "Charging" => Charge::Charging,
        "Discharging" => Charge::Discharging,
        "Full" => Charge::Full,
        _ => Charge::Unknown,
    }
}

fn headphones_in(gpio: &str) -> bool {
    gpio.lines().any(|l| {
        let name = l.to_ascii_lowercase();
        let jack = ["headphone", "hpdet", "hp-det", "hp_det", "hp jack"]
            .iter()
            .any(|k| name.contains(k));
        jack && l.rsplit_once(')').is_some_and(|(_, state)| {
            matches!(state.split_whitespace().nth(1), Some("hi") | Some("1"))
        })
    })
}

fn switch_in(sysfs: &Path) -> bool {
    ["class/switch/h2w/state", "class/switch/headphone/state"]
        .iter()
        .any(|rel| {
            fs::read_to_string(sysfs.join(rel))
                .ok()
                .and_then(|s| s.trim().parse::<i32>().ok())
                .is_some_and(|n| n != 0)
        })
}

impl Platform for DevicePlatform {
    fn headphones(&self) -> bool {
        switch_in(&self.sysfs)
            || fs::read_to_string(self.sysfs.join("kernel/debug/gpio")).is_ok_and(|g| headphones_in(&g))
    }

    fn card_read_only(&self) -> bool {
        read_only(&self.root)
    }

    fn set_backlight(&mut self, step: u8) {
        let Some(backlight) = &self.backlight else {
            return;
        };
        let step = u32::from(step).min(TOP_STEP);
        let value = self.max_brightness * step / TOP_STEP;
        let write = |file: PathBuf, body: String| {
            if let Err(e) = fs::write(&file, body) {
                eprintln!("slot: {}: {e}", file.display());
            }
        };
        match backlight {
            Backlight::Class(dir) => write(dir.join("brightness"), value.to_string()),
            Backlight::Dispdbg(dir) => {
                write(dir.join("name"), "lcd0".to_string());
                write(dir.join("command"), "setbl".to_string());
                write(dir.join("param"), value.to_string());
                write(dir.join("start"), "1".to_string());
            }
            Backlight::Disp(node) => {
                // As NextUI does on the Brick: a panel brought back from 0 goes
                // through a low level first, or it can stay dark.
                if self.disp_dark && value > 0 {
                    let _ = disp_brightness(node, DISP_WAKE);
                }
                if let Err(e) = disp_brightness(node, value) {
                    eprintln!("slot: {}: {e}", node.display());
                }
                self.disp_dark = value == 0;
            }
        }
    }

    fn battery(&self) -> Option<Battery> {
        let dir = self.battery.as_ref()?;
        let percent = read_number(&dir.join("capacity"))?;
        Some(Battery {
            percent: percent.min(100) as u8,
            charge: charge_at(dir),
        })
    }

    fn charge(&self) -> Charge {
        self.battery
            .as_ref()
            .map_or(Charge::Unknown, |dir| charge_at(dir))
    }

    fn set_led(&mut self, state: LedState) {
        self.led_state = Some(state);
        if let Some(stop) = self.rainbow.take() {
            stop.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let lights = self.lights;
        let lit = match lights.mode {
            LightsMode::Off => false,
            LightsMode::Battery => !matches!(state, LedState::Off | LedState::Low),
            LightsMode::Colour => state != LedState::Off,
        };
        for light in &self.led_extra {
            light.set(lit);
        }
        let Some(led) = &self.led else {
            return;
        };
        let shown = match (lights.mode, state) {
            (_, LedState::Off) | (LightsMode::Off, _) => None,
            (LightsMode::Colour, _) => Some((lights.rgb, lights.effect, lights.percent)),
            (LightsMode::Battery, state) => battery_colour(state).map(|rgb| {
                let percent = lights.percent.min(ANIM_BATTERY_MAX);
                (rgb, LightsEffect::Static, percent)
            }),
        };
        match led {
            Led::Multi(dir) => {
                let rgb = match lights.mode {
                    LightsMode::Battery => match state {
                        LedState::Off => 0,
                        LedState::Running | LedState::Charged => 0x00FF00,
                        LedState::Low => 0xFF0000,
                        LedState::Charging => 0xFF8C00,
                    },
                    _ => shown.map_or(0, |(rgb, _, _)| rgb),
                };
                let (r, g, b) = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF);
                let _ = fs::write(dir.join("multi_intensity"), format!("{r} {g} {b}\n"));
                let _ = fs::write(dir.join("brightness"), "255\n");
            }
            Led::Mono { dir, max } => {
                let on = match lights.mode {
                    LightsMode::Battery => !matches!(state, LedState::Off | LedState::Low),
                    _ => shown.is_some(),
                };
                let value = if on { *max } else { 0 };
                let _ = fs::write(dir.join("brightness"), format!("{value}\n"));
            }
            Led::Anim(dir) => {
                let lit = match (lights.mode, shown) {
                    (_, None) => 0,
                    (LightsMode::Colour, _) => ANIM_ZONES.len(),
                    _ => 1,
                };
                for (_, scale) in &ANIM_ZONES[lit..] {
                    let _ = fs::write(dir.join(scale), "0\n");
                }
                let Some((rgb, effect, percent)) = shown else {
                    return;
                };
                let shown_effect = match effect.animated() {
                    true => LightsEffect::Static,
                    false => effect,
                };
                for (zone, scale) in &ANIM_ZONES[..lit] {
                    anim_zone(dir, zone, rgb, shown_effect);
                    if *zone == "f1" {
                        anim_zone(dir, "f2", rgb, shown_effect);
                    }
                    let _ = fs::write(dir.join(scale), format!("{percent}\n"));
                }
                if effect.animated() {
                    self.rainbow = Some(spawn_animation(dir.clone(), lit, effect, rgb));
                }
            }
        }
    }

    fn set_cpu(&mut self, profile: CpuProfile) {
        let dir = self.sysfs.join(CPUFREQ);
        let Some(now) = CpuSettings::read(&dir) else {
            return;
        };
        let stock = self.cpu_stock.get_or_insert(now).clone();
        let want = match profile {
            CpuProfile::Stock => stock,
            CpuProfile::Saver => stock.saver(&dir),
            CpuProfile::Performance => stock.performance(&dir),
        };
        want.write(&dir);
        eprintln!(
            "slot: cpu {profile:?}: {} {}-{} kHz",
            want.governor, want.min, want.max
        );
    }

    fn can_deep_sleep(&self) -> bool {
        self.board.is_trimui() && self.sysfs.join("power/state").is_file()
    }

    fn deep_sleep(&mut self) -> bool {
        if !self.can_deep_sleep() {
            return false;
        }
        self.breadcrumb("deep sleep: syncing");
        let _ = Command::new("sync").status();
        let net = self.root.join("System/net.sh");
        let _ = Command::new("/bin/sh").arg(&net).arg("pause").status();
        let slept = suspend(&self.sysfs.join("power/state"));
        self.breadcrumb(if slept {
            "deep sleep: woke"
        } else {
            "deep sleep: the kernel would not suspend"
        });
        let _ = Command::new("/bin/sh").arg(&net).arg("resume").spawn();
        slept
    }

    fn set_lights(&mut self, lights: Lights) {
        if self.lights == lights {
            return;
        }
        self.lights = lights;
        if let Some(state) = self.led_state {
            self.set_led(state);
        }
    }

    fn restart(&mut self) -> ! {
        self.breadcrumb("restart: reached slot, about to sync");
        let _ = Command::new("sync").status();
        self.breadcrumb("restart: sync returned, about to signal init");
        let _ = Command::new("reboot").status();
        std::thread::sleep(Duration::from_secs(10));
        std::process::exit(0)
    }

    fn poweroff(&mut self) -> ! {
        self.breadcrumb("poweroff: reached slot, about to sync");
        let _ = Command::new("sync").status();
        self.breadcrumb("poweroff: sync returned, about to signal init");
        let _ = Command::new("poweroff").status();
        self.breadcrumb("poweroff: signalled init, waiting for it to take the machine down");
        std::thread::sleep(Duration::from_secs(10));
        std::process::exit(0)
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }

    fn set_clock(&mut self, secs: i64) {
        ran(
            "date",
            Command::new("date").arg(format!("-s@{secs}")).status(),
        );
        ran(
            "hwclock",
            Command::new("hwclock").args(["-w", "-u"]).status(),
        );
    }

    fn relink_adb(&mut self) -> bool {
        let gadget = self.sysfs.join("kernel/config/usb_gadget/g1/UDC");
        if !gadget.is_file() {
            return false;
        }
        let Some(udc) = first_dir(&self.sysfs.join("class/udc"), |_| true) else {
            return false;
        };
        let Some(name) = udc.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            return false;
        };
        let _ = fs::write(&gadget, "\n");
        match fs::write(&gadget, &name) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("slot: adb {}: {e}", gadget.display());
                false
            }
        }
    }

    fn set_rumble(&mut self, strength: u16) {
        let Some(motor) = &mut self.motor else {
            return;
        };
        if let Some(on) = motor_change(strength, motor.running) {
            motor.play(on);
        }
    }
}

pub fn read_only(path: &Path) -> bool {
    fs::read_to_string("/proc/mounts").is_ok_and(|m| read_only_in(&m, path))
}

pub fn read_only_in(mounts: &str, path: &Path) -> bool {
    mounts
        .lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let point = Path::new(f.nth(1)?);
            let options = f.nth(1)?;
            path.starts_with(point).then_some((point, options))
        })
        .max_by_key(|(point, _)| point.as_os_str().len())
        .is_some_and(|(_, options)| options.split(',').any(|o| o == "ro"))
}
