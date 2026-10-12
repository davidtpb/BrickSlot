//! The Socket look: a light dotted backdrop, the selected cart large and
//! swaying in 3D between greyed neighbours, and the controls on pills.

use std::sync::OnceLock;

use fontdue::{Font, FontSettings};
use slot_gfx::{Draw, TexId, OUT_H, OUT_W};

use crate::cart::{cart_box, label_colour, label_text};
use crate::plate::UndoFace;
use crate::shelf::Shelf;

const INTER_TTF: &[u8] = include_bytes!("../assets/Inter-SemiBold.ttf");

pub fn inter_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(INTER_TTF, FontSettings::default()).ok())
        .as_ref()
}

/// Splits a key from its label in a pill's text: `"A\u{1}Play"`.
pub const KEY_SEP: char = '\u{1}';

pub const SOCKET_CLOCK: usize = 0;
pub const SOCKET_L: usize = 1;
pub const SOCKET_R: usize = 2;
pub const SOCKET_STATUS: usize = 3;
pub const SOCKET_TABS: usize = 4;
const TAB0: usize = 4;
pub const SOCKET_NAME: usize = TAB0 + SOCKET_TABS;
pub const SOCKET_BOTTOM: usize = 3;
const BOTTOM0: usize = SOCKET_NAME + 1;
pub const SOCKET_SLOTS: usize = BOTTOM0 + SOCKET_BOTTOM;

pub fn socket_tab_slot(i: usize) -> usize {
    TAB0 + i
}

pub fn socket_bottom_slot(i: usize) -> usize {
    BOTTOM0 + i
}

pub(crate) const TOP_Y: f32 = 10.0;
pub(crate) const TOP_H: u32 = 28;
const NAME_H: u32 = 34;
pub(crate) const BOTTOM_H: u32 = 32;
pub(crate) const EDGE: f32 = 12.0;
pub(crate) const GAP: f32 = 8.0;
pub const NAME_Y: f32 = 336.0;
pub const BOTTOM_Y: f32 = OUT_H as f32 - EDGE - BOTTOM_H as f32;

/// Where a screen taller than the scene opens it up: between the game's
/// name and the controls, on a row the dots miss so the stretch reads plain.
pub const SOCKET_SPLIT: f32 = 400.0;

/// Where the carousel's carts sit.
pub const ROW_Y: f32 = 205.0;
pub const SPACING: f32 = 255.0;
pub const MAIN_BOX: (f32, f32) = (290.0, 215.0);
pub const SIDE_BOX: (f32, f32) = (170.0, 128.0);
const SIDE_ALPHA: f32 = 0.82;
const SIDE_GREY: f32 = 0.7;

/// How far the selected cart swings, in radians, and how long each swing
/// takes, in seconds.
const YAW: (f32, f32) = (0.30, 4.2);
const PITCH: (f32, f32) = (0.13, 5.6);
const ROLL: (f32, f32) = (0.045, 4.2);
/// The camera's distance in cart widths: nearer exaggerates perspective.
const EYE: f32 = 2.4;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Style {
    /// White on near-black: the clock and the status.
    Dark,
    /// A shoulder button: a white key on near-black.
    Shoulder,
    /// Near-black on near-white: the selected game's name.
    Light,
    /// Near-black on mid grey: the controls along the bottom.
    Grey,
    /// A platform's name, set off when it is the one showing.
    Tab(bool),
    /// Near-black on red: the way out of the settings.
    Red,
    /// Near-black on nothing, for a pill laid on a bar of its own.
    Bare,
}

fn style_of(slot: usize, lit: bool) -> Style {
    match slot {
        SOCKET_CLOCK | SOCKET_STATUS => Style::Dark,
        SOCKET_L | SOCKET_R => Style::Shoulder,
        SOCKET_NAME => Style::Light,
        s if s >= BOTTOM0 => Style::Grey,
        _ => Style::Tab(lit),
    }
}

fn height_of(slot: usize) -> u32 {
    match slot {
        SOCKET_NAME => NAME_H,
        s if s >= BOTTOM0 => BOTTOM_H,
        _ => TOP_H,
    }
}

