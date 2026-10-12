use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::{arrows_hint_face, centred_hints, hint_face, UndoFace, HINT_H, LEGEND_GAP};
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD, MENU_PX};
use crate::slot_chrome::{edge, opening};
use crate::text;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QuickRow {
    FastForward,
    FastForwardSound,
    Screen,
    Rumble,
    DateTime,
    About,
    GbaShader,
    GbShader,
    ColourCorrection,
    Game,
    EjectSave,
    Turbo,
    Rewind,
    GbPalettes,
    Lights,
    LightMode,
    LightColour,
    LightEffect,
    LightLevel,
    Brightness,
    Warmth,
    Network,
    Wifi,
    Syncthing,
    Achievements,
    SaveFormat,
    AutoTime,
    Search,
    Performance,
    Labels,
    FileTransfer,
    Look,
    Tint,
    MenuSounds,
    Bluetooth,
    BtDevices,
    EmulatorSettings,
    DeviceSync,
    ClockFormat,
    JoinNetwork,
    StartFresh,
    WifiStatus,
}

impl QuickRow {
    pub const ALL: [QuickRow; 42] = [
        QuickRow::FastForward,
        QuickRow::FastForwardSound,
        QuickRow::Screen,
        QuickRow::Rumble,
        QuickRow::DateTime,
        QuickRow::About,
        QuickRow::GbaShader,
        QuickRow::GbShader,
        QuickRow::ColourCorrection,
        QuickRow::Game,
        QuickRow::EjectSave,
        QuickRow::Turbo,
        QuickRow::Rewind,
        QuickRow::GbPalettes,
        QuickRow::Lights,
        QuickRow::LightMode,
        QuickRow::LightColour,
        QuickRow::LightEffect,
        QuickRow::LightLevel,
        QuickRow::Brightness,
        QuickRow::Warmth,
        QuickRow::Network,
        QuickRow::Wifi,
        QuickRow::Syncthing,
        QuickRow::Achievements,
        QuickRow::SaveFormat,
        QuickRow::AutoTime,
        QuickRow::Search,
        QuickRow::Performance,
        QuickRow::Labels,
        QuickRow::FileTransfer,
        QuickRow::Look,
        QuickRow::Tint,
        QuickRow::MenuSounds,
        QuickRow::Bluetooth,
        QuickRow::BtDevices,
        QuickRow::EmulatorSettings,
        QuickRow::DeviceSync,
        QuickRow::ClockFormat,
        QuickRow::JoinNetwork,
        QuickRow::StartFresh,
        QuickRow::WifiStatus,
    ];

    pub const MAIN: [QuickRow; 7] = [
        QuickRow::Search,
        QuickRow::Screen,
        QuickRow::Game,
        QuickRow::Lights,
        QuickRow::Network,
        QuickRow::DateTime,
        QuickRow::About,
    ];

    pub const SCREEN: [QuickRow; 9] = [
        QuickRow::Brightness,
        QuickRow::Warmth,
        QuickRow::Look,
        QuickRow::Tint,
        QuickRow::ClockFormat,
        QuickRow::GbaShader,
        QuickRow::GbShader,
        QuickRow::ColourCorrection,
        QuickRow::GbPalettes,
    ];

    pub const GAME: [QuickRow; 11] = [
        QuickRow::FastForward,
        QuickRow::FastForwardSound,
        QuickRow::Rewind,
        QuickRow::Turbo,
        QuickRow::Rumble,
        QuickRow::EjectSave,
        QuickRow::StartFresh,
        QuickRow::SaveFormat,
        QuickRow::Performance,
        QuickRow::EmulatorSettings,
        QuickRow::MenuSounds,
    ];

    pub const LIGHTS: [QuickRow; 4] = [
        QuickRow::LightMode,
        QuickRow::LightColour,
        QuickRow::LightEffect,
        QuickRow::LightLevel,
    ];

