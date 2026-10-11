use std::ffi::{c_int, c_ulong, c_void};
use std::fs;
use std::io::Write;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{Battery, Charge, LedState, Lights, LightsEffect, LightsMode, Platform};

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
    board: Board,
    lights: Lights,
    led_state: Option<LedState>,
    disp_dark: bool,
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
        let led = first_dir(&sysfs.join("class/leds"), |d| {
            d.join("multi_intensity").is_file() || d.join("brightness").is_file()
        })
        .map(|dir| {
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
            board,
            lights: Lights::default(),
            led_state: None,
            disp_dark: false,
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
            board,
            lights: Lights::default(),
            led_state: None,
            disp_dark: false,
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

const ANIM_ZONE_RAINBOW: u32 = 16;

fn anim_zone(dir: &Path, zone: &str, rgb: u32, effect: LightsEffect) {
    let (id, ms) = match effect {
        LightsEffect::Static => (4, 1000),
        LightsEffect::Breathe => (2, 2000),
        LightsEffect::Blink => (5, 1000),
        // The top bar and the side lights have their own rainbow, as
        // NextUI's ledcontrol lists them for the Brick.
        LightsEffect::Rainbow if matches!(zone, "m" | "lr") => (ANIM_ZONE_RAINBOW, 3000),
        LightsEffect::Rainbow => (8, 3000),
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
    gpio.lines()
        .find(|l| l.contains("|Headphone detection"))
        .and_then(|l| l.rsplit_once(')'))
        .is_some_and(|(_, state)| state.split_whitespace().nth(1) == Some("hi"))
}

impl Platform for DevicePlatform {
    fn headphones(&self) -> bool {
        fs::read_to_string(self.sysfs.join("kernel/debug/gpio")).is_ok_and(|g| headphones_in(&g))
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
        let Some(led) = &self.led else {
            return;
        };
        let lights = self.lights;
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
                for (zone, scale) in &ANIM_ZONES[..lit] {
                    anim_zone(dir, zone, rgb, effect);
                    if *zone == "f1" {
                        anim_zone(dir, "f2", rgb, effect);
                    }
                    let _ = fs::write(dir.join(scale), format!("{percent}\n"));
                }
            }
        }
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
