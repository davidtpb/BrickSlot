use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::UndoFace;
use crate::power_menu::MENU_INK;
use crate::quick_menu::QUICK_EDGE;
use crate::slot_chrome::{edge, opening};
use crate::text;

pub const SEARCH_HITS: usize = 4;
pub const SEARCH_ROWS: usize = 4;
pub const SEARCH_COLS: usize = 10;

/// Every piece of text on the search screen has a slot: the query, the hits,
/// one per key, and the legend.
pub const SEARCH_QUERY: usize = 0;
pub const SEARCH_LEGEND: usize = 1 + SEARCH_HITS + SEARCH_ROWS * SEARCH_COLS;
pub const SEARCH_SLOTS: usize = SEARCH_LEGEND + 1;

pub fn search_hit_slot(i: usize) -> usize {
    1 + i
}

pub fn search_key_slot(row: usize, col: usize) -> usize {
    1 + SEARCH_HITS + row * SEARCH_COLS + col
}

const DIM_INK: [u8; 3] = [0x9a, 0x9a, 0xa4];

const LINE_H: u32 = 40;
const LINE_PX: f32 = 26.0;
const KEY_H: u32 = 36;
const KEY_PX: f32 = 22.0;
const LEGEND_PX: f32 = 18.0;

const QUERY_Y: f32 = 18.0;
const HITS_Y: f32 = 68.0;
const HIT_PITCH: f32 = 44.0;
const KEYS_Y: f32 = 258.0;
const KEY_PITCH: f32 = 40.0;
const KEY_W: f32 = 64.0;
const KEYS_X: f32 = (OUT_W as f32 - KEY_W * SEARCH_COLS as f32) / 2.0;
const LEGEND_Y: f32 = 430.0;
const BAR_INSET: f32 = 3.0;

fn face(label: &str, colour: [u8; 3], px: f32, h: u32, max_w: f32) -> UndoFace {
    let empty = UndoFace {
        rgba: Vec::new(),
        w: 0,
        h: 0,
    };
    let Some(font) = text::label_font() else {
        return empty;
    };
    if label.is_empty() {
        return empty;
    }
    let layout = text::fit(font, label, max_w, 1, px, px.min(12.0));
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = (set.ceil() as u32 + 8).max(1);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    text::draw_centred(&mut rgba, w, h, &layout, colour);
    UndoFace { rgba, w, h }
}

/// The face for one slot of the search screen, lit where the cursor is.
pub fn search_face(slot: usize, label: &str, lit: bool) -> UndoFace {
    let ink = if lit { MENU_INK } else { DIM_INK };
    let line_w = OUT_W as f32 - 2.0 * QUICK_EDGE;
    match slot {
        SEARCH_QUERY => face(label, MENU_INK, LINE_PX, LINE_H, line_w),
        SEARCH_LEGEND => face(label, DIM_INK, LEGEND_PX, LINE_H, line_w),
        s if s <= SEARCH_HITS => face(label, ink, LINE_PX, LINE_H, line_w),
        _ => face(label, ink, KEY_PX, KEY_H, KEY_W - 6.0),
    }
}

pub struct SearchScreen<'a> {
    pub faces: &'a [Option<(TexId, u32, u32)>],
    pub row: usize,
    pub col: usize,
    pub pick: Option<usize>,
}

impl SearchScreen<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: opening(),
        });
        if let Some(i) = self.pick {
            out.push(Draw::Rect {
                x: 0.0,
                y: HITS_Y + HIT_PITCH * i as f32 + BAR_INSET,
                w: OUT_W as f32,
                h: HIT_PITCH - 2.0 * BAR_INSET,
                colour: edge(),
            });
        }
        out.push(Draw::Rect {
            x: KEYS_X + KEY_W * self.col as f32 + BAR_INSET,
            y: KEYS_Y + KEY_PITCH * self.row as f32 + BAR_INSET,
            w: KEY_W - 2.0 * BAR_INSET,
            h: KEY_PITCH - 2.0 * BAR_INSET,
            colour: edge(),
        });
        let mut put = |slot: usize, x: f32, y: f32, centre_w: Option<f32>, row_h: f32| {
            let Some(Some((tex, w, h))) = self.faces.get(slot).copied() else {
                return;
            };
            let x = match centre_w {
                Some(cw) => x + (cw - w as f32) / 2.0,
                None => x,
            };
            out.push(Draw::Tex {
                x: x.round(),
                y: (y + (row_h - h as f32) / 2.0).round(),
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        };
        put(SEARCH_QUERY, QUICK_EDGE, QUERY_Y, None, LINE_H as f32);
        for i in 0..SEARCH_HITS {
            let y = HITS_Y + HIT_PITCH * i as f32;
            put(search_hit_slot(i), QUICK_EDGE, y, None, HIT_PITCH);
        }
        for row in 0..SEARCH_ROWS {
            for col in 0..SEARCH_COLS {
                let (x, y) = (KEYS_X + KEY_W * col as f32, KEYS_Y + KEY_PITCH * row as f32);
                put(search_key_slot(row, col), x, y, Some(KEY_W), KEY_PITCH);
            }
        }
        put(
            SEARCH_LEGEND,
            0.0,
            LEGEND_Y,
            Some(OUT_W as f32),
            LINE_H as f32,
        );
    }
}
