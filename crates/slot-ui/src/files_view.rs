//! The file explorer's screen: the folder's path along the top, a page of
//! its files and folders with their sizes, and what the buttons do along
//! the foot. Light on the Socket backdrop, or dark in slot's own style.

use crate::draw::{Draw, TexId, OUT_W};
use crate::plate::UndoFace;
use crate::power_menu::MENU_INK;
use crate::search::sized;

/// How many lines of the folder show at once.
pub const FILES_ROWS: usize = 8;

pub const FILES_TITLE: usize = 0;
pub const FILES_STATUS: usize = 1;
pub const FILES_LEGEND: usize = 2;
const NAME0: usize = 3;
const SIZE0: usize = NAME0 + FILES_ROWS;
pub const FILES_SLOTS: usize = SIZE0 + FILES_ROWS;

pub fn files_name_slot(i: usize) -> usize {
    NAME0 + i
}

pub fn files_size_slot(i: usize) -> usize {
    SIZE0 + i
}

const EDGE: f32 = 24.0;
const HEAD_Y: f32 = 12.0;
const HEAD_H: u32 = 44;
pub const FILES_ROW_Y: f32 = 64.0;
pub const FILES_PITCH: f32 = 38.0;
const ROW_H: u32 = 34;
const NAME_W: f32 = 520.0;
const SIZE_W: f32 = 120.0;
const LEGEND_Y: f32 = FILES_ROW_Y + FILES_PITCH * FILES_ROWS as f32 + 6.0;
const LEGEND_H: u32 = 30;
const STATUS_W: f32 = 230.0;
const DIM: [u8; 3] = [0x9a, 0x9a, 0xa4];

/// The face for one slot, dark text on light for the Socket style.
pub fn files_face_light(slot: usize, label: &str, lit: bool) -> UndoFace {
    use crate::socket::{inter_fit_face, INK_DARK};
    let grey = [96, 96, 96];
    let ink = if lit { INK_DARK } else { [40, 40, 40] };
    let wide = OUT_W as f32 - 2.0 * EDGE;
    match slot {
        FILES_TITLE => inter_fit_face(label, 20.0, 12.0, HEAD_H, INK_DARK, wide - STATUS_W),
        FILES_STATUS => inter_fit_face(label, 15.0, 11.0, HEAD_H, grey, STATUS_W),
        FILES_LEGEND => inter_fit_face(label, 15.0, 11.0, LEGEND_H, grey, wide),
        s if s < SIZE0 => inter_fit_face(label, 19.0, 12.0, ROW_H, ink, NAME_W),
        _ => inter_fit_face(label, 15.0, 11.0, ROW_H, grey, SIZE_W),
    }
}

/// The face for one slot in slot's own style.
pub fn files_face(slot: usize, label: &str, lit: bool) -> UndoFace {
    let ink = if lit { MENU_INK } else { [214, 214, 222] };
    let wide = OUT_W as f32 - 2.0 * EDGE;
    match slot {
        FILES_TITLE => sized(label, MENU_INK, 22.0, 13.0, HEAD_H, wide - STATUS_W),
        FILES_STATUS => sized(label, DIM, 16.0, 11.0, HEAD_H, STATUS_W),
        FILES_LEGEND => sized(label, DIM, 16.0, 11.0, LEGEND_H, wide),
        s if s < SIZE0 => sized(label, ink, 19.0, 12.0, ROW_H, NAME_W),
        _ => sized(label, DIM, 16.0, 11.0, ROW_H, SIZE_W),
    }
}

pub struct FilesScreen<'a> {
    pub faces: &'a [Option<(TexId, u32, u32)>],
    /// The row the cursor is on, of those shown.
    pub cursor: Option<usize>,
    /// Which rows shown are ticked.
    pub ticked: [bool; FILES_ROWS],
    /// How far a copy or delete has got, 0 to 1, while one runs.
    pub progress: Option<f32>,
    pub light: bool,
    pub backdrop: Option<TexId>,
}

fn rect(out: &mut Vec<Draw>, x: f32, y: f32, w: f32, h: f32, colour: [f32; 4]) {
    out.push(Draw::Rect { x, y, w, h, colour });
}

