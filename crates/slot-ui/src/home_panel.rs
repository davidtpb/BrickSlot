use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::UndoFace;
use crate::power_menu::MENU_INK;
use crate::search::sized;
use crate::slot_chrome::{edge, opening, recess};

/// The buttons along the top of the Home panel, left to right.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HomeTile {
    Screenshot,
    Sound,
    Rumble,
    Wifi,
    Lights,
    Power,
}

impl HomeTile {
    pub const ALL: [HomeTile; 6] = [
        HomeTile::Screenshot,
        HomeTile::Sound,
        HomeTile::Rumble,
        HomeTile::Wifi,
        HomeTile::Lights,
        HomeTile::Power,
    ];

    pub fn name(self) -> &'static str {
        match self {
            HomeTile::Screenshot => "Screenshot",
            HomeTile::Sound => "Sound",
            HomeTile::Rumble => "Rumble",
            HomeTile::Wifi => "Wi-Fi",
            HomeTile::Lights => "Lights",
            HomeTile::Power => "Power",
        }
    }
}

/// What the cursor rests on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum HomeFocus {
    Tile(HomeTile),
    Brightness,
    Volume,
}

pub const HOME_STATS: usize = 4;

/// Every piece of text on the panel has a slot.
pub const HOME_TITLE: usize = 0;
pub const HOME_CLOCK: usize = 1;
const TILE_NAME0: usize = 2;
const TILE_STATE0: usize = TILE_NAME0 + HomeTile::ALL.len();
const SLIDER_NAME0: usize = TILE_STATE0 + HomeTile::ALL.len();
const SLIDER_VALUE0: usize = SLIDER_NAME0 + 2;
const STAT_NAME0: usize = SLIDER_VALUE0 + 2;
const STAT_VALUE0: usize = STAT_NAME0 + HOME_STATS;
pub const HOME_LEGEND: usize = STAT_VALUE0 + HOME_STATS;
pub const HOME_SLOTS: usize = HOME_LEGEND + 1;

pub fn home_tile_name_slot(i: usize) -> usize {
    TILE_NAME0 + i
}

pub fn home_tile_state_slot(i: usize) -> usize {
    TILE_STATE0 + i
}

/// 0 is brightness, 1 is volume.
pub fn home_slider_name_slot(i: usize) -> usize {
    SLIDER_NAME0 + i
}

pub fn home_slider_value_slot(i: usize) -> usize {
    SLIDER_VALUE0 + i
}

pub fn home_stat_name_slot(i: usize) -> usize {
    STAT_NAME0 + i
}

pub fn home_stat_value_slot(i: usize) -> usize {
    STAT_VALUE0 + i
}

const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];
pub const HOME_ACCENT: [f32; 4] = [0.27, 0.52, 0.92, 1.0];
const SHADE: [f32; 4] = [0.0, 0.0, 0.0, 0.6];

pub const CARD_X: f32 = 48.0;
pub const CARD_Y: f32 = 26.0;
pub const CARD_W: f32 = OUT_W as f32 - 2.0 * CARD_X;
pub const CARD_H: f32 = OUT_H as f32 - 2.0 * CARD_Y;
const INSET: f32 = 20.0;

const TITLE_Y: f32 = CARD_Y + 8.0;
const TITLE_H: u32 = 40;

pub const TILE_W: f32 = 92.0;
pub const TILE_H: f32 = 84.0;
const TILE_GAP: f32 = 10.0;
pub const TILES_Y: f32 = CARD_Y + 56.0;
pub const TILES_X: f32 = CARD_X
    + (CARD_W - TILE_W * HomeTile::ALL.len() as f32 - TILE_GAP * (HomeTile::ALL.len() - 1) as f32)
        / 2.0;
const TILE_NAME_H: u32 = 34;
const TILE_STATE_H: u32 = 30;

pub const SLIDERS_Y: f32 = TILES_Y + TILE_H + 14.0;
pub const SLIDER_PITCH: f32 = 48.0;
const SLIDER_NAME_W: f32 = 150.0;
pub const TRACK_X: f32 = CARD_X + INSET + SLIDER_NAME_W;
pub const TRACK_W: f32 = 330.0;
const TRACK_H: f32 = 10.0;
const SLIDER_VALUE_X: f32 = TRACK_X + TRACK_W + 12.0;
const SLIDER_VALUE_W: f32 = CARD_X + CARD_W - INSET - SLIDER_VALUE_X;

