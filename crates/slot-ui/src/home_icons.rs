//! The Home panel's round buttons, drawn here: a disc with a line icon on
//! it, after NX Redux's quick menu.

use crate::home_panel::HomeTile;
use crate::plate::UndoFace;

/// A button's width and height.
pub const ICON_D: u32 = 64;
/// The focus ring's, around a button.
pub const RING_D: u32 = ICON_D + 16;

/// One stroke or fill of an icon, in a unit square over the button.
#[derive(Copy, Clone)]
enum Shape {
    /// A line with round ends, `w` wide.
    Seg(f32, f32, f32, f32, f32),
    /// An arc of a circle with round ends, `w` wide, between two angles in
    /// degrees measured anticlockwise from three o'clock.
    Arc(f32, f32, f32, f32, f32, f32),
    Disc(f32, f32, f32),
    /// A filled box with rounded corners.
    Box(f32, f32, f32, f32, f32),
    /// A filled triangle.
    Tri([(f32, f32); 3]),
}

impl Shape {
    /// The signed distance from `p` to the shape, in units of the square:
    /// below zero inside.
    fn distance(self, (px, py): (f32, f32)) -> f32 {
        match self {
            Shape::Seg(x0, y0, x1, y1, w) => segment((px, py), (x0, y0), (x1, y1)) - w / 2.0,
            Shape::Arc(cx, cy, r, w, a0, a1) => {
                let (dx, dy) = (px - cx, cy - py);
                let a = dy.atan2(dx).to_degrees().rem_euclid(360.0);
                let (a0, a1) = (a0.rem_euclid(360.0), a1.rem_euclid(360.0));
                let within = match a0 <= a1 {
                    true => a >= a0 && a <= a1,
                    false => a >= a0 || a <= a1,
                };
                let d = match within {
                    true => ((dx * dx + dy * dy).sqrt() - r).abs(),
                    false => {
                        let end = |deg: f32| {
                            let t = deg.to_radians();
                            let (ex, ey) = (cx + r * t.cos(), cy - r * t.sin());
                            ((px - ex).powi(2) + (py - ey).powi(2)).sqrt()
                        };
                        end(a0).min(end(a1))
                    }
                };
                d - w / 2.0
            }
            Shape::Disc(cx, cy, r) => ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - r,
            Shape::Box(x, y, w, h, r) => {
                let (cx, cy) = (x + w / 2.0, y + h / 2.0);
                let qx = (px - cx).abs() - (w / 2.0 - r);
                let qy = (py - cy).abs() - (h / 2.0 - r);
                (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r
            }
            Shape::Tri(v) => {
                let edge = (0..3)
                    .map(|i| segment((px, py), v[i], v[(i + 1) % 3]))
                    .fold(f32::MAX, f32::min);
                let side = |a: (f32, f32), b: (f32, f32)| {
                    (b.0 - a.0) * (py - a.1) - (b.1 - a.1) * (px - a.0)
                };
                let s = [side(v[0], v[1]), side(v[1], v[2]), side(v[2], v[0])];
                let inside = s.iter().all(|&x| x >= 0.0) || s.iter().all(|&x| x <= 0.0);
                if inside {
                    -edge
                } else {
                    edge
                }
            }
        }
    }
}

fn segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let len = abx * abx + aby * aby;
    let t = match len > 0.0 {
        true => (((p.0 - a.0) * abx + (p.1 - a.1) * aby) / len).clamp(0.0, 1.0),
        false => 0.0,
    };
    let (x, y) = (a.0 + abx * t, a.1 + aby * t);
    ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt()
}

const LINE: f32 = 0.045;

/// What a button's icon is made of: shapes drawn, then shapes cut out.
fn icon(tile: HomeTile) -> (Vec<Shape>, Vec<Shape>) {
    use Shape::*;
    match tile {
        HomeTile::Screenshot => (
            vec![
                Box(0.27, 0.37, 0.46, 0.31, 0.05),
                Box(0.40, 0.31, 0.16, 0.09, 0.02),
            ],
            vec![Disc(0.5, 0.525, 0.095)],
        ),
        HomeTile::Sound => (
            vec![
                Box(0.27, 0.43, 0.09, 0.14, 0.015),
                Tri([(0.34, 0.43), (0.47, 0.32), (0.47, 0.68)]),
                Tri([(0.34, 0.43), (0.47, 0.68), (0.34, 0.57)]),
                Arc(0.47, 0.5, 0.10, LINE, -50.0, 50.0),
                Arc(0.47, 0.5, 0.19, LINE, -50.0, 50.0),
            ],
            vec![],
        ),
        HomeTile::Rumble => (
            vec![Box(0.39, 0.30, 0.22, 0.40, 0.04)],
            vec![Box(0.425, 0.345, 0.15, 0.31, 0.015)],
        ),
        HomeTile::Wifi => (
            vec![
                Arc(0.5, 0.67, 0.12, LINE, 45.0, 135.0),
                Arc(0.5, 0.67, 0.22, LINE, 45.0, 135.0),
                Arc(0.5, 0.67, 0.32, LINE, 45.0, 135.0),
                Disc(0.5, 0.67, 0.04),
            ],
            vec![],
        ),
        HomeTile::Bluetooth => (
            vec![
                Seg(0.48, 0.27, 0.48, 0.73, LINE),
                Seg(0.48, 0.27, 0.61, 0.385, LINE),
                Seg(0.61, 0.385, 0.36, 0.615, LINE),
                Seg(0.48, 0.73, 0.61, 0.615, LINE),
                Seg(0.61, 0.615, 0.36, 0.385, LINE),
            ],
            vec![],
        ),
        HomeTile::Lights => (
            vec![Disc(0.5, 0.44, 0.15), Box(0.435, 0.53, 0.13, 0.15, 0.03)],
            vec![Seg(0.43, 0.615, 0.57, 0.615, 0.018)],
        ),
        HomeTile::Power => (
            vec![
                Arc(0.5, 0.53, 0.18, LINE + 0.005, 125.0, 55.0),
                Seg(0.5, 0.27, 0.5, 0.49, LINE + 0.005),
            ],
            vec![],
        ),
    }
}

