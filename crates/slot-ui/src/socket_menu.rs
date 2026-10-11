//! Settings in the Socket style: a light card on a dark ground, the
//! categories down the left, the selected category's settings as cards on
//! the right, and the power menu as a popup over whatever is showing.

use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::plate::UndoFace;
use crate::socket::{draw_run, inter_text_face, pill, rounded_rect, run_width, Style, KEY_SEP};

/// The pieces of the settings screen that never change, built once.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Chrome {
    Card,
    SideRest,
    SideLit,
    SideFocus,
    RowRest,
    RowFocus,
    Knob,
    KnobCycle,
    Popup,
    PopupTitle,
    PopupRest,
    PopupFocus,
    PopupClose,
    Panel,
    PanelTitle,
    PanelRow,
    PanelRowFocus,
}

impl Chrome {
    pub const ALL: [Chrome; 17] = [
        Chrome::Card,
        Chrome::SideRest,
        Chrome::SideLit,
        Chrome::SideFocus,
        Chrome::RowRest,
        Chrome::RowFocus,
        Chrome::Knob,
        Chrome::KnobCycle,
        Chrome::Popup,
        Chrome::PopupTitle,
        Chrome::PopupRest,
        Chrome::PopupFocus,
        Chrome::PopupClose,
        Chrome::Panel,
        Chrome::PanelTitle,
        Chrome::PanelRow,
        Chrome::PanelRowFocus,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

const GROUND: [f32; 4] = [0.2, 0.2, 0.2, 1.0];
/// How much of the card covers the backdrop's dots.
const CARD_VEIL: f32 = 0.35;
const CARD: [u8; 3] = [220, 220, 220];
const REST: [u8; 3] = [236, 236, 236];
const LIT: [u8; 3] = [172, 172, 172];
const FOCUS: [u8; 3] = [189, 189, 189];
const OUTLINE: [u8; 3] = [30, 30, 30];
const KNOB: [u8; 3] = [20, 20, 20];
const POPUP_TITLE: [u8; 3] = [205, 205, 205];
pub(crate) const RED: [u8; 3] = [223, 77, 83];
const INK: [u8; 3] = [22, 22, 22];
const CAPTION: [u8; 3] = [96, 96, 96];

pub const CARD_X: f32 = 14.0;
pub const CARD_Y: f32 = 12.0;
pub const CARD_W: f32 = OUT_W as f32 - 2.0 * CARD_X;
pub const CARD_H: f32 = OUT_H as f32 - 2.0 * CARD_Y;
pub const SIDE_X: f32 = CARD_X + 12.0;
pub const SIDE_W: f32 = 170.0;
pub const LIST_X: f32 = SIDE_X + SIDE_W + 12.0;
pub const LIST_W: f32 = CARD_X + CARD_W - 12.0 - LIST_X;
pub const ITEM_TOP: f32 = CARD_Y + 12.0;
pub const ITEM_PITCH: f32 = 53.0;
pub const ITEM_H: f32 = 47.0;
pub const ITEMS: usize = 7;
pub const FOOT_Y: f32 = 418.0;
const FOOT_H: u32 = 34;
const KNOB_D: f32 = 34.0;

/// Where a taller screen opens the settings up: in the gap below the last
/// card, where every column is plain.
pub const SETTINGS_SPLIT: f32 = 400.0;

pub const POPUP_W: f32 = 500.0;
const POPUP_PAD: f32 = 12.0;
const POPUP_ROW: f32 = 46.0;
const POPUP_GAP: f32 = 8.0;
pub const POPUP_OPTIONS: usize = 4;

fn chrome_size(c: Chrome) -> (u32, u32) {
    match c {
        Chrome::Card => (CARD_W as u32, CARD_H as u32),
        Chrome::SideRest | Chrome::SideLit | Chrome::SideFocus => (SIDE_W as u32, ITEM_H as u32),
        Chrome::RowRest | Chrome::RowFocus => (LIST_W as u32, ITEM_H as u32),
        Chrome::Knob | Chrome::KnobCycle => (KNOB_D as u32, KNOB_D as u32),
        Chrome::Popup => (POPUP_W as u32, popup_h(POPUP_OPTIONS) as u32),
        Chrome::PopupTitle | Chrome::PopupRest | Chrome::PopupFocus | Chrome::PopupClose => {
            ((POPUP_W - 2.0 * POPUP_PAD) as u32, POPUP_ROW as u32)
        }
        Chrome::Panel => (
            crate::customize::PANEL_W as u32,
            crate::customize::PANEL_H as u32,
        ),
        Chrome::PanelTitle => (
            crate::customize::PANEL_INNER_W as u32,
            crate::customize::PANEL_TITLE_H as u32,
        ),
        Chrome::PanelRow | Chrome::PanelRowFocus => (
            crate::customize::PANEL_INNER_W as u32,
            crate::customize::PANEL_ROW_H as u32,
        ),
    }
}

fn block(w: u32, h: u32, r: f32, fill: [u8; 3], outline: bool) -> UndoFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (wf, hf) = (w as f32, h as f32);
    if outline {
        rounded_rect(&mut rgba, w, h, 0.0, 0.0, wf, hf, r, OUTLINE, 1.0);
        rounded_rect(
            &mut rgba,
            w,
            h,
            3.0,
            3.0,
            wf - 3.0,
            hf - 3.0,
            r - 2.0,
            fill,
            1.0,
        );
    } else {
        rounded_rect(&mut rgba, w, h, 0.0, 0.0, wf, hf, r, fill, 1.0);
    }
    UndoFace { rgba, w, h }
}

fn knob(text: &str) -> UndoFace {
    let d = KNOB_D as u32;
    let mut rgba = vec![0u8; (d * d * 4) as usize];
    rounded_rect(
        &mut rgba,
        d,
        d,
        0.0,
        0.0,
        KNOB_D,
        KNOB_D,
        KNOB_D / 2.0,
        KNOB,
        1.0,
    );
    let px = 22.0;
    let tw = run_width(text, px);
    draw_run(
        &mut rgba,
        d,
        d,
        (KNOB_D - tw) / 2.0,
        px,
        text,
        [245, 245, 245],
    );
    UndoFace { rgba, w: d, h: d }
}

/// One piece of the settings screen's frame.
pub fn chrome_face(c: Chrome) -> UndoFace {
    let (w, h) = chrome_size(c);
    match c {
        Chrome::Card => block(w, h, 18.0, CARD, false),
        Chrome::SideRest | Chrome::RowRest | Chrome::PopupRest => block(w, h, 9.0, REST, false),
        Chrome::SideLit => block(w, h, 9.0, LIT, false),
        Chrome::SideFocus | Chrome::RowFocus | Chrome::PopupFocus => block(w, h, 10.0, FOCUS, true),
        Chrome::Knob => knob("\u{203a}"),
        Chrome::KnobCycle => knob("\u{2039}\u{203a}"),
        Chrome::Popup => block(w, h, 16.0, CARD, false),
        Chrome::PopupTitle => block(w, h, 8.0, POPUP_TITLE, false),
        Chrome::PopupClose => block(w, h, 8.0, RED, false),
        Chrome::Panel => block(w, h, 16.0, CARD, false),
        Chrome::PanelTitle => block(w, h, 8.0, POPUP_TITLE, false),
        Chrome::PanelRow => block(w, h, 9.0, REST, false),
        Chrome::PanelRowFocus => block(w, h, 10.0, FOCUS, true),
    }
}

pub const SIDE0: usize = 0;
const CAPTION0: usize = SIDE0 + ITEMS;
const VALUE0: usize = CAPTION0 + ITEMS;
pub const SETTINGS_CLOSE: usize = VALUE0 + ITEMS;
pub const SETTINGS_HINT: usize = SETTINGS_CLOSE + 1;
pub const POPUP_TITLE_SLOT: usize = SETTINGS_HINT + 1;
const OPTION0: usize = POPUP_TITLE_SLOT + 1;
pub const POPUP_CLOSE_SLOT: usize = OPTION0 + POPUP_OPTIONS;
pub const CUSTOM_TITLE: usize = POPUP_CLOSE_SLOT + 1;
pub const CUSTOM_MATERIAL: usize = CUSTOM_TITLE + 1;
pub const CUSTOM_COLOUR: usize = CUSTOM_MATERIAL + 1;
pub const CUSTOM_TYPE: usize = CUSTOM_COLOUR + 1;
pub const CUSTOM_TYPE_VALUE: usize = CUSTOM_TYPE + 1;
pub const CUSTOM_CLOSE: usize = CUSTOM_TYPE_VALUE + 1;
pub const SETTINGS_SLOTS: usize = CUSTOM_CLOSE + 1;

pub fn side_slot(i: usize) -> usize {
    SIDE0 + i
}

pub fn caption_slot(i: usize) -> usize {
    CAPTION0 + i
}

pub fn value_slot(i: usize) -> usize {
    VALUE0 + i
}

pub fn option_slot(i: usize) -> usize {
    OPTION0 + i
}

/// The face for one slot of the Socket settings screen.
pub fn settings_face(slot: usize, text: &str, _lit: bool) -> UndoFace {
    if text.is_empty() {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    }
    let split = |t: &str| match t.split_once(KEY_SEP) {
        Some((k, l)) => (Some(k.to_string()), l.to_string()),
        None => (None, t.to_string()),
    };
    match slot {
        s if s < CAPTION0 => inter_text_face(text, 19.0, ITEM_H as u32, INK),
        s if s < VALUE0 => inter_text_face(text, 13.0, 18, CAPTION),
        s if s < SETTINGS_CLOSE => inter_text_face(text, 19.0, 26, INK),
        SETTINGS_CLOSE => {
            let (k, l) = split(text);
            pill(k.as_deref(), &l, Style::Red, FOOT_H)
        }
        SETTINGS_HINT => {
            let (k, l) = split(text);
            pill(k.as_deref(), &l, Style::Grey, FOOT_H)
        }
        POPUP_TITLE_SLOT => inter_text_face(text, 22.0, POPUP_ROW as u32, INK),
        CUSTOM_TITLE => inter_text_face(text, 19.0, 40, INK),
        CUSTOM_MATERIAL | CUSTOM_COLOUR => inter_text_face(text, 15.0, 22, CAPTION),
        CUSTOM_TYPE => inter_text_face(text, 13.0, 18, CAPTION),
        CUSTOM_TYPE_VALUE => inter_text_face(text, 19.0, 26, INK),
        CUSTOM_CLOSE => {
            let (k, l) = split(text);
            pill(k.as_deref(), &l, Style::Red, FOOT_H)
        }
        POPUP_CLOSE_SLOT => {
            let (k, l) = split(text);
            pill(k.as_deref(), &l, Style::Bare, POPUP_ROW as u32)
        }
        _ => inter_text_face(text, 18.0, POPUP_ROW as u32, INK),
    }
}

/// One card on the right: its setting, and whether it opens rather than
/// steps through values.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Card {
    pub opens: bool,
}