/// The face for one slot of the Socket shelf.
pub fn socket_pill_face(slot: usize, text: &str, lit: bool) -> UndoFace {
    if text.is_empty() {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    }
    if slot == SOCKET_STATUS {
        return status_pill(&Status::parse(text), height_of(slot));
    }
    let (key, label) = match text.split_once(KEY_SEP) {
        Some((k, l)) => (Some(k), l),
        None => (None, text),
    };
    pill(key, label, style_of(slot, lit), height_of(slot))
}

pub(crate) const INK_DARK: [u8; 3] = [24, 24, 24];
pub(crate) const INK_LIGHT: [u8; 3] = [242, 242, 242];
pub(crate) const INK_DIM: [u8; 3] = [108, 108, 108];

pub(crate) fn pill(key: Option<&str>, label: &str, style: Style, h: u32) -> UndoFace {
    let hf = h as f32;
    let px = match style {
        Style::Bare => 18.0,
        _ => (hf * 0.56).round(),
    };
    let pad = (hf * 0.42).round();
    let (bg, ink): (Option<([u8; 3], f32)>, [u8; 3]) = match style {
        Style::Dark | Style::Shoulder => (Some(([44, 44, 44], 1.0)), INK_LIGHT),
        Style::Light => (Some(([238, 238, 238], 0.94)), INK_DARK),
        Style::Grey => (Some(([150, 150, 150], 0.9)), INK_DARK),
        Style::Tab(true) => (Some(([204, 204, 204], 0.85)), INK_DARK),
        Style::Tab(false) => (None, INK_DIM),
        Style::Red => (Some((crate::socket_menu::RED, 1.0)), INK_DARK),
        Style::Bare => (None, INK_DARK),
    };
    let text_w = run_width(label, px);
    let key_w = match (style, key) {
        (Style::Shoulder, _) => hf * 0.62,
        (_, Some(k)) if k.chars().count() == 1 => hf * 0.62,
        (_, Some(k)) => run_width(k, px * 0.72) + hf * 0.5,
        (_, None) => 0.0,
    };
    let key_gap = if key_w > 0.0 && !label.is_empty() {
        hf * 0.2
    } else {
        0.0
    };
    let pad = if style == Style::Shoulder {
        hf * 0.28
    } else {
        pad
    };
    let w = (pad * 2.0 + key_w + key_gap + text_w).ceil() as u32;
    let w = w.max(h);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    if let Some((rgb, a)) = bg {
        rounded_rect(&mut rgba, w, h, 0.0, 0.0, w as f32, hf, hf * 0.32, rgb, a);
    }
    let mut x = pad;
    if style == Style::Shoulder {
        let k = key.unwrap_or(label);
        let s = key_w;
        let top = (hf - s) / 2.0;
        rounded_rect(
            &mut rgba,
            w,
            h,
            x,
            top,
            x + s,
            top + s,
            s * 0.25,
            INK_LIGHT,
            1.0,
        );
        let kw = run_width(k, px * 0.72);
        draw_run(&mut rgba, w, h, x + (s - kw) / 2.0, px * 0.72, k, INK_DARK);
        return UndoFace { rgba, w, h };
    }
    if let Some(k) = key {
        let s = hf * 0.62;
        let top = (hf - s) / 2.0;
        rounded_rect(
            &mut rgba,
            w,
            h,
            x,
            top,
            x + key_w,
            top + s,
            s / 2.0,
            [12, 12, 12],
            1.0,
        );
        let kpx = px * 0.72;
        let kw = run_width(k, kpx);
        draw_run(&mut rgba, w, h, x + (key_w - kw) / 2.0, kpx, k, INK_LIGHT);
        x += key_w + key_gap;
    }
    draw_run(&mut rgba, w, h, x, px, label, ink);
    UndoFace { rgba, w, h }
}

/// What the status pill shows: the battery's charge, whether it is
/// charging, and whether Wi-Fi is on. As text: `"72"`, `"72+"` charging,
/// with a trailing `"w"` for Wi-Fi.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Status {
    pub percent: Option<u8>,
    pub charging: bool,
    pub wifi: bool,
}

impl Status {
    pub fn text(&self) -> String {
        let mut t = self.percent.map_or("-".into(), |p| p.to_string());
        if self.charging {
            t.push('+');
        }
        if self.wifi {
            t.push('w');
        }
        t
    }

    pub fn parse(text: &str) -> Status {
        let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
        Status {
            percent: digits.parse().ok(),
            charging: text.contains('+'),
            wifi: text.contains('w'),
        }
    }
}