pub const STATS_Y: f32 = SLIDERS_Y + 2.0 * SLIDER_PITCH + 10.0;
const STAT_W: f32 = 140.0;
const STAT_H: f32 = 78.0;
const STAT_GAP: f32 = 12.0;
const STATS_X: f32 =
    CARD_X + (CARD_W - STAT_W * HOME_STATS as f32 - STAT_GAP * (HOME_STATS - 1) as f32) / 2.0;
const STAT_NAME_H: u32 = 28;
const STAT_VALUE_H: u32 = 36;

const LEGEND_Y: f32 = STATS_Y + STAT_H + 6.0;
const LEGEND_H: u32 = 36;

const FRAME: f32 = 3.0;

/// Where a taller screen opens the panel up in the Socket style: below the
/// legend, across the card's plain foot.
pub const HOME_SPLIT: f32 = 430.0;

/// The face for one slot of the panel in the Socket style: dark Inter on
/// the light card.
pub fn home_face_light(slot: usize, label: &str, lit: bool) -> UndoFace {
    use crate::socket::{inter_fit_face, INK_DARK};
    let ink = if lit { INK_DARK } else { [96, 96, 96] };
    let card_w = CARD_W - 2.0 * INSET;
    let (px, min, h, w, ink) = match slot {
        HOME_TITLE => (24.0, 15.0, TITLE_H, card_w - 120.0, INK_DARK),
        HOME_CLOCK => (22.0, 15.0, TITLE_H, 110.0, INK_DARK),
        HOME_LEGEND => (16.0, 11.0, LEGEND_H, card_w, [96, 96, 96]),
        s if s < TILE_STATE0 => (17.0, 11.0, TILE_NAME_H, TILE_W - 8.0, INK_DARK),
        s if s < SLIDER_NAME0 => (15.0, 10.0, TILE_STATE_H, TILE_W - 8.0, ink),
        s if s < SLIDER_VALUE0 => (19.0, 13.0, 40, SLIDER_NAME_W - 10.0, ink),
        s if s < STAT_NAME0 => (18.0, 12.0, 40, SLIDER_VALUE_W, INK_DARK),
        s if s < STAT_VALUE0 => (14.0, 10.0, STAT_NAME_H, STAT_W - 8.0, [96, 96, 96]),
        _ => (21.0, 12.0, STAT_VALUE_H, STAT_W - 8.0, INK_DARK),
    };
    inter_fit_face(label, px, min, h, ink, w)
}

/// The face for one slot of the panel, lit where it reads as on.
pub fn home_face(slot: usize, label: &str, lit: bool) -> UndoFace {
    let ink = if lit { MENU_INK } else { DIM_INK };
    let card_w = CARD_W - 2.0 * INSET;
    match slot {
        HOME_TITLE => sized(label, MENU_INK, 26.0, 16.0, TITLE_H, card_w - 120.0),
        HOME_CLOCK => sized(label, MENU_INK, 24.0, 16.0, TITLE_H, 110.0),
        HOME_LEGEND => sized(label, DIM_INK, 18.0, 12.0, LEGEND_H, card_w),
        s if s < TILE_STATE0 => sized(label, MENU_INK, 20.0, 12.0, TILE_NAME_H, TILE_W - 8.0),
        s if s < SLIDER_NAME0 => sized(label, ink, 17.0, 11.0, TILE_STATE_H, TILE_W - 8.0),
        s if s < SLIDER_VALUE0 => sized(label, ink, 22.0, 14.0, 40, SLIDER_NAME_W - 10.0),
        s if s < STAT_NAME0 => sized(label, ink, 20.0, 12.0, 40, SLIDER_VALUE_W),
        s if s < STAT_VALUE0 => sized(label, DIM_INK, 16.0, 11.0, STAT_NAME_H, STAT_W - 8.0),
        _ => sized(label, MENU_INK, 24.0, 12.0, STAT_VALUE_H, STAT_W - 8.0),
    }
}

