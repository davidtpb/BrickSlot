//! Customize, after Socket's: the selected cart swaying on the left and a
//! panel on the right to pick its material and colour, seen on the cart as
//! they change.

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_store::{Outline, ShellChoice, ShellFinish};

use crate::socket_menu::Chrome;

/// What a cart is made of. Default leaves it to slot.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Material {
    Default,
    Opaque,
    Clear,
    Glitter,
}

impl Material {
    pub const ALL: [Material; 4] = [
        Material::Default,
        Material::Opaque,
        Material::Clear,
        Material::Glitter,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Material::Default => "Default",
            Material::Opaque => "Opaque",
            Material::Clear => "Clear",
            Material::Glitter => "Glitter",
        }
    }

    pub fn of(choice: Option<ShellChoice>) -> Material {
        match choice.map(|c| c.finish) {
            None => Material::Default,
            Some(ShellFinish::Solid) => Material::Opaque,
            Some(ShellFinish::Clear) => Material::Clear,
            Some(ShellFinish::Glitter) => Material::Glitter,
        }
    }

    fn finish(self) -> Option<ShellFinish> {
        match self {
            Material::Default => None,
            Material::Opaque => Some(ShellFinish::Solid),
            Material::Clear => Some(ShellFinish::Clear),
            Material::Glitter => Some(ShellFinish::Glitter),
        }
    }
}

/// The colours on offer, five to a row.
pub const SWATCHES: [[u8; 3]; 18] = [
    [0xee, 0xee, 0xee],
    [0xa0, 0xa0, 0xa0],
    [0x2e, 0x2e, 0x2e],
    [0xe8, 0xe4, 0xdc],
    [0xd2, 0xc3, 0xa6],
    [0x7a, 0x52, 0x33],
    [0xe8, 0x30, 0x2a],
    [0xe2, 0x58, 0x3a],
    [0xed, 0xe2, 0x1b],
    [0x2e, 0xcc, 0x5c],
    [0x2e, 0xe0, 0xa6],
    [0x3f, 0xa3, 0xa0],
    [0x2f, 0x6b, 0xb5],
    [0x6b, 0x4a, 0xa0],
    [0x8a, 0x72, 0x90],
    [0xe0, 0x7a, 0x9e],
    [0xd4, 0xa9, 0x3a],
    [0xc4, 0xc4, 0xcc],
];
pub const SWATCH_COLS: usize = 5;

/// A colour picked with no material yet chosen makes the cart opaque.
const FIRST_COLOUR: [u8; 3] = [0xa0, 0xa0, 0xa0];

/// The choice that results from picking `material` when `now` holds.
pub fn with_material(now: Option<ShellChoice>, material: Material) -> Option<ShellChoice> {
    let finish = material.finish()?;
    Some(ShellChoice {
        outline: now.map_or(Outline::Auto, |c| c.outline),
        colour: now.map_or(FIRST_COLOUR, |c| c.colour),
        finish,
    })
}