    pub const NETWORK: [QuickRow; 11] = [
        QuickRow::Wifi,
        QuickRow::WifiStatus,
        QuickRow::JoinNetwork,
        QuickRow::DeviceSync,
        QuickRow::Bluetooth,
        QuickRow::BtDevices,
        QuickRow::AutoTime,
        QuickRow::FileTransfer,
        QuickRow::Syncthing,
        QuickRow::Achievements,
        QuickRow::Labels,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn page(self) -> &'static [QuickRow] {
        match self {
            QuickRow::Brightness
            | QuickRow::Warmth
            | QuickRow::GbaShader
            | QuickRow::GbShader
            | QuickRow::ColourCorrection
            | QuickRow::GbPalettes
            | QuickRow::Look
            | QuickRow::Tint
            | QuickRow::ClockFormat => &QuickRow::SCREEN,
            QuickRow::LightMode
            | QuickRow::LightColour
            | QuickRow::LightEffect
            | QuickRow::LightLevel => &QuickRow::LIGHTS,
            QuickRow::Wifi
            | QuickRow::AutoTime
            | QuickRow::Syncthing
            | QuickRow::Achievements
            | QuickRow::Labels
            | QuickRow::FileTransfer
            | QuickRow::JoinNetwork
            | QuickRow::WifiStatus
            | QuickRow::Bluetooth
            | QuickRow::DeviceSync
            | QuickRow::BtDevices => &QuickRow::NETWORK,
            QuickRow::FastForward
            | QuickRow::FastForwardSound
            | QuickRow::Rewind
            | QuickRow::Turbo
            | QuickRow::Rumble
            | QuickRow::EjectSave
            | QuickRow::SaveFormat
            | QuickRow::Performance
            | QuickRow::MenuSounds
            | QuickRow::EmulatorSettings
            | QuickRow::StartFresh => &QuickRow::GAME,
            _ => &QuickRow::MAIN,
        }
    }

    pub fn parent(self) -> Option<QuickRow> {
        match self.page()[0] {
            QuickRow::Brightness => Some(QuickRow::Screen),
            QuickRow::FastForward => Some(QuickRow::Game),
            QuickRow::LightMode => Some(QuickRow::Lights),
            QuickRow::Wifi => Some(QuickRow::Network),
            _ => None,
        }
    }

    pub fn child(self) -> Option<QuickRow> {
        match self {
            QuickRow::Screen => Some(QuickRow::SCREEN[0]),
            QuickRow::Game => Some(QuickRow::GAME[0]),
            QuickRow::Lights => Some(QuickRow::LIGHTS[0]),
            QuickRow::Network => Some(QuickRow::NETWORK[0]),
            _ => None,
        }
    }

    pub fn position(self) -> usize {
        self.page().iter().position(|r| *r == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            QuickRow::FastForward => "Fast Forward",
            QuickRow::FastForwardSound => "Fast Forward Sound",
            QuickRow::Screen => "Screen",
            QuickRow::Rumble => "Rumble",
            QuickRow::DateTime => "Date & Time",
            QuickRow::About => "About",
            QuickRow::GbaShader => "GBA Shader",
            QuickRow::GbShader => "GB / GBC Shader",
            QuickRow::ColourCorrection => "Color Correction",
            QuickRow::Game => "Gameplay",
            QuickRow::EjectSave => "Auto Save on Eject",
            QuickRow::Turbo => "Turbo Buttons",
            QuickRow::Rewind => "Rewind",
            QuickRow::GbPalettes => "GB Palettes",
            QuickRow::Lights => "Lights",
            QuickRow::LightMode => "Lights",
            QuickRow::LightColour => "Light Color",
            QuickRow::LightEffect => "Light Effect",
            QuickRow::LightLevel => "Light Brightness",
            QuickRow::Brightness => "Brightness",
            QuickRow::Warmth => "Warmth",
            QuickRow::Network => "Network",
            QuickRow::Wifi => "Wi-Fi",
            QuickRow::Syncthing => "Syncthing",
            QuickRow::Achievements => "RetroAchievements",
            QuickRow::SaveFormat => "Save Format",
            QuickRow::AutoTime => "Set Time Automatically",
            QuickRow::Search => "Search Games",
            QuickRow::Performance => "Performance",
            QuickRow::Look => "Style",
            QuickRow::Tint => "Theme",
            QuickRow::MenuSounds => "Menu Sounds",
            QuickRow::ClockFormat => "Clock",
            QuickRow::Labels => "Download Labels",
            QuickRow::JoinNetwork => "Join Network",
            QuickRow::StartFresh => "Start Fresh",
            QuickRow::WifiStatus => "Wi-Fi Status",
            QuickRow::Bluetooth => "Bluetooth",
            QuickRow::BtDevices => "Bluetooth Devices",
            QuickRow::EmulatorSettings => "Emulator Settings",
            QuickRow::DeviceSync => "Device Sync",
            QuickRow::FileTransfer => "File Transfer",
        }
    }

