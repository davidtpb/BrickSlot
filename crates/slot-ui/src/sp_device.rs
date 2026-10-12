//! The Socket look's handheld: an open clamshell in cobalt blue, after the
//! Game Boy Advance SP, that a cart slides up into from below before the
//! view zooms into its screen and the game starts.
//!
//! It is drawn here, lit like a moulded thing: bevelled edges catching a
//! light from the top left, domed buttons, a glossy recessed bezel and the
//! base's thickness below it. The lid leans back in perspective and comes
//! upright as the view goes in. Its screen is 3:2, the shape of the frame
//! the game fills, so at the end of the zoom the screen covers the output
//! exactly where the game will play.
//!
//! Sizes are in device units, a device 300 wide and 452 tall; the
//! textures are drawn at twice that for a sharp zoom.

use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::UndoFace;

pub const SP_W: u32 = 300;
pub const SP_H: u32 = 452;
/// Texture pixels to a device unit.
const RES: f32 = 2.0;

/// The screen inside the lid, in device units: x, y, w, h.
pub const SP_SCREEN: (f32, f32, f32, f32) = (48.0, 30.0, 204.0, 136.0);
/// The power light on the hinge.
pub const SP_LED: (f32, f32) = (34.0, 219.0);
/// Where the cart goes in: the middle of the bottom edge.
pub const SP_SLOT: (f32, f32) = (150.0, 446.0);
/// How wide a cart is next to the device.
pub const SP_CART_W: f32 = 118.0;

/// The lid's texture covers this much of the device from the top, its
/// bottom edge the hinge it leans back on.
const LID_H: f32 = 208.0;
/// The base's texture starts here and runs to the bottom.
const BASE_Y: f32 = 198.0;
/// How far back the lid leans at rest, in radians.
const LEAN: f32 = 0.32;

const BODY: [f32; 3] = [0.17, 0.29, 0.66];
const BODY_SIDE: [f32; 3] = [0.08, 0.14, 0.36];
const HINGE: [f32; 3] = [0.13, 0.22, 0.52];
const BEZEL: [f32; 3] = [0.05, 0.05, 0.07];
const LCD_OFF: [f32; 3] = [0.10, 0.12, 0.11];
const BUTTON: [f32; 3] = [0.15, 0.15, 0.19];
const SLOT: [f32; 3] = [0.03, 0.04, 0.09];