pub struct SocketSettings<'a> {
    /// The shelf's dotted backdrop, drawn behind the card.
    pub backdrop: Option<TexId>,
    pub chrome: &'a [(TexId, u32, u32)],
    pub faces: &'a [Option<(TexId, u32, u32)>],
    /// The categories down the left, which is lit and whether the cursor
    /// is on it.
    pub sides: usize,
    pub side_lit: Option<usize>,
    pub side_focus: bool,
    /// The cards on the right and which the cursor is on.
    pub cards: &'a [Card],
    pub card_focus: Option<usize>,
}

pub fn item_top(i: usize) -> f32 {
    ITEM_TOP + ITEM_PITCH * i as f32
}

fn chrome_at(chrome: &[(TexId, u32, u32)], c: Chrome, x: f32, y: f32, out: &mut Vec<Draw>) {
    if let Some(&(tex, w, h)) = chrome.get(c.index()) {
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

fn face_at(
    faces: &[Option<(TexId, u32, u32)>],
    slot: usize,
    x: f32,
    y: f32,
    out: &mut Vec<Draw>,
) -> Option<(f32, f32)> {
    let (tex, w, h) = faces.get(slot).copied().flatten()?;
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
    Some((w as f32, h as f32))
}

fn face_w(faces: &[Option<(TexId, u32, u32)>], slot: usize) -> f32 {
    faces
        .get(slot)
        .copied()
        .flatten()
        .map_or(0.0, |(_, w, _)| w as f32)
}

impl SocketSettings<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        match self.backdrop {
            Some(tex) => out.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                tex,
                alpha: 1.0,
            }),
            None => out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: GROUND,
            }),
        }
        // On the shelf's backdrop the card is a light veil, so the dots show
        // through it and every screen reads as one surface.
        if let Some(&(tex, w, h)) = self.chrome.get(Chrome::Card.index()) {
            out.push(Draw::Tex {
                x: CARD_X,
                y: CARD_Y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: if self.backdrop.is_some() {
                    CARD_VEIL
                } else {
                    1.0
                },
            });
        }
        for i in 0..self.sides.min(ITEMS) {
            let y = item_top(i);
            let piece = match (self.side_lit == Some(i), self.side_focus) {
                (true, true) => Chrome::SideFocus,
                (true, false) => Chrome::SideLit,
                _ => Chrome::SideRest,
            };
            chrome_at(self.chrome, piece, SIDE_X, y, out);
            let w = face_w(self.faces, side_slot(i));
            face_at(
                self.faces,
                side_slot(i),
                SIDE_X + (SIDE_W - w) / 2.0,
                y,
                out,
            );
        }
        for (i, card) in self.cards.iter().take(ITEMS).enumerate() {
            let y = item_top(i);
            let piece = match self.card_focus == Some(i) {
                true => Chrome::RowFocus,
                false => Chrome::RowRest,
            };
            chrome_at(self.chrome, piece, LIST_X, y, out);
            let text_x = LIST_X + 18.0;
            let has_caption = face_w(self.faces, caption_slot(i)) > 0.0;
            if has_caption {
                face_at(self.faces, caption_slot(i), text_x, y + 4.0, out);
                face_at(self.faces, value_slot(i), text_x, y + 19.0, out);
            } else {
                face_at(
                    self.faces,
                    value_slot(i),
                    text_x,
                    y + (ITEM_H - 26.0) / 2.0,
                    out,
                );
            }
            let knob = if card.opens {
                Chrome::Knob
            } else {
                Chrome::KnobCycle
            };
            chrome_at(
                self.chrome,
                knob,
                LIST_X + LIST_W - 10.0 - KNOB_D,
                y + (ITEM_H - KNOB_D) / 2.0,
                out,
            );
        }
        face_at(self.faces, SETTINGS_CLOSE, SIDE_X, FOOT_Y, out);
        let hint = face_w(self.faces, SETTINGS_HINT);
        face_at(
            self.faces,
            SETTINGS_HINT,
            LIST_X + LIST_W - hint,
            FOOT_Y,
            out,
        );
    }
}