/// The shakes beside the rumble icon's handset.
fn rumble_shakes() -> Vec<Shape> {
    let zig = |x: f32, dir: f32| {
        let pts = [0.38, 0.45, 0.52, 0.59];
        (0..3)
            .map(|i| {
                let (y0, y1) = (pts[i], pts[i + 1]);
                let (x0, x1) = match i % 2 {
                    0 => (x, x + 0.05 * dir),
                    _ => (x + 0.05 * dir, x),
                };
                Shape::Seg(x0, y0 + 0.03, x1, y1 + 0.03, 0.035)
            })
            .collect::<Vec<_>>()
    };
    let mut out = zig(0.31, -1.0);
    out.extend(zig(0.69, 1.0));
    out
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

/// The colours of a button: its disc and its icon.
pub fn icon_colours(on: bool, light: bool) -> ([u8; 3], [u8; 3]) {
    let tint = crate::socket::tinted;
    match (light, on) {
        (true, false) => (tint([246, 246, 246]), [40, 40, 40]),
        (true, true) => ([36, 163, 120], [255, 255, 255]),
        (false, false) => ([58, 58, 66], [226, 226, 232]),
        (false, true) => ([69, 133, 235], [255, 255, 255]),
    }
}

/// A round button with its icon, lit or not.
pub fn home_icon_face(tile: HomeTile, on: bool, light: bool) -> UndoFace {
    let d = ICON_D;
    let (disc, ink) = icon_colours(on, light);
    let (mut fills, holes) = icon(tile);
    if tile == HomeTile::Rumble {
        fills.extend(rumble_shakes());
    }
    let mut rgba = vec![0u8; (d * d * 4) as usize];
    let px = 1.0 / d as f32;
    for y in 0..d {
        for x in 0..d {
            let p = ((x as f32 + 0.5) * px, (y as f32 + 0.5) * px);
            let at = ((y * d + x) * 4) as usize;
            let back = Shape::Disc(0.5, 0.5, 0.5 - px).distance(p) / px;
            put(&mut rgba, at, disc, (0.5 - back).clamp(0.0, 1.0));
            let fill = fills
                .iter()
                .map(|s| s.distance(p) / px)
                .fold(f32::MAX, f32::min);
            let hole = holes
                .iter()
                .map(|s| s.distance(p) / px)
                .fold(f32::MAX, f32::min);
            let cover = (0.5 - fill).clamp(0.0, 1.0) * (1.0 - (0.5 - hole).clamp(0.0, 1.0));
            put(&mut rgba, at, ink, cover);
        }
    }
    UndoFace { rgba, w: d, h: d }
}

/// The ring drawn around the button under the cursor.
pub fn home_ring_face(light: bool) -> UndoFace {
    let d = RING_D;
    let colour = match light {
        true => [24, 24, 24],
        false => [72, 214, 230],
    };
    let mut rgba = vec![0u8; (d * d * 4) as usize];
    let c = d as f32 / 2.0;
    let (r, w) = (c - 3.5, 4.0);
    for y in 0..d {
        for x in 0..d {
            let dist = ((x as f32 + 0.5 - c).powi(2) + (y as f32 + 0.5 - c).powi(2)).sqrt();
            let edge = (dist - r).abs() - w / 2.0;
            put(
                &mut rgba,
                ((y * d + x) * 4) as usize,
                colour,
                (0.5 - edge).clamp(0.0, 1.0),
            );
        }
    }
    UndoFace { rgba, w: d, h: d }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_pixels(f: &UndoFace, ink: [u8; 3]) -> usize {
        f.rgba
            .chunks_exact(4)
            .filter(|p| p[3] == 255 && p[..3] == ink)
            .count()
    }

    #[test]
    fn every_icon_draws_something_inside_its_disc_and_differs() {
        let mut seen = Vec::new();
        for tile in HomeTile::ALL {
            for (on, light) in [(false, false), (true, false), (false, true), (true, true)] {
                let f = home_icon_face(tile, on, light);
                assert_eq!((f.w, f.h), (ICON_D, ICON_D));
                let (_, ink) = icon_colours(on, light);
                let n = ink_pixels(&f, ink);
                assert!(n > 60, "{tile:?} on={on} light={light}: {n} pixels");
                let corner = &f.rgba[..4];
                assert_eq!(corner[3], 0, "the corner is not clear");
            }
            let f = home_icon_face(tile, false, true);
            assert!(!seen.contains(&f.rgba), "{tile:?} looks like another");
            seen.push(f.rgba);
        }
    }

    #[test]
    fn the_ring_is_hollow() {
        let f = home_ring_face(true);
        let mid = ((RING_D / 2 * RING_D + RING_D / 2) * 4) as usize;
        assert_eq!(f.rgba[mid + 3], 0);
        let edge = ((RING_D / 2 * RING_D + 3) * 4) as usize;
        assert!(f.rgba[edge + 3] > 200);
    }
}