const LIGHT: [f32; 3] = [-0.38, -0.70, 0.60];

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn rounded(p: (f32, f32), (x, y, w, h): (f32, f32, f32, f32), r: f32) -> f32 {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let qx = (p.0 - cx).abs() - (w / 2.0 - r);
    let qy = (p.1 - cy).abs() - (h / 2.0 - r);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn disc(p: (f32, f32), (cx, cy): (f32, f32), r: f32) -> f32 {
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() - r
}

/// How a part of the device is made.
#[derive(Copy, Clone)]
struct Material {
    colour: [f32; 3],
    /// How wide its rounded edge is, in device units.
    bevel: f32,
    /// How glossy: the highlight's strength and tightness.
    shine: f32,
    tight: f32,
    /// Sunk into the surface rather than raised from it.
    recessed: bool,
}

const SHELL: Material = Material {
    colour: BODY,
    bevel: 11.0,
    shine: 0.45,
    tight: 28.0,
    recessed: false,
};

/// The height of a surface at a point, from its distance to the edge.
fn height(d: f32, m: &Material) -> f32 {
    let t = (-d / m.bevel).clamp(0.0, 1.0);
    let h = (1.0 - (1.0 - t) * (1.0 - t)).sqrt() * m.bevel * 0.7;
    if m.recessed {
        -h
    } else {
        h
    }
}

/// Lights a surface at `p` (device units) whose edge distance is `sdf`,
/// tipped by `swell` for the gentle curve across a whole part.
fn lit(p: (f32, f32), sdf: &dyn Fn((f32, f32)) -> f32, m: &Material, swell: f32) -> [f32; 3] {
    let e = 0.5;
    let hx = height(sdf((p.0 + e, p.1)), m) - height(sdf((p.0 - e, p.1)), m);
    let hy = height(sdf((p.0, p.1 + e)), m) - height(sdf((p.0, p.1 - e)), m);
    let n = norm([-hx / (2.0 * e), -hy / (2.0 * e) + swell, 1.0]);
    let l = norm(LIGHT);
    let half = norm([l[0], l[1], l[2] + 1.0]);
    let diffuse = dot(n, l).max(0.0);
    let spec = dot(n, half).max(0.0).powf(m.tight) * m.shine;
    m.colour.map(|c| c * (0.42 + 0.78 * diffuse) + spec)
}

/// How a part's surface tips, top to bottom, for its swell.
fn swell_at(p: (f32, f32), part: (f32, f32, f32, f32)) -> f32 {
    ((p.1 - (part.1 + part.3 / 2.0)) / part.3) * 0.35
}

struct Canvas {
    w: u32,
    h: u32,
    /// Where the canvas starts down the device, in device units.
    y0: f32,
    rgba: Vec<f32>,
}

impl Canvas {
    fn new(w: f32, h: f32, y0: f32) -> Canvas {
        let (w, h) = ((w * RES) as u32, (h * RES) as u32);
        Canvas {
            w,
            h,
            y0,
            rgba: vec![0.0; (w * h * 4) as usize],
        }
    }

    fn at(&self, x: u32, y: u32) -> (f32, f32) {
        ((x as f32 + 0.5) / RES, (y as f32 + 0.5) / RES + self.y0)
    }

    /// Paints a part: `shape` gives each point's signed distance to its
    /// edge, `colour` its colour there.
    fn paint(
        &mut self,
        shape: &dyn Fn((f32, f32)) -> f32,
        colour: &dyn Fn((f32, f32)) -> [f32; 3],
        opacity: f32,
    ) {
        for y in 0..self.h {
            for x in 0..self.w {
                let p = self.at(x, y);
                let d = shape(p) * RES;
                let a = (0.5 - d).clamp(0.0, 1.0) * opacity;
                if a <= 0.0 {
                    continue;
                }
                let c = colour(p);
                let at = ((y * self.w + x) * 4) as usize;
                let under = self.rgba[at + 3];
                let out_a = a + under * (1.0 - a);
                for (v, ch) in self.rgba[at..at + 3].iter_mut().zip(c) {
                    *v = (ch * a + *v * under * (1.0 - a)) / out_a.max(1e-6);
                }
                self.rgba[at + 3] = out_a;
            }
        }
    }

    fn part(&mut self, shape: &dyn Fn((f32, f32)) -> f32, m: Material) {
        self.paint(shape, &|p| lit(p, shape, &m, 0.0), 1.0);
    }

    /// A faint diagonal reflection over a glossy part, left off `except`.
    fn sheen(&mut self, shape: &dyn Fn((f32, f32)) -> f32, except: &dyn Fn((f32, f32)) -> bool) {
        for y in 0..self.h {
            for x in 0..self.w {
                let p = self.at(x, y);
                if shape(p) > 0.0 || except(p) {
                    continue;
                }
                let u = (p.0 * 0.55 + p.1) / 260.0;
                let a = (1.0 - ((u - 0.42) / 0.12).abs()).clamp(0.0, 1.0) * 0.10;
                let at = ((y * self.w + x) * 4) as usize;
                for v in &mut self.rgba[at..at + 3] {
                    *v += (1.0 - *v) * a;
                }
            }
        }
    }

    fn face(self) -> UndoFace {
        let rgba = self
            .rgba
            .iter()
            .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect();
        UndoFace {
            rgba,
            w: self.w,
            h: self.h,
        }
    }
}

const LID: (f32, f32, f32, f32) = (10.0, 2.0, 280.0, 204.0);
const INNER: (f32, f32, f32, f32) = (26.0, 16.0, 248.0, 176.0);
const HINGE_BOX: (f32, f32, f32, f32) = (4.0, 203.0, 292.0, 31.0);
const BASE: (f32, f32, f32, f32) = (10.0, 230.0, 280.0, 214.0);
const BASE_SIDE: f32 = 8.0;
const DPAD: (f32, f32) = (82.0, 318.0);
const A_BUTTON: (f32, f32) = (234.0, 300.0);
const B_BUTTON: (f32, f32) = (196.0, 326.0);

/// The lid: shell, glossy black bezel, and the dark screen sunk into it.
pub fn sp_lid_face() -> UndoFace {
    let mut c = Canvas::new(SP_W as f32, LID_H, 0.0);
    let lid = |p| rounded(p, LID, 22.0);
    c.paint(&lid, &|p| lit(p, &lid, &SHELL, swell_at(p, LID)), 1.0);
    let bezel = |p| rounded(p, INNER, 12.0);
    c.part(
        &bezel,
        Material {
            colour: BEZEL,
            bevel: 4.0,
            shine: 0.7,
            tight: 60.0,
            recessed: true,
        },
    );
    let screen = |p| rounded(p, SP_SCREEN, 2.0);
    c.paint(
        &screen,
        &|p: (f32, f32)| {
            // Darker under the bezel's lip at the top and left.
            let from_edge = (p.1 - SP_SCREEN.1).min(p.0 - SP_SCREEN.0);
            let lip = (1.0 - from_edge / 6.0).clamp(0.0, 1.0) * 0.5;
            LCD_OFF.map(|v| v * (1.0 - lip))
        },
        1.0,
    );
    c.sheen(&bezel, &|p| screen(p) < 0.0);
    c.face()
}

/// The base: its thickness, the shell, the hinge, and the controls.
pub fn sp_base_face() -> UndoFace {
    let mut c = Canvas::new(SP_W as f32, SP_H as f32 - BASE_Y, BASE_Y);
    // The base's edge, seen below its face.
    let side = |p: (f32, f32)| rounded((p.0, p.1 - BASE_SIDE), BASE, 22.0);
    c.paint(
        &side,
        &|p: (f32, f32)| {
            let t = ((p.1 - (BASE.1 + BASE.3 - 10.0)) / (BASE_SIDE + 10.0)).clamp(0.0, 1.0);
            BODY_SIDE.map(|v| v * (1.0 - 0.45 * t))
        },
        1.0,
    );
    let base = |p| rounded(p, BASE, 22.0);
    c.paint(&base, &|p| lit(p, &base, &SHELL, swell_at(p, BASE)), 1.0);
    // The hinge, a cylinder lit along its length.
    let hinge = |p| rounded(p, HINGE_BOX, 15.0);
    c.paint(
        &hinge,
        &|p: (f32, f32)| {
            let v =
                ((p.1 - (HINGE_BOX.1 + HINGE_BOX.3 / 2.0)) / (HINGE_BOX.3 / 2.0)).clamp(-1.0, 1.0);
            let n = norm([0.0, v, (1.0 - v * v).max(0.0).sqrt()]);
            let l = norm(LIGHT);
            let half = norm([l[0], l[1], l[2] + 1.0]);
            let diffuse = dot(n, l).max(0.0);
            let spec = dot(n, half).max(0.0).powf(30.0) * 0.5;
            HINGE.map(|ch| ch * (0.35 + 0.8 * diffuse) + spec)
        },
        1.0,
    );
    // The light's window on the hinge.
    let led = |p| disc(p, SP_LED, 4.5);
    c.part(
        &led,
        Material {
            colour: [0.1, 0.13, 0.11],
            bevel: 2.0,
            shine: 0.6,
            tight: 40.0,
            recessed: true,
        },
    );
    // The D-pad: a bevelled cross in a shallow well, with a dimple.
    let well = |p: (f32, f32)| disc(p, DPAD, 42.0);
    c.part(
        &well,
        Material {
            colour: [0.14, 0.24, 0.56],
            bevel: 6.0,
            shine: 0.2,
            tight: 20.0,
            recessed: true,
        },
    );
    let cross = |p: (f32, f32)| {
        let q = (p.0 - DPAD.0, p.1 - DPAD.1);
        rounded(q, (-35.0, -11.5, 70.0, 23.0), 4.0).min(rounded(q, (-11.5, -35.0, 23.0, 70.0), 4.0))
    };
    let plastic = Material {
        colour: BUTTON,
        bevel: 6.0,
        shine: 0.55,
        tight: 36.0,
        recessed: false,
    };
    c.part(&cross, plastic);
    let dimple = |p| disc(p, DPAD, 6.0);
    c.part(
        &dimple,
        Material {
            recessed: true,
            ..plastic
        },
    );
    // A and B, domed, each in a ring.
    for b in [A_BUTTON, B_BUTTON] {
        let ring = move |p| disc(p, b, 21.0);
        c.part(
            &ring,
            Material {
                colour: [0.12, 0.2, 0.48],
                bevel: 4.0,
                shine: 0.1,
                tight: 20.0,
                recessed: true,
            },
        );
        let dome = move |p| disc(p, b, 17.0);
        c.part(
            &dome,
            Material {
                colour: [0.2, 0.17, 0.27],
                bevel: 17.0,
                shine: 0.75,
                tight: 50.0,
                recessed: false,
            },
        );
    }
    // Start and Select, small pills in slots.
    for sx in [126.0, 174.0] {
        let slot = move |p| rounded(p, (sx - 18.0, 395.0, 36.0, 16.0), 8.0);
        c.part(
            &slot,
            Material {
                colour: [0.11, 0.19, 0.45],
                bevel: 3.0,
                shine: 0.1,
                tight: 20.0,
                recessed: true,
            },
        );
        let pill = move |p| rounded(p, (sx - 14.0, 398.0, 28.0, 10.0), 5.0);
        c.part(
            &pill,
            Material {
                bevel: 5.0,
                ..plastic
            },
        );
    }
    // The cart slot, a dark mouth along the bottom edge.
    let mouth = |p| rounded(p, (SP_SLOT.0 - 64.0, 437.0, 128.0, 9.0), 3.0);
    c.paint(&mouth, &|_| SLOT, 0.9);
    c.face()
}

/// The pale light of a screen switched on.
pub fn sp_glow_face() -> UndoFace {
    let (w, h) = (8, 8);
    UndoFace {
        rgba: [222u8, 230, 214, 255].repeat((w * h) as usize),
        w,
        h,
    }
}

/// The soft shadow the device casts on the stage.
pub fn sp_shadow_face() -> UndoFace {
    let (w, h) = (256u32, 64u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt();
            let a = (1.0 - r).clamp(0.0, 1.0).powf(1.6) * 0.75;
            rgba[((y * w + x) * 4 + 3) as usize] = (a * 255.0) as u8;
        }
    }
    UndoFace { rgba, w, h }
}