impl FilesScreen<'_> {
    fn put(&self, out: &mut Vec<Draw>, slot: usize, x: f32, y: f32, h: f32, right: bool) {
        let Some(Some((tex, w, fh))) = self.faces.get(slot).copied() else {
            return;
        };
        let x = if right { x - w as f32 } else { x };
        out.push(Draw::Tex {
            x: x.round(),
            y: (y + (h - fh as f32) / 2.0).round(),
            w: w as f32,
            h: fh as f32,
            tex,
            alpha: 1.0,
        });
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        let w = OUT_W as f32;
        let list_h = FILES_PITCH * FILES_ROWS as f32 + 8.0;
        let (panel, bar, head) = match self.light {
            true => (
                [0.93, 0.93, 0.93, 0.92],
                [0.72, 0.72, 0.72, 1.0],
                [0.86, 0.86, 0.86, 0.92],
            ),
            false => (
                [0.10, 0.10, 0.13, 1.0],
                [0.27, 0.52, 0.92, 1.0],
                [0.14, 0.14, 0.18, 1.0],
            ),
        };
        match (self.light, self.backdrop) {
            (true, Some(tex)) => out.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w,
                h: crate::draw::OUT_H as f32,
                tex,
                alpha: 1.0,
            }),
            _ => rect(
                out,
                0.0,
                0.0,
                w,
                crate::draw::OUT_H as f32,
                [0.07, 0.07, 0.09, 1.0],
            ),
        }
        rect(
            out,
            EDGE - 8.0,
            HEAD_Y,
            w - 2.0 * (EDGE - 8.0),
            HEAD_H as f32,
            head,
        );
        self.put(out, FILES_TITLE, EDGE, HEAD_Y, HEAD_H as f32, false);
        self.put(out, FILES_STATUS, w - EDGE, HEAD_Y, HEAD_H as f32, true);
        rect(
            out,
            EDGE - 8.0,
            FILES_ROW_Y - 4.0,
            w - 2.0 * (EDGE - 8.0),
            list_h,
            panel,
        );
        for i in 0..FILES_ROWS {
            let y = FILES_ROW_Y + FILES_PITCH * i as f32;
            if self.cursor == Some(i) {
                rect(
                    out,
                    EDGE - 4.0,
                    y,
                    w - 2.0 * (EDGE - 4.0),
                    FILES_PITCH - 2.0,
                    bar,
                );
            }
            if self.ticked[i] {
                let tick = match self.light {
                    true => [0.14, 0.64, 0.47, 1.0],
                    false => [0.36, 0.78, 0.55, 1.0],
                };
                let mid = y + (FILES_PITCH - 2.0) / 2.0;
                rect(out, EDGE + 2.0, mid - 6.0, 12.0, 12.0, tick);
            }
            self.put(
                out,
                files_name_slot(i),
                EDGE + 22.0,
                y,
                FILES_PITCH - 2.0,
                false,
            );
            self.put(
                out,
                files_size_slot(i),
                w - EDGE - 6.0,
                y,
                FILES_PITCH - 2.0,
                true,
            );
        }
        match self.progress {
            Some(p) => {
                let (track, fill) = match self.light {
                    true => ([0.8, 0.8, 0.8, 1.0], [0.14, 0.64, 0.47, 1.0]),
                    false => ([0.2, 0.2, 0.24, 1.0], [0.27, 0.52, 0.92, 1.0]),
                };
                let bw = w - 2.0 * EDGE;
                rect(out, EDGE, LEGEND_Y + 4.0, bw, 6.0, track);
                rect(out, EDGE, LEGEND_Y + 4.0, bw * p.clamp(0.0, 1.0), 6.0, fill);
                self.put(
                    out,
                    FILES_LEGEND,
                    EDGE,
                    LEGEND_Y + 8.0,
                    LEGEND_H as f32,
                    false,
                );
            }
            None => self.put(out, FILES_LEGEND, EDGE, LEGEND_Y, LEGEND_H as f32, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_and_legend_end_above_the_split() {
        let bottom = LEGEND_Y + 8.0 + LEGEND_H as f32;
        assert!(bottom <= crate::search::SEARCH_SPLIT, "{bottom}");
    }
}