pub(crate) fn status_pill(st: &Status, h: u32) -> UndoFace {
    let hf = h as f32;
    let pad = hf * 0.42;
    let (bw, bh) = (hf * 0.82, hf * 0.42);
    let gap = hf * 0.36;
    let wifi_w = if st.wifi { hf * 0.62 } else { 0.0 };
    // The charge as a number, ahead of the battery.
    let label = st.percent.map(|p| format!("{}%", p.min(100)));
    let text_px = hf * 0.5;
    let text_w = label
        .as_deref()
        .map_or(0.0, |t| run_width(t, text_px) + gap * 0.6);
    let w =
        (pad * 2.0 + text_w + bw + 2.5 + if st.wifi { gap + wifi_w } else { 0.0 }).ceil() as u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    rounded_rect(
        &mut rgba,
        w,
        h,
        0.0,
        0.0,
        w as f32,
        hf,
        hf * 0.32,
        [44, 44, 44],
        1.0,
    );
    if let Some(t) = &label {
        draw_run(&mut rgba, w, h, pad, text_px, t, INK_LIGHT);
    }
    let (x, y) = (pad + text_w, (hf - bh) / 2.0);
    rounded_rect(&mut rgba, w, h, x, y, x + bw, y + bh, 2.5, INK_LIGHT, 1.0);
    let inset = 1.6;
    rounded_rect(
        &mut rgba,
        w,
        h,
        x + inset,
        y + inset,
        x + bw - inset,
        y + bh - inset,
        1.5,
        [44, 44, 44],
        1.0,
    );
    let level = st.percent.map_or(1.0, |p| f32::from(p.min(100)) / 100.0);
    let fill = if st.percent.is_some_and(|p| p <= 15) {
        [235, 80, 70]
    } else if st.charging {
        [110, 210, 120]
    } else {
        INK_LIGHT
    };
    let inner = (bw - 2.0 * inset - 2.0) * level;
    if inner > 0.5 {
        rounded_rect(
            &mut rgba,
            w,
            h,
            x + inset + 1.0,
            y + inset + 1.0,
            x + inset + 1.0 + inner,
            y + bh - inset - 1.0,
            1.0,
            fill,
            1.0,
        );
    }
    rounded_rect(
        &mut rgba,
        w,
        h,
        x + bw + 0.5,
        hf / 2.0 - bh * 0.2,
        x + bw + 2.5,
        hf / 2.0 + bh * 0.2,
        1.0,
        INK_LIGHT,
        1.0,
    );
    if st.wifi {
        wifi_glyph(&mut rgba, w, h, x + bw + 2.5 + gap, wifi_w);
    }
    UndoFace { rgba, w, h }
}

/// Three arcs over a dot, fanned upward from the bottom of a `size` box.
fn wifi_glyph(dst: &mut [u8], w: u32, h: u32, x0: f32, size: f32) {
    let hf = h as f32;
    let (cx, cy) = (x0 + size / 2.0, hf / 2.0 + size * 0.38);
    let stroke = size * 0.13;
    let radii = [size * 0.18, size * 0.46, size * 0.74];
    for y in 0..h as i32 {
        for x in x0.floor() as i32..(x0 + size).ceil() as i32 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            if dy > 0.0 {
                continue;
            }
            let angle = dx.atan2(-dy).abs();
            let d = (dx * dx + dy * dy).sqrt();
            let mut cover: f32 = 0.0;
            if d <= radii[0] {
                cover = (radii[0] - d + 0.5).clamp(0.0, 1.0);
            } else if angle <= std::f32::consts::FRAC_PI_4 + 0.05 {
                for r in &radii[1..] {
                    cover = cover.max((stroke / 2.0 - (d - r).abs() + 0.5).clamp(0.0, 1.0));
                }
            }
            blend(dst, w, h, x, y, INK_LIGHT, cover);
        }
    }
}

fn font_for(c: char) -> Option<&'static Font> {
    if ('\u{e000}'..='\u{f8ff}').contains(&c) {
        crate::icon::symbols_font()
    } else {
        inter_font()
    }
}

/// Text alone, in Inter, `h` high and as wide as it sets.
pub fn inter_text_face(text: &str, px: f32, h: u32, ink: [u8; 3]) -> UndoFace {
    if text.is_empty() {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    }
    let w = (run_width(text, px).ceil() as u32 + 4).max(1);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    draw_run(&mut rgba, w, h, 2.0, px, text, ink);
    UndoFace { rgba, w, h }
}