    pub fn opens(self) -> bool {
        matches!(
            self,
            QuickRow::DateTime
                | QuickRow::About
                | QuickRow::Screen
                | QuickRow::Game
                | QuickRow::Lights
                | QuickRow::Network
                | QuickRow::Search
                | QuickRow::Labels
                | QuickRow::JoinNetwork
                | QuickRow::WifiStatus
                | QuickRow::BtDevices
                | QuickRow::EmulatorSettings
                | QuickRow::DeviceSync
        )
    }

    pub fn up(self) -> QuickRow {
        let page = self.page();
        page[(self.position() + page.len() - 1) % page.len()]
    }

    pub fn down(self) -> QuickRow {
        let page = self.page();
        page[(self.position() + 1) % page.len()]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum QuickValue {
    Speed2,
    Speed3,
    Speed4,
    Speed6,
    On,
    Off,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
    Battery,
    Colour,
    White,
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
    Static,
    Breathe,
    Blink,
    Rainbow,
    Low,
    Medium,
    High,
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
    Level5,
    Level6,
    Level7,
    Level8,
    Level9,
    RetroArch,
    Slot,
    RetroArchCompressed,
    RainbowWave,
    Fire,
    Ocean,
    Aurora,
    Sunset,
    Candy,
    Police,
    Heartbeat,
    CpuStock,
    CpuSaver,
    CpuPerformance,
    LookClassic,
    LookSocket,
    TintDefault,
    TintPastel,
    TintSepia,
    Hour12,
    Hour24,
}

impl QuickValue {
    pub const LEVELS: [QuickValue; 10] = [
        QuickValue::Level0,
        QuickValue::Level1,
        QuickValue::Level2,
        QuickValue::Level3,
        QuickValue::Level4,
        QuickValue::Level5,
        QuickValue::Level6,
        QuickValue::Level7,
        QuickValue::Level8,
        QuickValue::Level9,
    ];

    pub const ALL: [QuickValue; 59] = [
        QuickValue::Speed2,
        QuickValue::Speed3,
        QuickValue::Speed4,
        QuickValue::Speed6,
        QuickValue::On,
        QuickValue::Off,
        QuickValue::Lcd3x,
        QuickValue::Grid,
        QuickValue::Dot,
        QuickValue::Simpletex,
        QuickValue::Battery,
        QuickValue::Colour,
        QuickValue::White,
        QuickValue::Red,
        QuickValue::Orange,
        QuickValue::Yellow,
        QuickValue::Green,
        QuickValue::Cyan,
        QuickValue::Blue,
        QuickValue::Purple,
        QuickValue::Pink,
        QuickValue::Static,
        QuickValue::Breathe,
        QuickValue::Blink,
        QuickValue::Rainbow,
        QuickValue::Low,
        QuickValue::Medium,
        QuickValue::High,
        QuickValue::Level0,
        QuickValue::Level1,
        QuickValue::Level2,
        QuickValue::Level3,
        QuickValue::Level4,
        QuickValue::Level5,
        QuickValue::Level6,
        QuickValue::Level7,
        QuickValue::Level8,
        QuickValue::Level9,
        QuickValue::RetroArch,
        QuickValue::Slot,
        QuickValue::RetroArchCompressed,
        QuickValue::RainbowWave,
        QuickValue::Fire,
        QuickValue::Ocean,
        QuickValue::Aurora,
        QuickValue::Sunset,
        QuickValue::Candy,
        QuickValue::Police,
        QuickValue::Heartbeat,
        QuickValue::CpuStock,
        QuickValue::CpuSaver,
        QuickValue::CpuPerformance,
        QuickValue::LookClassic,
        QuickValue::LookSocket,
        QuickValue::TintDefault,
        QuickValue::TintPastel,
        QuickValue::TintSepia,
        QuickValue::Hour12,
        QuickValue::Hour24,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            QuickValue::Speed2 => "2×",
            QuickValue::Speed3 => "3×",
            QuickValue::Speed4 => "4×",
            QuickValue::Speed6 => "6×",
            QuickValue::On => "On",
            QuickValue::Off => "Off",
            QuickValue::Lcd3x => "LCD3x",
            QuickValue::Grid => "Grid",
            QuickValue::Dot => "Dot",
            QuickValue::Simpletex => "Simpletex",
            QuickValue::Battery => "Battery",
            QuickValue::Colour => "Color",
            QuickValue::White => "White",
            QuickValue::Red => "Red",
            QuickValue::Orange => "Orange",
            QuickValue::Yellow => "Yellow",
            QuickValue::Green => "Green",
            QuickValue::Cyan => "Cyan",
            QuickValue::Blue => "Blue",
            QuickValue::Purple => "Purple",
            QuickValue::Pink => "Pink",
            QuickValue::Static => "Static",
            QuickValue::Breathe => "Breathe",
            QuickValue::Blink => "Blink",
            QuickValue::Rainbow => "Rainbow",
            QuickValue::Low => "Low",
            QuickValue::Medium => "Medium",
            QuickValue::High => "High",
            QuickValue::Level0 => "0",
            QuickValue::Level1 => "1",
            QuickValue::Level2 => "2",
            QuickValue::Level3 => "3",
            QuickValue::Level4 => "4",
            QuickValue::Level5 => "5",
            QuickValue::Level6 => "6",
            QuickValue::Level7 => "7",
            QuickValue::Level8 => "8",
            QuickValue::Level9 => "9",
            QuickValue::RetroArch => "RetroArch (.srm)",
            QuickValue::Slot => "slot (.sav)",
            QuickValue::RetroArchCompressed => "RetroArch, compressed",
            QuickValue::RainbowWave => "Rainbow Wave",
            QuickValue::Fire => "Fire",
            QuickValue::Ocean => "Ocean",
            QuickValue::Aurora => "Aurora",
            QuickValue::Sunset => "Sunset",
            QuickValue::Candy => "Candy",
            QuickValue::Police => "Police",
            QuickValue::Heartbeat => "Heartbeat",
            QuickValue::CpuStock => "Default",
            QuickValue::CpuSaver => "Battery Saver",
            QuickValue::CpuPerformance => "Performance",
            QuickValue::LookClassic => "Slot",
            QuickValue::LookSocket => "Socket",
            QuickValue::TintDefault => "Default",
            QuickValue::TintPastel => "Pastel",
            QuickValue::TintSepia => "Sepia",
            QuickValue::Hour12 => "12-hour",
            QuickValue::Hour24 => "24-hour",
        }
    }

    pub fn speed(frames: u8) -> Option<QuickValue> {
        match frames {
            2 => Some(QuickValue::Speed2),
            3 => Some(QuickValue::Speed3),
            4 => Some(QuickValue::Speed4),
            6 => Some(QuickValue::Speed6),
            _ => None,
        }
    }

    pub fn level(n: u8) -> QuickValue {
        QuickValue::LEVELS[usize::from(n.min(9))]
    }

    pub fn flag(on: bool) -> QuickValue {
        if on {
            QuickValue::On
        } else {
            QuickValue::Off
        }
    }
}

pub const QUICK_PITCH: f32 = 52.0;
pub const QUICK_TOP: f32 = (OUT_H as f32 - QUICK_PITCH * QuickRow::MAIN.len() as f32) / 2.0;
pub const QUICK_EDGE: f32 = 32.0;
/// The most rows a page shows at once. Below the legend split the frame is
/// drawn differently, so a longer page scrolls instead of running into it.
pub const QUICK_VISIBLE: usize = QuickRow::MAIN.len();
const BAR_INSET: f32 = 4.0;
const TYPE_DROP: f32 = 4.0;
const CARET_GAP: f32 = 14.0;
const CARET_PX: f32 = 24.0;
const LEGEND_Y: f32 = 427.0;
pub const QUICK_SPLIT: f32 = LEGEND_Y - 2.0;
const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];