/// Every texture of the device, in the order [`draw_sp`] takes them.
pub fn sp_faces() -> Vec<UndoFace> {
    vec![
        sp_lid_face(),
        sp_base_face(),
        sp_glow_face(),
        sp_shadow_face(),
    ]
}

/// Where everything is in one frame of the scene.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SpPose {
    /// How far the view has zoomed into the screen, 0 to 1.
    pub zoom: f32,
    /// How visible the device is, 0 to 1.
    pub alpha: f32,
    /// How far the cart has gone in, 0 (below, clear of the device) to 1.
    pub cart_in: f32,
    pub cart_alpha: f32,
    pub led: bool,
    /// How much the screen is lit, pale.
    pub glow: f32,
    /// How much of the screen is black, for the game to come in from.
    pub black: f32,
}

/// The device's size and top-left on the output at rest.
pub fn sp_rest() -> (f32, f32, f32) {
    let scale = (OUT_H as f32 - 20.0) / SP_H as f32 * 0.86;
    let (w, h) = (SP_W as f32 * scale, SP_H as f32 * scale);
    let x = (OUT_W as f32 - w) / 2.0;
    let y = (OUT_H as f32 - h) / 2.0 - 10.0;
    (scale, x, y)
}

/// The device's scale and top-left with the view `zoom` of the way into
/// its screen: at 1 the screen covers the whole output.
pub fn sp_placed(zoom: f32) -> (f32, f32, f32) {
    let (rest, rx, ry) = sp_rest();
    let full = OUT_W as f32 / SP_SCREEN.2;
    let z = zoom.clamp(0.0, 1.0);
    let scale = (rest.ln() + (full.ln() - rest.ln()) * z).exp();
    let (sx, sy) = (
        SP_SCREEN.0 + SP_SCREEN.2 / 2.0,
        SP_SCREEN.1 + SP_SCREEN.3 / 2.0,
    );
    let from = (rx + sx * rest, ry + sy * rest);
    let to = (OUT_W as f32 / 2.0, OUT_H as f32 / 2.0);
    let k = (scale - rest) / (full - rest);
    let centre = (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k);
    (scale, centre.0 - sx * scale, centre.1 - sy * scale)
}