/// Text in Inter shrunk from `px` toward `min_px` until it fits `max_w`.
pub fn inter_fit_face(
    text: &str,
    px: f32,
    min_px: f32,
    h: u32,
    ink: [u8; 3],
    max_w: f32,
) -> UndoFace {
    let mut at = px;
    while at > min_px && run_width(text, at) > max_w {
        at -= 1.0;
    }
    inter_text_face(text, at, h, ink)
}

/// A plain rounded block, for the bar behind a selected row.
pub fn socket_bar_face(w: u32, h: u32) -> UndoFace {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    rounded_rect(
        &mut rgba,
        w,
        h,
        0.0,
        0.0,
        w as f32,
        h as f32,
        h as f32 * 0.3,
        [255, 255, 255],
        1.0,
    );
    UndoFace { rgba, w, h }
}

/// How wide a run of text sets at `px`.
pub fn run_width(text: &str, px: f32) -> f32 {
    let mut w = 0.0;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let Some(font) = font_for(c) else {
            continue;
        };
        if let (Some(p), Some(inter)) = (prev, inter_font()) {
            if std::ptr::eq(font, inter) {
                w += inter.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
        }
        w += font.metrics(c, px).advance_width;
        prev = Some(c);
    }
    w
}

/// Sets a run of text, its x-height centred in the face's height.
pub(crate) fn draw_run(dst: &mut [u8], w: u32, h: u32, x: f32, px: f32, text: &str, ink: [u8; 3]) {
    let Some(inter) = inter_font() else {
        return;
    };
    let cap = inter.metrics('H', px).height as f32;
    let baseline = ((h as f32 + cap) / 2.0).round();
    let mut pen = x;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        let Some(font) = font_for(c) else {
            continue;
        };
        if let Some(p) = prev {
            if std::ptr::eq(font, inter) {
                pen += inter.horizontal_kern(p, c, px).unwrap_or(0.0);
            }
        }
        let (m, cov) = font.rasterize(c, px);
        let gx = (pen + m.xmin as f32).round() as i32;
        let gy = baseline as i32 - m.height as i32 - m.ymin;
        for yy in 0..m.height {
            for xx in 0..m.width {
                let a = cov[yy * m.width + xx] as f32 / 255.0;
                blend(dst, w, h, gx + xx as i32, gy + yy as i32, ink, a);
            }
        }
        pen += m.advance_width;
        prev = Some(c);
    }
}