/// Where the cursor is on the Home panel: the row of tiles, then the two
/// sliders. The tile last stood on is kept when the cursor leaves the row.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct HomePanel {
    row: usize,
    col: usize,
}

const ROWS: usize = 3;

impl HomePanel {
    pub fn new() -> HomePanel {
        HomePanel::default()
    }

    pub fn focus(&self) -> HomeFocus {
        match self.row {
            0 => HomeFocus::Tile(HomeTile::ALL[self.col]),
            1 => HomeFocus::Brightness,
            _ => HomeFocus::Volume,
        }
    }

    pub fn up(&mut self) {
        self.row = (self.row + ROWS - 1) % ROWS;
    }

    pub fn down(&mut self) {
        self.row = (self.row + 1) % ROWS;
    }

    /// Moves along the row of tiles, wrapping. Off the tiles it does nothing
    /// and says so, since left and right then move a slider instead.
    pub fn step(&mut self, right: bool) -> bool {
        if self.row != 0 {
            return false;
        }
        let n = HomeTile::ALL.len();
        self.col = match right {
            true => (self.col + 1) % n,
            false => (self.col + n - 1) % n,
        };
        true
    }

    pub fn col(&self) -> usize {
        self.col
    }
}

pub fn tile_rect(i: usize) -> (f32, f32, f32, f32) {
    (
        TILES_X + (TILE_W + TILE_GAP) * i as f32,
        TILES_Y,
        TILE_W,
        TILE_H,
    )
}

pub fn track_rect(i: usize) -> (f32, f32, f32, f32) {
    let y = SLIDERS_Y + SLIDER_PITCH * i as f32 + (SLIDER_PITCH - TRACK_H) / 2.0;
    (TRACK_X, y, TRACK_W, TRACK_H)
}

pub fn stat_rect(i: usize) -> (f32, f32, f32, f32) {
    (
        STATS_X + (STAT_W + STAT_GAP) * i as f32,
        STATS_Y,
        STAT_W,
        STAT_H,
    )
}

pub struct HomeScreen<'a> {
    pub faces: &'a [Option<(TexId, u32, u32)>],
    pub panel: HomePanel,
    /// Which tiles read as switched on.
    pub on: [bool; 6],
    /// Brightness and volume, each from 0 to 1.
    pub levels: [f32; 2],
    /// Drawn light, for the Socket style.
    pub light: bool,
}

struct Palette {
    card: [f32; 4],
    tile: [f32; 4],
    row: [f32; 4],
    on: [f32; 4],
    frame: [f32; 4],
}

pub const SOCKET_ON: [f32; 4] = [0.14, 0.64, 0.47, 1.0];

fn palette(light: bool) -> Palette {
    match light {
        true => Palette {
            card: [0.86, 0.86, 0.86, 1.0],
            tile: [0.93, 0.93, 0.93, 1.0],
            row: [0.74, 0.74, 0.74, 1.0],
            on: SOCKET_ON,
            frame: [0.12, 0.12, 0.12, 1.0],
        },
        false => Palette {
            card: opening(),
            tile: recess(),
            row: edge(),
            on: HOME_ACCENT,
            frame: [
                MENU_INK[0] as f32 / 255.0,
                MENU_INK[1] as f32 / 255.0,
                MENU_INK[2] as f32 / 255.0,
                1.0,
            ],
        },
    }
}

fn rect(out: &mut Vec<Draw>, (x, y, w, h): (f32, f32, f32, f32), colour: [f32; 4]) {
    out.push(Draw::Rect { x, y, w, h, colour });
}

fn frame(out: &mut Vec<Draw>, (x, y, w, h): (f32, f32, f32, f32), ink: [f32; 4]) {
    let (x, y, w, h) = (x - FRAME, y - FRAME, w + 2.0 * FRAME, h + 2.0 * FRAME);
    rect(out, (x, y, w, FRAME), ink);
    rect(out, (x, y + h - FRAME, w, FRAME), ink);
    rect(out, (x, y, FRAME, h), ink);
    rect(out, (x + w - FRAME, y, FRAME, h), ink);
}

