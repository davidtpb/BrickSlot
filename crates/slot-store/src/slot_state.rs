use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::gb_palette::GbPalette;
use crate::lights::{LightColour, LightEffect, LightLevel, LightMode};
use crate::platform::Platform;

pub const BRIGHTNESS_MAX: u8 = 9;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const VOLUME_MAX: u8 = 100;

pub const UTC_OFFSET_MIN: i16 = -720;
pub const UTC_OFFSET_MAX: i16 = 840;

pub const FF_SPEEDS: [u8; 4] = [2, 3, 4, 6];

pub const FF_SPEED_DEFAULT: u8 = 6;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotState {
    pub cart: Option<String>,
    pub cart_platform: Option<Platform>,
    pub brightness: u8,
    pub blue_light: u8,
    pub volume: u8,
    pub volume_hp: u8,
    pub muted: bool,
    pub muted_hp: bool,
    pub clock_set: bool,
    pub utc_offset_min: i16,
    pub rumble: bool,
    pub ff_speed: u8,
    pub ff_sound: bool,
    pub colour_correction: bool,
    pub shader_gba: Shader,
    pub shader_gb: Shader,
    pub eject_save: bool,
    pub turbo: bool,
    pub rewind: bool,
    pub gb_palettes: bool,
    pub gb_palette: GbPalette,
    pub light_mode: LightMode,
    pub light_colour: LightColour,
    pub light_effect: LightEffect,
    pub light_level: LightLevel,
    pub wifi: bool,
    pub syncthing: bool,
    pub achievements: bool,
    pub save_format: SaveFormat,
    pub auto_time: bool,
    pub cpu: CpuMode,
    pub look: Look,
    pub tint: Tint,
    /// Ticks and blips as the menus are used.
    pub menu_sounds: bool,
    pub bluetooth: bool,
    pub clock_12h: bool,
    pub start_fresh: bool,
}

/// How battery saves are written: RetroArch's `.srm` with the clock in a
/// `.rtc` beside it, or slot's own `.sav` with the clock on the end.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum SaveFormat {
    #[default]
    RetroArch,
    Slot,
}

impl SaveFormat {
    pub fn name(self) -> &'static str {
        match self {
            SaveFormat::RetroArch => "retroarch",
            SaveFormat::Slot => "slot",
        }
    }

    pub fn parse(name: &str) -> Option<SaveFormat> {
        [SaveFormat::RetroArch, SaveFormat::Slot]
            .into_iter()
            .find(|f| f.name() == name)
    }

    pub fn other(self) -> SaveFormat {
        match self {
            SaveFormat::RetroArch => SaveFormat::Slot,
            SaveFormat::Slot => SaveFormat::RetroArch,
        }
    }
}

/// How the shelf is drawn.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Look {
    /// slot's own shelf above the console's cartridge slot.
    Classic,
    /// A light carousel of tilting carts, after the Socket frontend.
    Socket,
}

impl Look {
    pub fn name(self) -> &'static str {
        match self {
            Look::Classic => "classic",
            Look::Socket => "socket",
        }
    }

    pub fn parse(name: &str) -> Option<Look> {
        [Look::Classic, Look::Socket]
            .into_iter()
            .find(|l| l.name() == name)
    }

    pub fn other(self) -> Look {
        match self {
            Look::Classic => Look::Socket,
            Look::Socket => Look::Classic,
        }
    }
}

/// The colours of the Socket look.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Tint {
    /// Neutral greys.
    Default,
    /// Soft lilac and pink.
    Pastel,
    /// Warm browns, like an old photograph.
    Sepia,
}

impl Tint {
    pub const ALL: [Tint; 3] = [Tint::Default, Tint::Pastel, Tint::Sepia];

