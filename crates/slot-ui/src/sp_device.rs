//! The Socket look's handheld: an open clamshell in cobalt blue, after the
//! Game Boy Advance SP, that a cart slides up into from below before the
//! view zooms into its screen and the game starts.
//!
//! The device is one texture drawn here. Its screen is 3:2, the shape of
//! the frame the game fills, so zooming until the screen covers the output
//! leaves the game exactly where it will play.

use crate::draw::{Draw, TexId, OUT_H, OUT_W};
use crate::plate::UndoFace;

pub const SP_W: u32 = 300;
pub const SP_H: u32 = 452;

/// The screen inside the top half, in texture pixels: x, y, w, h.
pub const SP_SCREEN: (f32, f32, f32, f32) = (48.0, 30.0, 204.0, 136.0);
/// The power light on the hinge.
pub const SP_LED: (f32, f32) = (34.0, 219.0);
/// Where the cart goes in: the middle of the bottom edge.
pub const SP_SLOT: (f32, f32) = (150.0, 448.0);
/// How wide a cart is next to the device.
pub const SP_CART_W: f32 = 118.0;

const BODY: [u8; 3] = [46, 72, 156];
const BODY_LIGHT: [u8; 3] = [74, 104, 190];
const BODY_DARK: [u8; 3] = [30, 48, 112];
const BEZEL: [u8; 3] = [18, 18, 24];
const LCD_OFF: [u8; 3] = [30, 34, 32];
const BUTTON: [u8; 3] = [32, 32, 38];
const BUTTON_LIGHT: [u8; 3] = [70, 70, 80];

fn rounded(px: (f32, f32), (x, y, w, h): (f32, f32, f32, f32), r: f32) -> f32 {
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let qx = (px.0 - cx).abs() - (w / 2.0 - r);
    let qy = (px.1 - cy).abs() - (h / 2.0 - r);
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
}

fn disc(px: (f32, f32), (cx, cy): (f32, f32), r: f32) -> f32 {
    ((px.0 - cx).powi(2) + (px.1 - cy).powi(2)).sqrt() - r
}