impl HomeScreen<'_> {
    /// Puts a slot's face centred in the box, or left in it when `left`.
    fn put(
        &self,
        out: &mut Vec<Draw>,
        slot: usize,
        (x, y, w, h): (f32, f32, f32, f32),
        left: bool,
    ) {
        let Some(Some((tex, fw, fh))) = self.faces.get(slot).copied() else {
            return;
        };
        let (fw, fh) = (fw as f32, fh as f32);
        let x = match left {
            true => x,
            false => x + (w - fw) / 2.0,
        };
        out.push(Draw::Tex {
            x: x.round(),
            y: (y + (h - fh) / 2.0).round(),
            w: fw,
            h: fh,
            tex,
            alpha: 1.0,
        });
    }

    fn put_right(&self, out: &mut Vec<Draw>, slot: usize, right: f32, y: f32, h: f32) {
        let Some(Some((_, fw, _))) = self.faces.get(slot).copied() else {
            return;
        };
        self.put(out, slot, (right - fw as f32, y, fw as f32, h), true);
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        let pal = palette(self.light);
        rect(out, (0.0, 0.0, OUT_W as f32, OUT_H as f32), SHADE);
        rect(out, (CARD_X, CARD_Y, CARD_W, CARD_H), pal.card);

        let title = (
            CARD_X + INSET,
            TITLE_Y,
            CARD_W - 2.0 * INSET,
            TITLE_H as f32,
        );
        self.put(out, HOME_TITLE, title, true);
        self.put_right(
            out,
            HOME_CLOCK,
            CARD_X + CARD_W - INSET,
            TITLE_Y,
            TITLE_H as f32,
        );

        let focus = self.panel.focus();
        for (i, tile) in HomeTile::ALL.iter().enumerate() {
            let r = tile_rect(i);
            let fill = if self.on[i] { pal.on } else { pal.tile };
            rect(out, r, fill);
            if focus == HomeFocus::Tile(*tile) {
                frame(out, r, pal.frame);
            }
            let (x, y, w, _) = r;
            self.put(
                out,
                home_tile_name_slot(i),
                (x, y + 8.0, w, TILE_NAME_H as f32),
                false,
            );
            self.put(
                out,
                home_tile_state_slot(i),
                (
                    x,
                    y + TILE_H - TILE_STATE_H as f32 - 8.0,
                    w,
                    TILE_STATE_H as f32,
                ),
                false,
            );
        }

        for i in 0..2 {
            let y = SLIDERS_Y + SLIDER_PITCH * i as f32;
            let row_focus = [HomeFocus::Brightness, HomeFocus::Volume][i];
            if focus == row_focus {
                rect(
                    out,
                    (CARD_X + 6.0, y + 2.0, CARD_W - 12.0, SLIDER_PITCH - 4.0),
                    pal.row,
                );
            }
            self.put(
                out,
                home_slider_name_slot(i),
                (CARD_X + INSET, y, SLIDER_NAME_W, SLIDER_PITCH),
                true,
            );
            let track = track_rect(i);
            rect(out, track, pal.tile);
            let level = self.levels[i].clamp(0.0, 1.0);
            if level > 0.0 {
                let (x, ty, w, h) = track;
                rect(out, (x, ty, (w * level).round(), h), pal.on);
            }
            self.put(
                out,
                home_slider_value_slot(i),
                (SLIDER_VALUE_X, y, SLIDER_VALUE_W, SLIDER_PITCH),
                false,
            );
        }

        for i in 0..HOME_STATS {
            let r = stat_rect(i);
            rect(out, r, pal.tile);
            let (x, y, w, _) = r;
            self.put(
                out,
                home_stat_name_slot(i),
                (x, y + 6.0, w, STAT_NAME_H as f32),
                false,
            );
            self.put(
                out,
                home_stat_value_slot(i),
                (
                    x,
                    y + STAT_H - STAT_VALUE_H as f32 - 6.0,
                    w,
                    STAT_VALUE_H as f32,
                ),
                false,
            );
        }

        self.put(
            out,
            HOME_LEGEND,
            (CARD_X, LEGEND_Y, CARD_W, LEGEND_H as f32),
            false,
        );
    }
}
