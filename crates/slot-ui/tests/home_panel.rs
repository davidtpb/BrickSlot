use slot_gfx::{Draw, OUT_H, OUT_W};
use slot_ui::{
    stat_rect, tile_rect, track_rect, HomeFocus, HomePanel, HomeScreen, HomeTile, CARD_H, CARD_W,
    CARD_X, CARD_Y, HOME_ACCENT, HOME_STATS,
};

fn inside((x, y, w, h): (f32, f32, f32, f32)) -> bool {
    x >= CARD_X && y >= CARD_Y && x + w <= CARD_X + CARD_W && y + h <= CARD_Y + CARD_H
}

fn rects(out: &[Draw]) -> Vec<(f32, f32, f32, f32, [f32; 4])> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, h, colour } => Some((x, y, w, h, colour)),
            _ => None,
        })
        .collect()
}

fn draw(panel: HomePanel, on: [bool; HomeTile::ALL.len()], levels: [f32; 2]) -> Vec<Draw> {
    let mut out = Vec::new();
    HomeScreen {
        faces: &[],
        panel,
        on,
        levels,
        light: false,
        icons: &[],
    }
    .draw(&mut out);
    out
}

#[test]
fn the_card_sits_on_the_screen() {
    const {
        assert!(CARD_X >= 0.0 && CARD_Y >= 0.0);
        assert!(CARD_X + CARD_W <= OUT_W as f32);
        assert!(CARD_Y + CARD_H <= OUT_H as f32);
    }
}

#[test]
fn every_tile_slider_and_stat_fits_inside_the_card_without_overlap() {
    let tiles: Vec<_> = (0..HomeTile::ALL.len()).map(tile_rect).collect();
    for t in &tiles {
        assert!(inside(*t), "a tile at {t:?} leaves the card");
    }
    for pair in tiles.windows(2) {
        assert!(pair[0].0 + pair[0].2 < pair[1].0, "tiles touch");
    }
    for i in 0..2 {
        assert!(inside(track_rect(i)));
    }
    for i in 0..HOME_STATS {
        let s = stat_rect(i);
        assert!(inside(s), "a stat at {s:?} leaves the card");
        assert!(
            s.1 + s.3 < track_rect(0).1,
            "the stats run into the sliders"
        );
    }
    assert!(
        tiles[0].1 > track_rect(1).1 + track_rect(1).3,
        "the buttons run into the sliders"
    );
    assert!(
        tiles[0].1 + tiles[0].3 + 44.0 < slot_ui::HOME_SPLIT,
        "the buttons' names reach the split"
    );
}

#[test]
fn the_buttons_are_round_textures_with_a_ring_on_the_focused_one() {
    let mut p = HomePanel::new();
    p.step(true);
    let icons: Vec<_> = (0..HomeTile::ALL.len() * 2 + 1)
        .map(|i| {
            let d = if i == HomeTile::ALL.len() * 2 {
                slot_ui::RING_D
            } else {
                slot_ui::ICON_D
            };
            (slot_gfx::TexId::from_raw(500 + i), d, d)
        })
        .collect();
    let mut on = [false; HomeTile::ALL.len()];
    on[2] = true;
    let mut out = Vec::new();
    HomeScreen {
        faces: &[],
        panel: p,
        on,
        levels: [0.0, 0.0],
        light: true,
        icons: &icons,
    }
    .draw(&mut out);
    let texes: Vec<usize> = out
        .iter()
        .filter_map(|d| match d {
            Draw::Tex { tex, .. } => icons.iter().position(|i| i.0 == *tex),
            _ => None,
        })
        .collect();
    assert!(texes.contains(&0), "the screenshot button is not drawn off");
    assert!(texes.contains(&5), "the rumble button is not drawn on");
    assert!(
        !texes.contains(&4),
        "the rumble button is drawn off as well"
    );
    assert!(texes.contains(&(HomeTile::ALL.len() * 2)), "no focus ring");
}

#[test]
fn the_cursor_wraps_along_the_tiles_and_through_the_rows() {
    let mut p = HomePanel::new();
    assert_eq!(p.focus(), HomeFocus::Tile(HomeTile::Screenshot));
    assert!(p.step(false));
    assert_eq!(p.focus(), HomeFocus::Tile(HomeTile::Power));
    assert!(p.step(true));
    assert!(p.step(true));
    assert_eq!(p.focus(), HomeFocus::Tile(HomeTile::Sound));
    p.down();
    assert_eq!(p.focus(), HomeFocus::Brightness);
    assert!(!p.step(true), "left and right belong to the slider here");
    p.down();
    assert_eq!(p.focus(), HomeFocus::Volume);
    p.down();
    assert_eq!(
        p.focus(),
        HomeFocus::Tile(HomeTile::Sound),
        "the tile last stood on was forgotten"
    );
    p.up();
    assert_eq!(p.focus(), HomeFocus::Volume);
}

#[test]
fn a_switched_on_tile_is_lit() {
    let mut on = [false; HomeTile::ALL.len()];
    on[3] = true;
    let out = draw(HomePanel::new(), on, [0.5, 0.5]);
    let (x, y, w, h) = tile_rect(3);
    assert!(rects(&out)
        .iter()
        .any(|r| (r.0, r.1, r.2, r.3) == (x, y, w, h) && r.4 == HOME_ACCENT));
    let (x, y, w, h) = tile_rect(2);
    assert!(rects(&out)
        .iter()
        .any(|r| (r.0, r.1, r.2, r.3) == (x, y, w, h) && r.4 != HOME_ACCENT));
}

#[test]
fn a_slider_fills_to_its_level() {
    let out = draw(HomePanel::new(), [false; HomeTile::ALL.len()], [0.5, 0.0]);
    let (x, y, w, h) = track_rect(0);
    assert!(rects(&out)
        .iter()
        .any(|r| (r.0, r.1, r.3) == (x, y, h) && r.2 == (w * 0.5).round() && r.4 == HOME_ACCENT));
    let (x, y, _, _) = track_rect(1);
    assert!(
        !rects(&out)
            .iter()
            .any(|r| (r.0, r.1) == (x, y) && r.4 == HOME_ACCENT),
        "an empty slider shows a fill"
    );
}

#[test]
fn the_focused_tile_is_framed() {
    let mut p = HomePanel::new();
    p.step(true);
    let out = draw(p, [false; HomeTile::ALL.len()], [0.0, 0.0]);
    let (x, y, _, _) = tile_rect(1);
    assert!(
        rects(&out)
            .iter()
            .any(|r| r.0 < x && r.1 < y && r.0 > x - 10.0 && r.1 > y - 10.0),
        "no frame around the focused tile"
    );
}