/// The choice that results from picking `colour` when `now` holds.
pub fn with_colour(now: Option<ShellChoice>, colour: [u8; 3]) -> ShellChoice {
    ShellChoice {
        outline: now.map_or(Outline::Auto, |c| c.outline),
        colour,
        finish: now.map_or(ShellFinish::Solid, |c| c.finish),
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CustomFocus {
    Material,
    Swatch(usize),
}

/// Where the cursor is on the panel, and the material list when it is open.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CustomizePanel {
    pub focus: CustomFocus,
    pub popup: Option<usize>,
}

impl Default for CustomizePanel {
    fn default() -> Self {
        CustomizePanel {
            focus: CustomFocus::Material,
            popup: None,
        }
    }
}

impl CustomizePanel {
    pub fn up(&mut self) {
        if let Some(at) = &mut self.popup {
            *at = (*at + Material::ALL.len() - 1) % Material::ALL.len();
            return;
        }
        self.focus = match self.focus {
            CustomFocus::Material => CustomFocus::Material,
            CustomFocus::Swatch(i) if i < SWATCH_COLS => CustomFocus::Material,
            CustomFocus::Swatch(i) => CustomFocus::Swatch(i - SWATCH_COLS),
        };
    }

    pub fn down(&mut self) {
        if let Some(at) = &mut self.popup {
            *at = (*at + 1) % Material::ALL.len();
            return;
        }
        self.focus = match self.focus {
            CustomFocus::Material => CustomFocus::Swatch(0),
            CustomFocus::Swatch(i) => {
                CustomFocus::Swatch((i + SWATCH_COLS).min(SWATCHES.len() - 1))
            }
        };
    }

    pub fn step(&mut self, right: bool) {
        if self.popup.is_some() {
            return;
        }
        if let CustomFocus::Swatch(i) = self.focus {
            let col = i % SWATCH_COLS;
            let to = match right {
                true if col + 1 < SWATCH_COLS && i + 1 < SWATCHES.len() => i + 1,
                false if col > 0 => i - 1,
                _ => i,
            };
            self.focus = CustomFocus::Swatch(to);
        }
    }
}

pub const PANEL_X: f32 = 372.0;
pub const PANEL_Y: f32 = 12.0;
pub const PANEL_W: f32 = OUT_W as f32 - PANEL_X - 12.0;
pub const PANEL_H: f32 = 456.0;
pub const PANEL_INNER_X: f32 = PANEL_X + 12.0;
pub const PANEL_INNER_W: f32 = PANEL_W - 24.0;
pub const PANEL_TITLE_H: f32 = 40.0;
pub const PANEL_ROW_H: f32 = 52.0;
const TITLE_Y: f32 = PANEL_Y + 12.0;
const MATERIAL_CAPTION_Y: f32 = TITLE_Y + PANEL_TITLE_H + 6.0;
const CAPTION_H: f32 = 22.0;
const TYPE_Y: f32 = MATERIAL_CAPTION_Y + CAPTION_H + 4.0;
const COLOUR_CAPTION_Y: f32 = TYPE_Y + PANEL_ROW_H + 8.0;
pub const SWATCH_Y: f32 = COLOUR_CAPTION_Y + CAPTION_H + 6.0;
pub const SWATCH_W: f32 = 52.0;
pub const SWATCH_H: f32 = 36.0;
const SWATCH_GAP_X: f32 =
    (PANEL_INNER_W - SWATCH_W * SWATCH_COLS as f32) / (SWATCH_COLS - 1) as f32;
pub const SWATCH_PITCH_Y: f32 = 46.0;
/// The cart's centre while the panel is up.
pub const CUSTOM_CART_X: f32 = PANEL_X / 2.0;

pub fn swatch_rect(i: usize) -> (f32, f32, f32, f32) {
    let (row, col) = (i / SWATCH_COLS, i % SWATCH_COLS);
    (
        PANEL_INNER_X + (SWATCH_W + SWATCH_GAP_X) * col as f32,
        SWATCH_Y + SWATCH_PITCH_Y * row as f32,
        SWATCH_W,
        SWATCH_H,
    )
}

pub struct CustomizeScreen<'a> {
    pub chrome: &'a [(TexId, u32, u32)],
    pub faces: &'a [Option<(TexId, u32, u32)>],
    pub panel: CustomizePanel,
    /// The swatch matching the cart's colour, if any.
    pub chosen: Option<usize>,
}

fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

fn outline(out: &mut Vec<Draw>, (x, y, w, h): (f32, f32, f32, f32), t: f32, colour: [f32; 4]) {
    out.push(Draw::Rect {
        x,
        y,
        w,
        h: t,
        colour,
    });
    out.push(Draw::Rect {
        x,
        y: y + h - t,
        w,
        h: t,
        colour,
    });
    out.push(Draw::Rect {
        x,
        y,
        w: t,
        h,
        colour,
    });
    out.push(Draw::Rect {
        x: x + w - t,
        y,
        w: t,
        h,
        colour,
    });
}