/// Lays `rgb` at `a` over one pixel of a straight-alpha buffer.
fn blend(dst: &mut [u8], w: u32, h: u32, x: i32, y: i32, rgb: [u8; 3], a: f32) {
    if a <= 0.0 || x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return;
    }
    let at = ((y as u32 * w + x as u32) * 4) as usize;
    let px = &mut dst[at..at + 4];
    let da = px[3] as f32 / 255.0;
    let out_a = a + da * (1.0 - a);
    if out_a <= 0.0 {
        return;
    }
    for i in 0..3 {
        let c = (rgb[i] as f32 * a + px[i] as f32 * da * (1.0 - a)) / out_a;
        px[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    px[3] = (out_a * 255.0).round() as u8;
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rounded_rect(
    dst: &mut [u8],
    w: u32,
    h: u32,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r: f32,
    rgb: [u8; 3],
    a: f32,
) {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
    for y in y0.floor().max(0.0) as i32..(y1.ceil() as i32).min(h as i32) {
        for x in x0.floor().max(0.0) as i32..(x1.ceil() as i32).min(w as i32) {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            let qx = (cx - (x0 + x1) / 2.0).abs() - ((x1 - x0) / 2.0 - r);
            let qy = (cy - (y0 + y1) / 2.0).abs() - ((y1 - y0) / 2.0 - r);
            let outside =
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
            let cover = (0.5 - outside).clamp(0.0, 1.0);
            blend(dst, w, h, x, y, rgb, a * cover);
        }
    }
}

static TINT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Colours the Socket look's greys: neutral, pastel or sepia.
pub fn set_socket_tint(tint: slot_store::Tint) {
    let i = slot_store::Tint::ALL
        .iter()
        .position(|&t| t == tint)
        .unwrap_or(0);
    TINT.store(i as u8, std::sync::atomic::Ordering::Relaxed);
}

pub fn socket_tint() -> slot_store::Tint {
    let i = TINT.load(std::sync::atomic::Ordering::Relaxed) as usize;
    slot_store::Tint::ALL
        .get(i)
        .copied()
        .unwrap_or(slot_store::Tint::Default)
}

/// The colour a grey of the Socket look takes under the chosen tint: light
/// greys turn to the tint's paper, dark ones stay dark, and colours (the red
/// close pill) stay as they are.
pub fn tinted(grey: [u8; 3]) -> [u8; 3] {
    let paper: [f32; 3] = match socket_tint() {
        slot_store::Tint::Default => return grey,
        slot_store::Tint::Pastel => [252.0, 226.0, 255.0],
        slot_store::Tint::Sepia => [255.0, 233.0, 196.0],
    };
    if grey[0] != grey[1] || grey[1] != grey[2] {
        return grey;
    }
    let g = grey[0] as f32;
    paper.map(|p| (g / 255.0 * p).round().clamp(0.0, 255.0) as u8)
}

/// The backdrop: mid grey, lighter behind the carts and darker toward the
/// bottom, under a fine grid of pale dots.
pub fn socket_backdrop() -> UndoFace {
    let (w, h) = (OUT_W, OUT_H);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut shades = [[0u8; 3]; 256];
    for (i, shade) in shades.iter_mut().enumerate() {
        *shade = tinted([i as u8; 3]);
    }
    let paint = |c: [u8; 3]| shades[c[0] as usize];
    const PITCH: f32 = 10.0;
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let v = fy / h as f32;
            let mut g = 183.0 - 26.0 * ((v - 0.72) / 0.28).max(0.0).powi(2);
            let (dx, dy) = ((fx - OUT_W as f32 / 2.0) / 300.0, (fy - ROW_Y) / 170.0);
            g += 22.0 * (-(dx * dx + dy * dy)).exp();
            let (gx, gy) = (
                fx.rem_euclid(PITCH) - PITCH / 2.0,
                fy.rem_euclid(PITCH) - PITCH / 2.0,
            );
            let d = (gx * gx + gy * gy).sqrt();
            let dot = (1.4 - d).clamp(0.0, 1.0);
            g += 26.0 * dot * (1.0 - 0.5 * v);
            let c = g.round().clamp(0.0, 255.0) as u8;
            let [r, g, b] = paint([c, c, c]);
            let at = ((y * w + x) * 4) as usize;
            rgba[at..at + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    UndoFace { rgba, w, h }
}

/// The four corners of a `w` by `h` card centred on (`cx`, `cy`), turned
/// by `yaw` about its upright, `pitch` about its width and `roll` in the
/// screen's plane, then seen in perspective; and one over each corner's
/// depth for the texture to follow.
pub fn tilt(
    (cx, cy): (f32, f32),
    (w, h): (f32, f32),
    yaw: f32,
    pitch: f32,
    roll: f32,
) -> ([[f32; 2]; 4], [f32; 4]) {
    let eye = EYE * w.max(h);
    let (sy, cyw) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let (sr, cr) = roll.sin_cos();
    let mut corners = [[0.0; 2]; 4];
    let mut q = [1.0; 4];
    for (i, (lx, ly)) in [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)]
        .into_iter()
        .enumerate()
    {
        let (x, y, z) = (lx * w, ly * h, 0.0f32);
        let (x, z) = (x * cyw + z * sy, -x * sy + z * cyw);
        let (y, z) = (y * cp - z * sp, y * sp + z * cp);
        let (x, y) = (x * cr - y * sr, x * sr + y * cr);
        let k = eye / (eye + z);
        corners[i] = [cx + x * k, cy + y * k];
        q[i] = k;
    }
    (corners, q)
}

/// The selected cart's swing `ms` into its sway: yaw, pitch and roll.
pub fn sway(ms: u64) -> (f32, f32, f32) {
    let t = ms as f32 / 1000.0;
    let wave = |(amp, period): (f32, f32), phase: f32| {
        amp * (t * std::f32::consts::TAU / period + phase).sin()
    };
    (wave(YAW, 0.0), wave(PITCH, 1.0), -wave(ROLL, 0.6))
}

fn fit((bw, bh): (f32, f32), (w, h): (u32, u32)) -> (f32, f32) {
    let k = (bw / w as f32).min(bh / h as f32);
    (w as f32 * k, h as f32 * k)
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Shelf {
    /// The carousel as Socket draws it. `ms` drives the sway; `zoom` runs
    /// from 0 to 1 as the selected cart is played, swelling it while the
    /// others fall away.
    /// The selected cart alone, full size and swaying, centred on `cx`.
    pub fn draw_socket_solo(&self, ms: u64, cx: f32, out: &mut Vec<Draw>) {
        let Some(cart) = self.carts.get(self.index) else {
            return;
        };
        let (yaw, pitch, roll) = sway(ms);
        let wh = fit(MAIN_BOX, cart_box(cart.platform));
        let (corners, q) = tilt((cx, ROW_Y), wh, yaw, pitch, roll);
        if let Some(tex) = self.face(self.index) {
            out.push(Draw::Warp {
                corners,
                q,
                tex,
                alpha: 1.0,
                grey: 0.0,
            });
        }
    }

    pub fn draw_socket(&self, ms: u64, zoom: f32, out: &mut Vec<Draw>) {
        let zoom = zoom.clamp(0.0, 1.0);
        let target = self.scroll_target();
        let mut placed = Vec::new();
        for slot in -3..=3 {
            let Some(i) = self.cart_at_offset(slot) else {
                continue;
            };
            let offset = target + slot as f32 - self.scroll;
            placed.push((i, offset));
        }
        placed.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        let (yaw, pitch, roll) = sway(ms);
        for (i, offset) in placed {
            let cart = &self.carts[i];
            let near = 1.0 - offset.abs().min(1.0);
            let size = cart_box(cart.platform);
            let main = fit(MAIN_BOX, size);
            let side = fit(SIDE_BOX, size);
            let grow = 1.0 + 0.7 * zoom * near;
            let wh = (
                mix(side.0, main.0, near) * grow,
                mix(side.1, main.1, near) * grow,
            );
            let cx = OUT_W as f32 / 2.0 + offset * SPACING;
            if cx + wh.0 < 0.0 || cx - wh.0 > OUT_W as f32 {
                continue;
            }
            let alpha = mix(SIDE_ALPHA, 1.0, near) * (1.0 - zoom * (1.0 - near * 0.4));
            let grey = SIDE_GREY * (1.0 - near);
            let (corners, q) = tilt(
                (cx, ROW_Y),
                wh,
                yaw * near * (1.0 - zoom),
                pitch * near * (1.0 - zoom),
                roll * near * (1.0 - zoom),
            );
            match self.face(i) {
                Some(tex) => out.push(Draw::Warp {
                    corners,
                    q,
                    tex,
                    alpha,
                    grey,
                }),
                None => {
                    let c = label_colour(&label_text(cart));
                    out.push(Draw::Rect {
                        x: cx - wh.0 / 2.0,
                        y: ROW_Y - wh.1 / 2.0,
                        w: wh.0,
                        h: wh.1,
                        colour: [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            alpha,
                        ],
                    });
                }
            }
        }
    }
}

pub struct SocketScreen<'a> {
    pub backdrop: Option<TexId>,
    pub faces: &'a [Option<(TexId, u32, u32)>],
    /// How far the pills have faded, as a game is played.
    pub fade: f32,
}

impl SocketScreen<'_> {
    fn face(&self, slot: usize) -> Option<(TexId, f32, f32)> {
        self.faces
            .get(slot)
            .copied()
            .flatten()
            .map(|(t, w, h)| (t, w as f32, h as f32))
    }

    fn put(&self, out: &mut Vec<Draw>, slot: usize, x: f32, y: f32) -> f32 {
        let Some((tex, w, h)) = self.face(slot) else {
            return 0.0;
        };
        out.push(Draw::Tex {
            x: x.round(),
            y: y.round(),
            w,
            h,
            tex,
            alpha: 1.0 - self.fade,
        });
        w
    }

    fn width(&self, slot: usize) -> f32 {
        self.face(slot).map_or(0.0, |(_, w, _)| w)
    }

    pub fn draw_backdrop(&self, out: &mut Vec<Draw>) {
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
                colour: [0.72, 0.72, 0.72, 1.0],
            }),
        }
    }

    /// The game's name alone, centred on `cx`.
    pub fn draw_name_at(&self, cx: f32, out: &mut Vec<Draw>) {
        let name = self.width(SOCKET_NAME);
        self.put(out, SOCKET_NAME, cx - name / 2.0, NAME_Y);
    }

    /// Everything but the carts: the bar along the top, the game's name and
    /// the controls along the bottom.
    /// The clock alone, top left.
    pub fn draw_clock(&self, out: &mut Vec<Draw>) {
        self.put(out, SOCKET_CLOCK, EDGE, TOP_Y);
    }

    pub fn draw_pills(&self, out: &mut Vec<Draw>) {
        self.draw_clock(out);
        let status = self.width(SOCKET_STATUS);
        self.put(out, SOCKET_STATUS, OUT_W as f32 - EDGE - status, TOP_Y);

        let tabs: Vec<usize> = (0..SOCKET_TABS)
            .map(socket_tab_slot)
            .filter(|s| self.width(*s) > 0.0)
            .collect();
        let row = [SOCKET_L]
            .into_iter()
            .chain(tabs)
            .chain([SOCKET_R])
            .filter(|s| self.width(*s) > 0.0)
            .collect::<Vec<_>>();
        let total: f32 = row.iter().map(|s| self.width(*s)).sum::<f32>()
            + GAP * row.len().saturating_sub(1) as f32;
        let mut x = (OUT_W as f32 - total) / 2.0;
        for s in row {
            x += self.put(out, s, x, TOP_Y) + GAP;
        }

        let name = self.width(SOCKET_NAME);
        self.put(out, SOCKET_NAME, (OUT_W as f32 - name) / 2.0, NAME_Y);

        let mut x = EDGE;
        for i in 0..SOCKET_BOTTOM - 1 {
            let w = self.put(out, socket_bottom_slot(i), x, BOTTOM_Y);
            if w > 0.0 {
                x += w + GAP + 2.0;
            }
        }
        let last = socket_bottom_slot(SOCKET_BOTTOM - 1);
        let w = self.width(last);
        self.put(out, last, OUT_W as f32 - EDGE - w, BOTTOM_Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tint_colours_the_greys_and_leaves_the_red_alone() {
        let red = [223, 77, 83];
        set_socket_tint(slot_store::Tint::Sepia);
        let paper = tinted([220, 220, 220]);
        let kept = tinted(red);
        set_socket_tint(slot_store::Tint::Default);
        assert!(paper[0] > paper[2], "sepia is not warm: {paper:?}");
        assert_eq!(kept, red);
        assert_eq!(tinted([220, 220, 220]), [220, 220, 220]);
    }

    #[test]
    fn a_flat_card_keeps_its_box() {
        let (c, q) = tilt((100.0, 50.0), (40.0, 20.0), 0.0, 0.0, 0.0);
        assert_eq!(
            c,
            [[80.0, 40.0], [120.0, 40.0], [80.0, 60.0], [120.0, 60.0]]
        );
        assert_eq!(q, [1.0; 4]);
    }

    #[test]
    fn a_turned_card_grows_the_edge_that_swings_near() {
        let (c, q) = tilt((0.0, 0.0), (100.0, 60.0), 0.3, 0.0, 0.0);
        let left = c[2][1] - c[0][1];
        let right = c[3][1] - c[1][1];
        assert!(right > left, "the near edge is not the taller");
        assert!(q[1] > q[0]);
    }

    #[test]
    fn the_sway_stays_gentle() {
        for ms in (0..20_000).step_by(37) {
            let (y, p, r) = sway(ms);
            assert!(
                y.abs() <= YAW.0 + 1e-6 && p.abs() <= PITCH.0 + 1e-6 && r.abs() <= ROLL.0 + 1e-6
            );
        }
    }

    #[test]
    fn a_pill_is_its_slots_height_and_hugs_its_text() {
        let short = socket_pill_face(SOCKET_NAME, "A\u{1}Go", true);
        let long = socket_pill_face(SOCKET_NAME, "A\u{1}Pokémon Emerald Version", true);
        assert_eq!((short.h, long.h), (NAME_H, NAME_H));
        assert!(long.w > short.w + 100);
        assert!(
            long.rgba.chunks(4).any(|p| p[3] == 255),
            "no solid ink or ground"
        );
    }

    #[test]
    fn an_unlit_tab_is_text_alone() {
        let tab = socket_pill_face(socket_tab_slot(0), "GBC", false);
        let corner = &tab.rgba[0..4];
        assert_eq!(corner[3], 0, "an unlit tab has a ground");
        let lit = socket_pill_face(socket_tab_slot(0), "GBC", true);
        let mid = ((lit.h / 2) * lit.w + 2) as usize * 4;
        assert!(lit.rgba[mid + 3] > 0, "a lit tab has no ground");
    }

    #[test]
    fn the_status_pill_grows_for_wifi_and_shows_its_charge() {
        let text = Status {
            percent: Some(72),
            charging: true,
            wifi: true,
        }
        .text();
        assert_eq!(text, "72+w");
        assert_eq!(
            Status::parse(&text),
            Status {
                percent: Some(72),
                charging: true,
                wifi: true
            }
        );
        let with = socket_pill_face(SOCKET_STATUS, "72w", true);
        let without = socket_pill_face(SOCKET_STATUS, "72", true);
        assert!(with.w > without.w);
        let ink = |f: &UndoFace| {
            f.rgba
                .chunks(4)
                .filter(|p| p[0] > 200 && p[3] > 200)
                .count()
        };
        assert!(ink(&without) > 20, "no battery drawn");
        assert!(ink(&with) > ink(&without) + 10, "no wifi drawn");
        let full = ink(&socket_pill_face(SOCKET_STATUS, "100", true));
        let low = ink(&socket_pill_face(SOCKET_STATUS, "20", true));
        assert!(full > low, "the battery does not fill with its charge");
        let numbered = socket_pill_face(SOCKET_STATUS, "72", true);
        let unknown = socket_pill_face(SOCKET_STATUS, "-", true);
        assert!(numbered.w > unknown.w, "the charge is not written out");
    }

    #[test]
    fn a_full_charging_status_with_wifi_stays_clear_of_the_tabs() {
        let w = |slot, t: &str| socket_pill_face(slot, t, false).w as f32;
        let tabs = [
            w(SOCKET_L, "L"),
            w(socket_tab_slot(0), "GBA"),
            w(socket_tab_slot(1), "GB"),
            w(socket_tab_slot(2), "GBC"),
            w(socket_tab_slot(3), "\u{2605} FAV"),
            w(SOCKET_R, "R"),
        ];
        let total = tabs.iter().sum::<f32>() + GAP * (tabs.len() - 1) as f32;
        let tabs_end = (OUT_W as f32 + total) / 2.0;
        let status_x = OUT_W as f32 - EDGE - w(SOCKET_STATUS, "100+w");
        assert!(status_x > tabs_end + GAP, "{status_x} vs {tabs_end}");
    }

    #[test]
    fn every_screens_split_falls_between_the_dots() {
        let b = socket_backdrop();
        let at = |x: u32, y: u32| b.rgba[((y * b.w + x) * 4) as usize];
        for (name, split) in [
            ("shelf", SOCKET_SPLIT),
            ("settings", crate::socket_menu::SETTINGS_SPLIT),
            ("search", crate::search::SEARCH_SPLIT),
            ("home", crate::home_panel::HOME_SPLIT),
        ] {
            let y = split as u32;
            assert!(
                (1..OUT_W).all(|x| at(x, y).abs_diff(at(x - 1, y)) <= 1),
                "the {name} split at {y} runs through a row of dots, which would stretch into stripes"
            );
        }
    }

    #[test]
    fn empty_text_draws_nothing() {
        assert_eq!(socket_pill_face(SOCKET_CLOCK, "", true).w, 0);
    }

    #[test]
    fn the_backdrop_is_grey_with_paler_dots() {
        let b = socket_backdrop();
        assert_eq!((b.w, b.h), (OUT_W, OUT_H));
        let at = |x: u32, y: u32| b.rgba[((y * b.w + x) * 4) as usize];
        let dot = at(5, 5);
        let between = at(0, 0);
        assert!(dot > between, "no dot at a grid point");
        assert!((150..230).contains(&between));
        assert!(at(0, OUT_H - 1) < at(0, 100), "the bottom is no darker");
        let split = SOCKET_SPLIT as u32;
        assert!(
            (1..OUT_W).all(|x| at(x, split).abs_diff(at(x - 1, split)) <= 1),
            "the split row carries dots, which would stretch into stripes"
        );
        assert!(NAME_Y + NAME_H as f32 <= SOCKET_SPLIT && SOCKET_SPLIT <= BOTTOM_Y);
    }
}