pub fn popup_h(options: usize) -> f32 {
    POPUP_PAD * 2.0
        + POPUP_ROW
        + POPUP_GAP
        + (POPUP_ROW + POPUP_GAP) * options as f32
        + POPUP_GAP * 2.0
        + POPUP_ROW
}

/// A popup list over whatever shows: a title, its options with the cursor
/// on one, and a red bar to close it.
pub struct SocketPopup<'a> {
    pub chrome: &'a [(TexId, u32, u32)],
    pub faces: &'a [Option<(TexId, u32, u32)>],
    pub options: usize,
    pub focus: usize,
}

impl SocketPopup<'_> {
    /// Centred, or higher where that would cross the split a taller screen
    /// opens up below.
    pub fn top() -> f32 {
        let h = popup_h(POPUP_OPTIONS);
        ((OUT_H as f32 - h) / 2.0).min(SETTINGS_SPLIT - 8.0 - h)
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: [0.0, 0.0, 0.0, 0.55],
        });
        let x = (OUT_W as f32 - POPUP_W) / 2.0;
        let top = Self::top();
        chrome_at(self.chrome, Chrome::Popup, x, top, out);
        let inner = x + POPUP_PAD;
        let inner_w = POPUP_W - 2.0 * POPUP_PAD;
        let mut y = top + POPUP_PAD;
        chrome_at(self.chrome, Chrome::PopupTitle, inner, y, out);
        let tw = face_w(self.faces, POPUP_TITLE_SLOT);
        face_at(
            self.faces,
            POPUP_TITLE_SLOT,
            inner + (inner_w - tw) / 2.0,
            y,
            out,
        );
        y += POPUP_ROW + POPUP_GAP;
        for i in 0..self.options.min(POPUP_OPTIONS) {
            let piece = match i == self.focus {
                true => Chrome::PopupFocus,
                false => Chrome::PopupRest,
            };
            chrome_at(self.chrome, piece, inner, y, out);
            face_at(self.faces, option_slot(i), inner + 14.0, y, out);
            y += POPUP_ROW + POPUP_GAP;
        }
        let close_y = top + popup_h(POPUP_OPTIONS) - POPUP_PAD - POPUP_ROW;
        chrome_at(self.chrome, Chrome::PopupClose, inner, close_y, out);
        let cw = face_w(self.faces, POPUP_CLOSE_SLOT);
        face_at(
            self.faces,
            POPUP_CLOSE_SLOT,
            inner + (inner_w - cw) / 2.0,
            close_y,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cards_end_above_the_split_and_the_footer_sits_below_it() {
        let last = item_top(ITEMS - 1) + ITEM_H;
        assert!(last < SETTINGS_SPLIT, "{last}");
        const {
            assert!(FOOT_Y > SETTINGS_SPLIT);
            assert!(FOOT_Y + FOOT_H as f32 <= CARD_Y + CARD_H);
            assert!(LIST_X + LIST_W <= CARD_X + CARD_W);
        }
    }

    #[test]
    fn the_popup_fits_on_the_screen() {
        assert!(SocketPopup::top() > 0.0);
        assert!(SocketPopup::top() + popup_h(POPUP_OPTIONS) < SETTINGS_SPLIT);
    }

    #[test]
    fn every_piece_of_chrome_is_its_size() {
        for c in Chrome::ALL {
            let f = chrome_face(c);
            assert_eq!((f.w, f.h), chrome_size(c), "{c:?}");
            assert_eq!(f.rgba.len(), (f.w * f.h * 4) as usize);
        }
    }

    #[test]
    fn a_focused_card_has_a_dark_outline() {
        let f = chrome_face(Chrome::RowFocus);
        let edge = &f.rgba[((f.h / 2) * f.w * 4) as usize..][..4];
        assert!(edge[0] < 60 && edge[3] > 200, "{edge:?}");
        let rest = chrome_face(Chrome::RowRest);
        let edge = &rest.rgba[((rest.h / 2) * rest.w * 4) as usize..][..4];
        assert!(edge[0] > 200, "{edge:?}");
    }

    #[test]
    fn the_split_row_is_plain_in_every_column() {
        let mut out = Vec::new();
        SocketSettings {
            backdrop: None,
            chrome: &[],
            faces: &[],
            sides: ITEMS,
            side_lit: Some(0),
            side_focus: true,
            cards: &[Card { opens: false }; ITEMS],
            card_focus: Some(ITEMS - 1),
        }
        .draw(&mut out);
        for d in &out {
            if let Some((_, y, _, h)) = d.bounds() {
                if h < OUT_H as f32 - 2.0 * CARD_Y {
                    assert!(
                        !(y < SETTINGS_SPLIT && y + h > SETTINGS_SPLIT),
                        "{d:?} crosses the split"
                    );
                }
            }
        }
    }
}