impl CustomizeScreen<'_> {
    fn chrome(&self, c: Chrome, x: f32, y: f32, out: &mut Vec<Draw>) {
        if let Some(&(tex, w, h)) = self.chrome.get(c.index()) {
            out.push(Draw::Tex {
                x: x.round(),
                y: y.round(),
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
    }

    fn face(&self, slot: usize, x: f32, y: f32, centre_w: Option<f32>, out: &mut Vec<Draw>) {
        let Some(Some((tex, w, h))) = self.faces.get(slot).copied() else {
            return;
        };
        let x = match centre_w {
            Some(cw) => x + (cw - w as f32) / 2.0,
            None => x,
        };
        out.push(Draw::Tex {
            x: x.round(),
            y: y.round(),
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }

    /// The panel on the right. The cart, backdrop and pills are drawn by
    /// the shelf's own Socket screen.
    pub fn draw(&self, out: &mut Vec<Draw>) {
        use crate::socket_menu::{
            CUSTOM_COLOUR, CUSTOM_MATERIAL, CUSTOM_TITLE, CUSTOM_TYPE, CUSTOM_TYPE_VALUE,
        };
        self.chrome(Chrome::Panel, PANEL_X, PANEL_Y, out);
        self.chrome(Chrome::PanelTitle, PANEL_INNER_X, TITLE_Y, out);
        self.face(
            CUSTOM_TITLE,
            PANEL_INNER_X,
            TITLE_Y,
            Some(PANEL_INNER_W),
            out,
        );
        self.face(
            CUSTOM_MATERIAL,
            PANEL_INNER_X + 4.0,
            MATERIAL_CAPTION_Y,
            None,
            out,
        );
        let row = match self.panel.focus {
            CustomFocus::Material => Chrome::PanelRowFocus,
            CustomFocus::Swatch(_) => Chrome::PanelRow,
        };
        self.chrome(row, PANEL_INNER_X, TYPE_Y, out);
        self.face(CUSTOM_TYPE, PANEL_INNER_X + 16.0, TYPE_Y + 5.0, None, out);
        self.face(
            CUSTOM_TYPE_VALUE,
            PANEL_INNER_X + 16.0,
            TYPE_Y + 21.0,
            None,
            out,
        );
        self.chrome(
            Chrome::Knob,
            PANEL_INNER_X + PANEL_INNER_W - 10.0 - 34.0,
            TYPE_Y + (PANEL_ROW_H - 34.0) / 2.0,
            out,
        );
        self.face(
            CUSTOM_COLOUR,
            PANEL_INNER_X + 4.0,
            COLOUR_CAPTION_Y,
            None,
            out,
        );
        let ring = [0.12, 0.12, 0.12, 1.0];
        let rim = [0.35, 0.35, 0.35, 1.0];
        for (i, colour) in SWATCHES.iter().enumerate() {
            let r = swatch_rect(i);
            let (x, y, w, h) = r;
            out.push(Draw::Rect {
                x,
                y,
                w,
                h,
                colour: rgb(*colour),
            });
            outline(out, r, 2.0, rim);
            if self.chosen == Some(i) {
                outline(
                    out,
                    (x + 4.0, y + 4.0, w - 8.0, h - 8.0),
                    2.0,
                    [1.0, 1.0, 1.0, 0.9],
                );
            }
            if self.panel.focus == CustomFocus::Swatch(i) {
                outline(out, (x - 4.0, y - 4.0, w + 8.0, h + 8.0), 3.0, ring);
            }
        }
    }
}

const INSPECT_ROW_TOP: f32 = PANEL_Y + 12.0 + PANEL_TITLE_H + 12.0;
pub const INSPECT_PITCH: f32 = 54.0;

pub fn inspect_row_top(i: usize) -> f32 {
    INSPECT_ROW_TOP + INSPECT_PITCH * i as f32
}

/// A game's particulars beside it: on a panel like Customize's, a card for
/// each fact, and the ways on from here along the foot.
pub struct InspectScreen<'a> {
    pub chrome: &'a [(TexId, u32, u32)],
    pub faces: &'a [Option<(TexId, u32, u32)>],
    pub rows: usize,
}

impl InspectScreen<'_> {
    fn chrome(&self, c: Chrome, x: f32, y: f32, out: &mut Vec<Draw>) {
        if let Some(&(tex, w, h)) = self.chrome.get(c.index()) {
            out.push(Draw::Tex {
                x: x.round(),
                y: y.round(),
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
    }

    fn face(&self, slot: usize) -> Option<(TexId, f32, f32)> {
        self.faces
            .get(slot)
            .copied()
            .flatten()
            .map(|(t, w, h)| (t, w as f32, h as f32))
    }

    fn put(&self, slot: usize, x: f32, y: f32, out: &mut Vec<Draw>) -> f32 {
        let Some((tex, w, h)) = self.face(slot) else {
            return 0.0;
        };
        out.push(Draw::Tex {
            x: x.round(),
            y: y.round(),
            w,
            h,
            tex,
            alpha: 1.0,
        });
        w
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        use crate::socket_menu::{
            inspect_caption_slot, inspect_hint_slot, inspect_value_slot, CUSTOM_TITLE, INSPECT_ROWS,
        };
        self.chrome(Chrome::Panel, PANEL_X, PANEL_Y, out);
        let title_y = PANEL_Y + 12.0;
        self.chrome(Chrome::PanelTitle, PANEL_INNER_X, title_y, out);
        if let Some((_, w, _)) = self.face(CUSTOM_TITLE) {
            self.put(
                CUSTOM_TITLE,
                PANEL_INNER_X + (PANEL_INNER_W - w) / 2.0,
                title_y,
                out,
            );
        }
        for i in 0..self.rows.min(INSPECT_ROWS) {
            let y = inspect_row_top(i);
            self.chrome(Chrome::PanelRow, PANEL_INNER_X, y, out);
            self.put(inspect_caption_slot(i), PANEL_INNER_X + 16.0, y + 5.0, out);
            self.put(inspect_value_slot(i), PANEL_INNER_X + 16.0, y + 21.0, out);
        }
        let foot = OUT_H as f32 - 12.0 - 34.0;
        let mut x = 12.0;
        for i in [0, 3, 1] {
            let w = self.put(inspect_hint_slot(i), x, foot, out);
            if w > 0.0 {
                x += w + 8.0;
            }
        }
        if let Some((_, w, _)) = self.face(inspect_hint_slot(2)) {
            self.put(
                inspect_hint_slot(2),
                PANEL_INNER_X + PANEL_INNER_W - w,
                foot,
                out,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_walks_from_the_material_into_the_grid_and_back() {
        let mut p = CustomizePanel::default();
        p.down();
        assert_eq!(p.focus, CustomFocus::Swatch(0));
        p.step(true);
        p.down();
        assert_eq!(p.focus, CustomFocus::Swatch(6));
        p.down();
        p.down();
        assert_eq!(p.focus, CustomFocus::Swatch(16));
        p.down();
        assert_eq!(
            p.focus,
            CustomFocus::Swatch(17),
            "the cursor fell off the grid"
        );
        p.step(true);
        assert_eq!(p.focus, CustomFocus::Swatch(17));
        for _ in 0..4 {
            p.up();
        }
        assert_eq!(p.focus, CustomFocus::Material);
    }

    #[test]
    fn left_and_right_stay_on_their_row() {
        let mut p = CustomizePanel {
            focus: CustomFocus::Swatch(4),
            popup: None,
        };
        p.step(true);
        assert_eq!(p.focus, CustomFocus::Swatch(4));
        p.focus = CustomFocus::Swatch(5);
        p.step(false);
        assert_eq!(p.focus, CustomFocus::Swatch(5));
    }

    #[test]
    fn the_material_list_wraps() {
        let mut p = CustomizePanel {
            focus: CustomFocus::Material,
            popup: Some(0),
        };
        p.up();
        assert_eq!(p.popup, Some(3));
        p.down();
        assert_eq!(p.popup, Some(0));
    }

    #[test]
    fn picking_a_colour_on_a_default_cart_makes_it_opaque() {
        let c = with_colour(None, SWATCHES[6]);
        assert_eq!(c.finish, ShellFinish::Solid);
        assert_eq!(c.colour, SWATCHES[6]);
        assert_eq!(with_material(Some(c), Material::Default), None);
        let clear = with_material(Some(c), Material::Clear).unwrap();
        assert_eq!(
            (clear.colour, clear.finish),
            (SWATCHES[6], ShellFinish::Clear)
        );
        assert_eq!(Material::of(Some(clear)), Material::Clear);
    }

    #[test]
    fn the_inspect_cards_end_above_the_split() {
        let last = inspect_row_top(crate::socket_menu::INSPECT_ROWS - 1) + PANEL_ROW_H;
        assert!(last < crate::socket::SOCKET_SPLIT, "{last}");
    }

    #[test]
    fn the_grid_fits_the_panel_and_ends_above_the_split() {
        let last = swatch_rect(SWATCHES.len() - 1);
        assert!(last.0 + last.2 <= PANEL_INNER_X + PANEL_INNER_W + 0.01);
        assert!(last.1 + last.3 + 4.0 < crate::socket::SOCKET_SPLIT);
    }
}