pub fn quick_label_face(row: QuickRow) -> UndoFace {
    quick_text_face(row.label(), MENU_INK)
}

pub fn quick_value_face(text: &str, lit: bool) -> UndoFace {
    quick_text_face(text, if lit { MENU_INK } else { DIM_INK })
}

fn quick_text_face(label: &str, colour: [u8; 3]) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(font, label, OUT_W as f32, 1, MENU_PX, MENU_PX);
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = set.ceil() as u32 + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * MENU_H * 4) as usize];
    text::draw_centred(&mut rgba, w, MENU_H, &layout, colour);
    UndoFace { rgba, w, h: MENU_H }
}

pub fn quick_caret_face(right: bool) -> UndoFace {
    let glyph = if right { '\u{f0da}' } else { '\u{f0d9}' };
    let (Some(symbols), Some(label)) = (crate::icon::symbols_font(), text::label_font()) else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let (m, cov) = symbols.rasterize(glyph, CARET_PX);
    let (w, h) = (m.width as u32, MENU_H);
    let centre = match label.horizontal_line_metrics(MENU_PX) {
        Some(v) => {
            (h as f32 - v.new_line_size) / 2.0 + v.ascent
                - label.metrics('H', MENU_PX).height as f32 / 2.0
        }
        None => h as f32 / 2.0,
    };
    let top = (centre - m.height as f32 / 2.0).round() as i32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for gy in 0..m.height {
        let y = top + gy as i32;
        if y < 0 || y >= h as i32 {
            continue;
        }
        for gx in 0..m.width {
            let at = ((y as u32 * w + gx as u32) * 4) as usize;
            let a = cov[gy * m.width + gx];
            rgba[at..at + 4].copy_from_slice(&[MENU_INK[0], MENU_INK[1], MENU_INK[2], a]);
        }
    }
    UndoFace { rgba, w, h }
}