fn put(rgba: &mut [u8], at: usize, rgb: [u8; 3], a: f32) {
    if a <= 0.0 {
        return;
    }
    let under = rgba[at + 3] as f32 / 255.0;
    let out_a = a + under * (1.0 - a);
    for c in 0..3 {
        let mixed = rgb[c] as f32 * a + rgba[at + c] as f32 * under * (1.0 - a);
        rgba[at + c] = (mixed / out_a.max(1e-6)).round().clamp(0.0, 255.0) as u8;
    }
    rgba[at + 3] = (out_a * 255.0).round() as u8;
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// Coverage of a shape whose signed distance is `d` pixels.
fn cover(d: f32) -> f32 {
    (0.5 - d).clamp(0.0, 1.0)
}

const TOP: (f32, f32, f32, f32) = (10.0, 2.0, 280.0, 204.0);
const HINGE: (f32, f32, f32, f32) = (4.0, 204.0, 292.0, 30.0);
const BOTTOM: (f32, f32, f32, f32) = (10.0, 232.0, 280.0, 216.0);
const INNER: (f32, f32, f32, f32) = (26.0, 16.0, 248.0, 176.0);
const DPAD: (f32, f32) = (82.0, 318.0);
const A_BUTTON: (f32, f32) = (234.0, 300.0);
const B_BUTTON: (f32, f32) = (196.0, 326.0);

/// The device, lit from above.
pub fn sp_face() -> UndoFace {
    let (w, h) = (SP_W, SP_H);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let at = ((y * w + x) * 4) as usize;
            // The two halves, shaded light at the top of each.
            for half in [TOP, BOTTOM] {
                let d = rounded(p, half, 22.0);
                let shade = (p.1 - half.1) / half.3;
                let body = mix(BODY_LIGHT, BODY, shade * 1.6);
                let rim = mix(BODY_LIGHT, body, (-d / 3.0).clamp(0.0, 1.0));
                put(&mut rgba, at, rim, cover(d));
            }
            // The hinge, a darker cylinder between them.
            let d = rounded(p, HINGE, 13.0);
            let across = ((p.1 - HINGE.1) / HINGE.3 - 0.4).abs() * 2.0;
            put(&mut rgba, at, mix(BODY, BODY_DARK, across), cover(d));
            // The screen's frame and the screen.
            put(&mut rgba, at, BEZEL, cover(rounded(p, INNER, 12.0)));
            put(&mut rgba, at, LCD_OFF, cover(rounded(p, SP_SCREEN, 2.0)));
            // The D-pad, a cross with a dimple.
            let (dx, dy) = (p.0 - DPAD.0, p.1 - DPAD.1);
            let arm = rounded((dx, dy), (-35.0, -11.5, 70.0, 23.0), 4.0).min(rounded(
                (dx, dy),
                (-11.5, -35.0, 23.0, 70.0),
                4.0,
            ));
            put(&mut rgba, at, BUTTON, cover(arm));
            put(&mut rgba, at, BUTTON_LIGHT, cover(disc(p, DPAD, 6.0)) * 0.5);
            // A and B.
            for b in [A_BUTTON, B_BUTTON] {
                put(&mut rgba, at, BUTTON, cover(disc(p, b, 17.0)));
                let gleam = disc(p, (b.0 - 5.0, b.1 - 5.0), 6.0);
                put(&mut rgba, at, BUTTON_LIGHT, cover(gleam) * 0.5);
            }
            // Start and Select.
            for sx in [126.0, 174.0] {
                let pill = rounded(p, (sx - 15.0, 398.0, 30.0, 10.0), 5.0);
                put(&mut rgba, at, BUTTON, cover(pill));
            }
            // The cart slot along the bottom edge.
            let slot = rounded(p, (SP_SLOT.0 - 66.0, 440.0, 132.0, 8.0), 2.0);
            put(&mut rgba, at, BODY_DARK, cover(slot) * 0.8);
            // The power light, dark until the device is on.
            put(&mut rgba, at, [40, 50, 44], cover(disc(p, SP_LED, 4.0)));
        }
    }
    UndoFace { rgba, w, h }
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
    /// How visible the cart is.
    pub cart_alpha: f32,
    pub led: bool,
    /// The screen's light: colour and how much it covers the dark LCD.
    pub lcd: [f32; 4],
}

/// The device's size and top-left on the output at rest.
pub fn sp_rest() -> (f32, f32, f32) {
    let scale = (OUT_H as f32 - 20.0) / SP_H as f32 * 0.86;
    let (w, h) = (SP_W as f32 * scale, SP_H as f32 * scale);
    let x = (OUT_W as f32 - w) / 2.0;
    let y = (OUT_H as f32 - h) / 2.0 - 18.0;
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
    // The screen's centre travels with the zoom, so it lands centred.
    let k = (scale - rest) / (full - rest);
    let centre = (from.0 + (to.0 - from.0) * k, from.1 + (to.1 - from.1) * k);
    (scale, centre.0 - sx * scale, centre.1 - sy * scale)
}

/// The device's screen on the output for a zoom.
pub fn sp_screen_rect(zoom: f32) -> (f32, f32, f32, f32) {
    let (s, x, y) = sp_placed(zoom);
    (
        x + SP_SCREEN.0 * s,
        y + SP_SCREEN.1 * s,
        SP_SCREEN.2 * s,
        SP_SCREEN.3 * s,
    )
}