    pub fn name(self) -> &'static str {
        match self {
            Tint::Default => "default",
            Tint::Pastel => "pastel",
            Tint::Sepia => "sepia",
        }
    }

    pub fn parse(name: &str) -> Option<Tint> {
        Tint::ALL.into_iter().find(|t| t.name() == name)
    }

    pub fn step(self, right: bool) -> Tint {
        let n = Tint::ALL.len();
        let at = Tint::ALL.iter().position(|&t| t == self).unwrap_or(0);
        Tint::ALL[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

/// How hard the processor is driven: left as the system set it, held back
/// for battery, or flat out.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum CpuMode {
    #[default]
    Stock,
    Saver,
    Performance,
}

impl CpuMode {
    pub const ALL: [CpuMode; 3] = [CpuMode::Stock, CpuMode::Saver, CpuMode::Performance];

    pub fn name(self) -> &'static str {
        match self {
            CpuMode::Stock => "stock",
            CpuMode::Saver => "saver",
            CpuMode::Performance => "performance",
        }
    }

    pub fn parse(name: &str) -> Option<CpuMode> {
        CpuMode::ALL.into_iter().find(|m| m.name() == name)
    }

    pub fn step(self, right: bool) -> CpuMode {
        let n = CpuMode::ALL.len();
        let at = CpuMode::ALL.iter().position(|&m| m == self).unwrap_or(0);
        CpuMode::ALL[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shader {
    Off,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
}

impl Shader {
    pub const GBA: [Shader; 4] = [Shader::Off, Shader::Lcd3x, Shader::Grid, Shader::Dot];
    pub const GB: [Shader; 3] = [Shader::Off, Shader::Grid, Shader::Simpletex];

    pub fn name(self) -> &'static str {
        match self {
            Shader::Off => "off",
            Shader::Lcd3x => "lcd3x",
            Shader::Grid => "grid",
            Shader::Dot => "dot",
            Shader::Simpletex => "simpletex",
        }
    }

    pub fn parse(name: &str) -> Option<Shader> {
        [
            Shader::Off,
            Shader::Lcd3x,
            Shader::Grid,
            Shader::Dot,
            Shader::Simpletex,
        ]
        .into_iter()
        .find(|s| s.name() == name)
    }

    pub fn step(self, choices: &[Shader], right: bool) -> Shader {
        let n = choices.len();
        let at = choices.iter().position(|&s| s == self).unwrap_or(0);
        choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

impl Default for SlotState {
    fn default() -> Self {
        SlotState {
            cart: None,
            cart_platform: None,
            brightness: 5,
            blue_light: 0,
            volume: 60,
            volume_hp: 60,
            muted: false,
            muted_hp: false,
            clock_set: false,
            utc_offset_min: 0,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction: false,
            shader_gba: Shader::Grid,
            shader_gb: Shader::Simpletex,
            eject_save: true,
            turbo: true,
            rewind: true,
            gb_palettes: false,
            gb_palette: GbPalette::DEFAULT,
            light_mode: LightMode::Battery,
            light_colour: LightColour::White,
            light_effect: LightEffect::Static,
            light_level: LightLevel::Medium,
            wifi: false,
            syncthing: false,
            achievements: false,
            save_format: SaveFormat::RetroArch,
            auto_time: true,
            cpu: CpuMode::Stock,
            look: Look::Classic,
            tint: Tint::Default,
            menu_sounds: true,
            bluetooth: false,
            clock_12h: false,
            start_fresh: true,
        }
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("Config").join("slot.state")
}

pub fn read_slot_state(root: &Path) -> SlotState {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default()
}

pub fn write_slot_state(root: &Path, s: &SlotState) -> std::io::Result<()> {
    let text = format!(
        "cart={}\ncart_platform={}\nbrightness={}\nblue_light={}\nvolume={}\nvolume_hp={}\nmuted={}\nmuted_hp={}\nclock_set={}\nutc_offset_min={}\nrumble={}\nff_speed={}\nff_sound={}\ncolour_correction={}\nshader_gba={}\nshader_gb={}\neject_save={}\nturbo={}\nrewind={}\ngb_palettes={}\ngb_palette={}\nlight_mode={}\nlight_colour={}\nlight_effect={}\nlight_level={}\nwifi={}\nsyncthing={}\nachievements={}\nsave_format={}\nauto_time={}\ncpu={}\nlook={}\nclock_12h={}\nstart_fresh={}\ntint={}\nmenu_sounds={}\nbluetooth={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.cart_platform.map_or(String::new(), platform_key),
        s.brightness,
        s.blue_light,
        s.volume,
        s.volume_hp,
        s.muted as u8,
        s.muted_hp as u8,
        s.clock_set as u8,
        s.utc_offset_min,
        s.rumble as u8,
        s.ff_speed,
        s.ff_sound as u8,
        s.colour_correction as u8,
        s.shader_gba.name(),
        s.shader_gb.name(),
        s.eject_save as u8,
        s.turbo as u8,
        s.rewind as u8,
        s.gb_palettes as u8,
        s.gb_palette.core_name(),
        s.light_mode.name(),
        s.light_colour.name(),
        s.light_effect.name(),
        s.light_level.name(),
        s.wifi as u8,
        s.syncthing as u8,
        s.achievements as u8,
        s.save_format.name(),
        s.auto_time as u8,
        s.cpu.name(),
        s.look.name(),
        s.clock_12h,
        s.start_fresh,
        s.tint.name(),
        s.menu_sounds,
        s.bluetooth
    );
    atomic_write(&state_path(root), text.as_bytes())
}

fn parse(text: &str) -> Option<SlotState> {
    let mut cart = None;
    let mut cart_platform = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut volume = None;
    let mut volume_hp = None;
    let mut muted = None;
    let mut muted_hp = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut rumble = None;
    let mut ff_speed = None;
    let mut ff_sound = None;
    let mut colour_correction = None;
    let mut shader_gba = None;
    let mut shader_gb = None;
    let mut eject_save = None;
    let mut turbo = None;
    let mut rewind = None;
    let mut gb_palettes = None;
    let mut gb_palette = None;
    let mut light_mode = None;
    let mut light_colour = None;
    let mut light_effect = None;
    let mut light_level = None;
    let mut wifi = None;
    let mut syncthing = None;
    let mut achievements = None;
    let mut save_format = None;
    let mut auto_time = None;
    let mut cpu = None;
    let mut look = None;
    let mut tint = None;
    let mut menu_sounds = None;
    let mut bluetooth = None;
    let mut clock_12h = None;
    let mut start_fresh = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "cart" => cart = Some(value.to_string()),
            "cart_platform" => cart_platform = platform_value(value),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "volume_hp" => volume_hp = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "muted_hp" => muted_hp = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            "rumble" => rumble = flag(value),
            "ff_speed" => ff_speed = ff_speed_value(value),
            "ff_sound" => ff_sound = flag(value),
            "colour_correction" => colour_correction = flag(value),
            "shader_gba" => shader_gba = Shader::parse(value).filter(|s| Shader::GBA.contains(s)),
            "eject_save" => eject_save = flag(value),
            "turbo" => turbo = flag(value),
            "rewind" => rewind = flag(value),
            "gb_palettes" => gb_palettes = flag(value),
            "gb_palette" => gb_palette = GbPalette::parse(value),
            "light_mode" => light_mode = LightMode::parse(value),
            "light_colour" => light_colour = LightColour::parse(value),
            "light_effect" => light_effect = LightEffect::parse(value),
            "light_level" => light_level = LightLevel::parse(value),
            "wifi" => wifi = flag(value),
            "syncthing" => syncthing = flag(value),
            "achievements" => achievements = flag(value),
            "save_format" => save_format = SaveFormat::parse(value),
            "auto_time" => auto_time = flag(value),
            "cpu" => cpu = CpuMode::parse(value),
            "look" => look = Look::parse(value),
            "tint" => tint = Tint::parse(value),
            "menu_sounds" => menu_sounds = value.parse().ok(),
            "bluetooth" => bluetooth = value.parse().ok(),
            "clock_12h" => clock_12h = value.parse().ok(),
            "start_fresh" => start_fresh = value.parse().ok(),
            "shader_gb" => shader_gb = Shader::parse(value).filter(|s| Shader::GB.contains(s)),
            _ => {}
        }
    }
    let cart = cart?;
    let fallback = SlotState::default();
    Some(SlotState {
        cart: (!cart.is_empty()).then_some(cart),
        cart_platform,
        brightness: brightness?,
        blue_light: blue_light?,
        volume: volume?,
        volume_hp: volume_hp.or(volume)?,
        muted: muted?,
        muted_hp: muted_hp.or(muted)?,
        clock_set: clock_set?,
        utc_offset_min: utc_offset_min?,
        rumble: rumble.unwrap_or(fallback.rumble),
        ff_speed: ff_speed.unwrap_or(fallback.ff_speed),
        ff_sound: ff_sound.unwrap_or(fallback.ff_sound),
        colour_correction: colour_correction.unwrap_or(fallback.colour_correction),
        shader_gba: shader_gba.unwrap_or(fallback.shader_gba),
        shader_gb: shader_gb.unwrap_or(fallback.shader_gb),
        eject_save: eject_save.unwrap_or(fallback.eject_save),
        turbo: turbo.unwrap_or(fallback.turbo),
        rewind: rewind.unwrap_or(fallback.rewind),
        gb_palettes: gb_palettes.unwrap_or(fallback.gb_palettes),
        gb_palette: gb_palette.unwrap_or(fallback.gb_palette),
        light_mode: light_mode.unwrap_or(fallback.light_mode),
        light_colour: light_colour.unwrap_or(fallback.light_colour),
        light_effect: light_effect.unwrap_or(fallback.light_effect),
        light_level: light_level.unwrap_or(fallback.light_level),
        wifi: wifi.unwrap_or(fallback.wifi),
        syncthing: syncthing.unwrap_or(fallback.syncthing),
        achievements: achievements.unwrap_or(fallback.achievements),
        save_format: save_format.unwrap_or(fallback.save_format),
        auto_time: auto_time.unwrap_or(fallback.auto_time),
        cpu: cpu.unwrap_or(fallback.cpu),
        look: look.unwrap_or(fallback.look),
        tint: tint.unwrap_or(fallback.tint),
        menu_sounds: menu_sounds.unwrap_or(fallback.menu_sounds),
        bluetooth: bluetooth.unwrap_or(fallback.bluetooth),
        clock_12h: clock_12h.unwrap_or(fallback.clock_12h),
        start_fresh: start_fresh.unwrap_or(fallback.start_fresh),
    })
}

fn platform_key(platform: Platform) -> String {
    platform.dir_name().to_ascii_lowercase()
}

fn platform_value(value: &str) -> Option<Platform> {
    Platform::ALL
        .into_iter()
        .find(|p| value.eq_ignore_ascii_case(p.dir_name()))
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn ff_speed_value(value: &str) -> Option<u8> {
    value.parse().ok().filter(|n| FF_SPEEDS.contains(n))
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}

fn flag(value: &str) -> Option<bool> {
    level(value, 1).map(|n| n == 1)
}