pub fn quick_legend_faces() -> [UndoFace; 3] {
    [
        hint_face("B", "Back"),
        arrows_hint_face("Change"),
        hint_face("A", "Open"),
    ]
}

pub struct QuickMenuFaces {
    pub labels: Vec<(TexId, u32, u32)>,
    pub values: Vec<[(TexId, u32, u32); 2]>,
    pub carets: [(TexId, u32, u32); 2],
    pub legend: [(TexId, u32); 3],
}

pub struct QuickMenu<'a> {
    pub row: QuickRow,
    pub values: [Option<QuickValue>; QuickRow::ALL.len()],
    pub clock: Option<[(TexId, u32, u32); 2]>,
    pub faces: Option<&'a QuickMenuFaces>,
}

impl QuickMenu<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: opening(),
        });
        out.push(Draw::Rect {
            x: 0.0,
            y: row_top(self.row, window(self.row).0) + BAR_INSET,
            w: OUT_W as f32,
            h: QUICK_PITCH - 2.0 * BAR_INSET,
            colour: edge(),
        });
        let Some(faces) = self.faces else {
            return;
        };
        let (right, pad) = (OUT_W as f32 - QUICK_EDGE, MENU_PAD as f32);
        let (first, shown) = window(self.row);
        for &row in &self.row.page()[first..first + shown] {
            let y = row_top(row, first) + TYPE_DROP;
            let lit = row == self.row;
            if let Some(&(tex, w, h)) = faces.labels.get(row.index()) {
                push(out, tex, QUICK_EDGE - pad, y, w, h);
            }
            let value = match row {
                QuickRow::DateTime => self.clock.map(|c| c[lit as usize]),
                _ => self.values[row.index()]
                    .and_then(|v| faces.values.get(v.index()))
                    .map(|v| v[lit as usize]),
            };
            let Some((tex, w, h)) = value else {
                continue;
            };
            if !lit || row.opens() {
                push(out, tex, right + pad - w as f32, y, w, h);
                continue;
            }
            let [(left_tex, lw, lh), (right_tex, rw, rh)] = faces.carets;
            let rx = right - rw as f32;
            push(out, right_tex, rx, y, rw, rh);
            let vx = rx - CARET_GAP + pad - w as f32;
            push(out, tex, vx, y, w, h);
            push(out, left_tex, vx + pad - CARET_GAP - lw as f32, y, lw, lh);
        }
        let [back, change, open] = faces.legend;
        let other = if self.row.opens() { open } else { change };
        for (tex, w, x) in centred_hints(&[back, other], LEGEND_GAP) {
            push(out, tex, x, LEGEND_Y, w, HINT_H);
        }
    }
}

/// The first row of the selected row's page on screen and how many show,
/// scrolled just far enough that the selection is in view.
pub fn window(row: QuickRow) -> (usize, usize) {
    let len = row.page().len();
    let shown = len.min(QUICK_VISIBLE);
    let first = row.position().saturating_sub(shown - 1).min(len - shown);
    (first, shown)
}

fn row_top(row: QuickRow, first: usize) -> f32 {
    let shown = row.page().len().min(QUICK_VISIBLE);
    let top = (OUT_H as f32 - QUICK_PITCH * shown as f32) / 2.0;
    top + QUICK_PITCH * (row.position() - first) as f32
}

fn push(out: &mut Vec<Draw>, tex: TexId, x: f32, y: f32, w: u32, h: u32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}