/// Draws the scene, for a black stage: the cart (behind the device, so the
/// body hides what has gone in), the device, its lit screen and light.
pub fn draw_sp(
    device: Option<(TexId, u32, u32)>,
    cart: Option<(TexId, (f32, f32))>,
    pose: SpPose,
    out: &mut Vec<Draw>,
) {
    let (s, x, y) = sp_placed(pose.zoom);
    if let Some((tex, (cw, ch))) = cart.filter(|_| pose.cart_alpha > 0.0) {
        let k = SP_CART_W / cw;
        let (w, h) = (cw * k * s, ch * k * s);
        // Out: its top a little under the device; in: all but a lip hidden.
        let out_top = SP_SLOT.1 + 26.0;
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
    if let Some((tex, w, h)) = device {
        out.push(Draw::Tex {
            x,
            y,
            w: w as f32 * s,
            h: h as f32 * s,
            tex,
            alpha: pose.alpha,
        });
    }
    if pose.lcd[3] > 0.0 {
        let (lx, ly, lw, lh) = sp_screen_rect(pose.zoom);
        out.push(Draw::Rect {
            x: lx,
            y: ly,
            w: lw,
            h: lh,
            colour: pose.lcd,
        });
    }
    if pose.led && pose.alpha > 0.0 {
        let r = 3.5 * s;
        out.push(Draw::Rect {
            x: x + SP_LED.0 * s - r,
            y: y + SP_LED.1 * s - r,
            w: 2.0 * r,
            h: 2.0 * r,
            colour: [0.35, 1.0, 0.45, pose.alpha],
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
/// zoom. See [`SP_INSERT_S`].
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
    // The screen comes on pale, then dims to black as it fills the view,
    // for the game to fade in from.
    let glow = 0.85 * lit * (1.0 - zoom);
    SpPose {
        zoom,
        alpha,
        cart_in: span(t, SP_RISE_S, SP_SEATED_S),
        cart_alpha: alpha,
        led: t >= SP_SEATED_S,
        lcd: [
            0.82 * glow,
            0.86 * glow,
            0.80 * glow,
            if zoom > 0.0 { lit.max(zoom) } else { lit * 0.9 },
        ],
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
        lcd: [0.0, 0.0, 0.0, if t < 0.5 { 1.0 } else { 0.0 }],
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
    fn a_full_zoom_puts_the_screen_over_the_whole_output() {
        let (x, y, w, h) = sp_screen_rect(1.0);
        assert!(x.abs() < 0.5 && y.abs() < 0.5, "{x} {y}");
        assert!((w - OUT_W as f32).abs() < 0.5 && (h - OUT_H as f32).abs() < 0.5);
    }

    #[test]
    fn at_rest_the_device_fits_the_output() {
        let (s, x, y) = sp_placed(0.0);
        assert!(x > 0.0 && y > 0.0);
        assert!(x + SP_W as f32 * s < OUT_W as f32);
        assert!(y + SP_H as f32 * s < OUT_H as f32);
    }

    #[test]
    fn the_insert_runs_in_order_and_ends_zoomed_in_and_dark() {
        let start = sp_insert_pose(0.0);
        assert_eq!((start.alpha, start.zoom, start.cart_in), (0.0, 0.0, 0.0));
        let seated = sp_insert_pose(SP_SEATED_S);
        assert!(seated.cart_in > 0.99 && seated.zoom == 0.0 && seated.led);
        let end = sp_insert_pose(SP_INSERT_S);
        assert!(end.zoom > 0.99);
        assert!(end.lcd[..3].iter().all(|c| *c < 0.01), "{:?}", end.lcd);
        assert!(end.lcd[3] > 0.99, "the screen does not cover the device");
    }

    #[test]
    fn the_eject_runs_the_insert_backwards() {
        let start = sp_eject_pose(0.0);
        assert!(start.zoom > 0.99 && start.cart_in > 0.99);
        let out = sp_eject_pose(0.95);
        assert!(out.zoom == 0.0 && out.cart_in < 0.01 && !out.led);
        assert_eq!(sp_eject_pose(SP_EJECT_S).alpha, 0.0);
    }

    #[test]
    fn the_device_has_its_body_screen_and_controls() {
        let f = sp_face();
        let at = |x: f32, y: f32| {
            let i = ((y as u32 * SP_W + x as u32) * 4) as usize;
            [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2], f.rgba[i + 3]]
        };
        assert_eq!(at(1.0, 1.0)[3], 0, "the corner is not clear");
        let screen = at(SP_SCREEN.0 + 50.0, SP_SCREEN.1 + 50.0);
        assert_eq!(&screen[..3], &LCD_OFF);
        let body = at(150.0, 260.0);
        assert!(body[2] > body[0], "the body is not blue: {body:?}");
        assert_eq!(&at(DPAD.0 + 25.0, DPAD.1)[..3], &BUTTON);
    }
}