/// How far back the lid leans at a zoom: all the way at rest, upright by
/// the time the view is half way in.
pub fn sp_lean(zoom: f32) -> f32 {
    LEAN * (1.0 - span(zoom, 0.0, 0.55))
}

/// The device's screen on the output for a zoom, once the lid is upright.
pub fn sp_screen_rect(zoom: f32) -> (f32, f32, f32, f32) {
    let (s, x, y) = sp_placed(zoom);
    (
        x + SP_SCREEN.0 * s,
        y + SP_SCREEN.1 * s,
        SP_SCREEN.2 * s,
        SP_SCREEN.3 * s,
    )
}

/// Where a point of the lid (device units) lands on the output, with the
/// lid leaning back on its hinge, and how near it is, for the texture to
/// follow the perspective.
fn on_lid(p: (f32, f32), zoom: f32) -> ([f32; 2], f32) {
    let (s, x, y) = sp_placed(zoom);
    let lean = sp_lean(zoom);
    let up = (LID_H - p.1) * s;
    let eye = 3.2 * LID_H * s;
    let k = eye / (eye + up * lean.sin());
    let hinge_y = y + LID_H * s;
    let cx = x + SP_W as f32 * s / 2.0;
    (
        [
            cx + (p.0 - SP_W as f32 / 2.0) * s * k,
            hinge_y - up * lean.cos() * k,
        ],
        k,
    )
}

/// A rectangle of the lid as a quad for [`Draw::Warp`].
fn lid_quad(r: (f32, f32, f32, f32), zoom: f32) -> ([[f32; 2]; 4], [f32; 4]) {
    let (x, y, w, h) = r;
    let pts = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)];
    let mut corners = [[0.0; 2]; 4];
    let mut q = [1.0; 4];
    for (i, p) in pts.into_iter().enumerate() {
        let (c, k) = on_lid(p, zoom);
        corners[i] = c;
        q[i] = k;
    }
    (corners, q)
}

/// Draws the scene, for a black stage: the shadow, the cart (behind the
/// base, so the body hides what has gone in), the base, the lid leaning
/// back, its screen's light and the power light. `faces` are
/// [`sp_faces`]' textures.
pub fn draw_sp(
    faces: &[(TexId, u32, u32)],
    cart: Option<(TexId, (f32, f32))>,
    pose: SpPose,
    out: &mut Vec<Draw>,
) {
    let (s, x, y) = sp_placed(pose.zoom);
    if let Some((tex, _, _)) = faces.get(3) {
        let (w, h) = (SP_W as f32 * 1.05 * s, 46.0 * s);
        out.push(Draw::Tex {
            x: x + (SP_W as f32 * s - w) / 2.0,
            y: y + (SP_H as f32 - 18.0) * s,
            w,
            h,
            tex: *tex,
            alpha: pose.alpha,
        });
    }
    if let Some((tex, (cw, ch))) = cart.filter(|_| pose.cart_alpha > 0.0) {
        let k = SP_CART_W / cw;
        let (w, h) = (cw * k * s, ch * k * s);
        let out_top = SP_SLOT.1 + 30.0;
        let in_top = SP_SLOT.1 - ch * k + 14.0;
        let top = out_top + (in_top - out_top) * pose.cart_in.clamp(0.0, 1.0);
        out.push(Draw::Tex {
            x: x + (SP_SLOT.0 - SP_CART_W / 2.0) * s,
            y: y + top * s,
            w,
            h,
            tex,
            alpha: pose.cart_alpha,
        });
    }
    if let Some((tex, w, h)) = faces.get(1) {
        out.push(Draw::Tex {
            x,
            y: y + BASE_Y * s,
            w: *w as f32 / RES * s,
            h: *h as f32 / RES * s,
            tex: *tex,
            alpha: pose.alpha,
        });
    }
    if let Some((tex, _, _)) = faces.first() {
        let (corners, q) = lid_quad((0.0, 0.0, SP_W as f32, LID_H), pose.zoom);
        out.push(Draw::Warp {
            corners,
            q,
            tex: *tex,
            alpha: pose.alpha,
            grey: 0.0,
        });
    }
    if let Some((tex, _, _)) = faces.get(2).filter(|_| pose.glow > 0.0) {
        let (corners, q) = lid_quad(SP_SCREEN, pose.zoom);
        out.push(Draw::Warp {
            corners,
            q,
            tex: *tex,
            alpha: pose.glow * pose.alpha,
            grey: 0.0,
        });
    }
    if pose.black > 0.0 {
        let (lx, ly, lw, lh) = sp_screen_rect(pose.zoom);
        out.push(Draw::Rect {
            x: lx,
            y: ly,
            w: lw,
            h: lh,
            colour: [0.0, 0.0, 0.0, pose.black],
        });
    }
    if pose.led && pose.alpha > 0.0 {
        let r = 3.5 * s;
        let (cx, cy) = (x + SP_LED.0 * s, y + SP_LED.1 * s);
        out.push(Draw::Rect {
            x: cx - r * 2.0,
            y: cy - r * 2.0,
            w: 4.0 * r,
            h: 4.0 * r,
            colour: [0.3, 1.0, 0.4, 0.18 * pose.alpha],
        });
        out.push(Draw::Rect {
            x: cx - r,
            y: cy - r,
            w: 2.0 * r,
            h: 2.0 * r,
            colour: [0.45, 1.0, 0.55, pose.alpha],
        });
    }
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn span(t: f32, from: f32, to: f32) -> f32 {
    smooth((t - from) / (to - from))
}

/// The insert, `t` seconds in: shelf fade, device up, cart in, power on,
/// zoom.
pub const SP_FADE_S: f32 = 0.28;
const SP_RISE_S: f32 = 0.58;
pub const SP_SEATED_S: f32 = 0.95;
const SP_POWER_S: f32 = 1.12;
pub const SP_INSERT_S: f32 = 1.8;

/// The shelf's fade to black over the first moments of the insert.
pub fn sp_shelf_dark(t: f32) -> f32 {
    span(t, 0.0, SP_FADE_S)
}

pub fn sp_insert_pose(t: f32) -> SpPose {
    let alpha = span(t, SP_FADE_S - 0.05, SP_RISE_S);
    let lit = span(t, SP_SEATED_S, SP_POWER_S);
    let zoom = span(t, SP_POWER_S + 0.05, SP_INSERT_S - 0.05);
    SpPose {
        zoom,
        alpha,
        cart_in: span(t, SP_RISE_S, SP_SEATED_S),
        cart_alpha: alpha,
        led: t >= SP_SEATED_S,
        glow: lit * 0.92,
        // The pale screen dims to black as it fills the view, for the game
        // to fade in from.
        black: span(zoom, 0.55, 1.0),
    }
}

/// The eject, `t` seconds in: zoom out, power off, cart out, fade.
pub const SP_EJECT_S: f32 = 1.25;
pub const SP_RELEASE_S: f32 = 0.62;
/// From here the shelf comes back.
pub const SP_SHELF_BACK_S: f32 = 1.05;

pub fn sp_eject_pose(t: f32) -> SpPose {
    let zoom = 1.0 - span(t, 0.0, 0.5);
    let alpha = 1.0 - span(t, 0.88, SP_SHELF_BACK_S);
    SpPose {
        zoom,
        alpha,
        cart_in: 1.0 - span(t, SP_RELEASE_S, 0.9),
        cart_alpha: alpha,
        led: t < 0.55,
        glow: 0.0,
        black: 1.0 - span(t, 0.2, 0.5),
    }
}

/// The shelf's fade back in at the end of the eject.
pub fn sp_shelf_back(t: f32) -> f32 {
    span(t, SP_SHELF_BACK_S, SP_EJECT_S)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_zoom_puts_the_upright_screen_over_the_whole_output() {
        let (x, y, w, h) = sp_screen_rect(1.0);
        assert!(x.abs() < 0.5 && y.abs() < 0.5, "{x} {y}");
        assert!((w - OUT_W as f32).abs() < 0.5 && (h - OUT_H as f32).abs() < 0.5);
        assert_eq!(sp_lean(1.0), 0.0);
        let (corners, q) = lid_quad(SP_SCREEN, 1.0);
        assert!(corners[0][0].abs() < 0.5 && corners[0][1].abs() < 0.5);
        assert!((corners[3][0] - OUT_W as f32).abs() < 0.5);
        assert_eq!(q, [1.0; 4]);
    }

    #[test]
    fn at_rest_the_lid_leans_back_and_the_device_fits() {
        let (s, x, y) = sp_placed(0.0);
        assert!(x > 0.0 && y > 0.0);
        assert!(x + SP_W as f32 * s < OUT_W as f32);
        assert!(y + SP_H as f32 * s < OUT_H as f32);
        let (corners, q) = lid_quad((0.0, 0.0, SP_W as f32, LID_H), 0.0);
        let top = corners[1][0] - corners[0][0];
        let bottom = corners[3][0] - corners[2][0];
        assert!(top < bottom, "the lid's far edge is not narrower");
        assert!(q[0] < 1.0 && q[2] == 1.0);
    }

    #[test]
    fn the_insert_runs_in_order_and_ends_zoomed_in_and_dark() {
        let start = sp_insert_pose(0.0);
        assert_eq!((start.alpha, start.zoom, start.cart_in), (0.0, 0.0, 0.0));
        let seated = sp_insert_pose(SP_SEATED_S);
        assert!(seated.cart_in > 0.99 && seated.zoom == 0.0 && seated.led);
        let end = sp_insert_pose(SP_INSERT_S);
        assert!(end.zoom > 0.99 && end.black > 0.99);
    }

    #[test]
    fn the_eject_runs_the_insert_backwards() {
        let start = sp_eject_pose(0.0);
        assert!(start.zoom > 0.99 && start.cart_in > 0.99 && start.black > 0.99);
        let out = sp_eject_pose(0.95);
        assert!(out.zoom == 0.0 && out.cart_in < 0.01 && !out.led);
        assert_eq!(sp_eject_pose(SP_EJECT_S).alpha, 0.0);
    }

    #[test]
    fn the_shell_is_lit_from_the_top_left() {
        let base = sp_base_face();
        let px = |x: f32, y: f32| {
            let (tx, ty) = ((x * RES) as u32, ((y - BASE_Y) * RES) as u32);
            let i = ((ty * base.w + tx) * 4) as usize;
            [
                base.rgba[i],
                base.rgba[i + 1],
                base.rgba[i + 2],
                base.rgba[i + 3],
            ]
        };
        let top_edge = px(150.0, BASE.1 + 3.0);
        let bottom_edge = px(150.0, BASE.1 + BASE.3 - 3.0);
        assert!(top_edge[2] > bottom_edge[2], "{top_edge:?} {bottom_edge:?}");
        assert_eq!(px(1.0, BASE.1 + 1.0)[3], 0, "the corner is not clear");
        let mid = px(150.0, 280.0);
        assert!(mid[2] > mid[0], "the body is not blue: {mid:?}");
    }
}
