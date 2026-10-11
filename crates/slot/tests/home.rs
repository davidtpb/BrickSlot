mod common;

use common::{app_booting_at, app_playing_in, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_ui::{HomeFocus, HomeTile};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn shelf_of(stems: &[&str]) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(stems);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    (d, a)
}

fn walk_to(a: &mut App, tile: HomeTile) {
    for _ in 0..HomeTile::ALL.len() {
        if a.home_lines().is_some() && focus_of(a) == HomeFocus::Tile(tile) {
            return;
        }
        press(a, Btn::Right);
    }
    panic!("never reached {tile:?}");
}

fn focus_of(a: &App) -> HomeFocus {
    a.home_focus().expect("the panel is open")
}

#[test]
fn home_on_the_shelf_opens_the_panel_and_b_closes_it() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    assert!(a.home_open());
    assert!(matches!(a.phase(), Phase::Shelf));
    let lines = a.home_lines().unwrap();
    assert_eq!(lines[slot_ui::HOME_TITLE].0, "brickslot");
    assert_eq!(lines[slot_ui::home_tile_name_slot(3)].0, "Wi-Fi");
    assert_eq!(lines[slot_ui::home_stat_name_slot(0)].0, "Temp");
    press(&mut a, Btn::B);
    assert!(!a.home_open());
    assert!(a.home_lines().is_none());
}

#[test]
fn home_again_closes_the_panel() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    press(&mut a, Btn::Home);
    assert!(!a.home_open());
}

#[test]
fn the_panel_keeps_the_shelf_from_moving() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3", "Zero Mission"]);
    let before = a.selected_stem().map(str::to_owned);
    press(&mut a, Btn::Home);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert_eq!(
        a.selected_stem().map(str::to_owned),
        before,
        "the shelf moved under the panel"
    );
}

#[test]
fn up_and_down_walk_the_tiles_then_the_sliders() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    assert_eq!(a.home_focus(), Some(HomeFocus::Tile(HomeTile::Screenshot)));
    press(&mut a, Btn::Down);
    assert_eq!(a.home_focus(), Some(HomeFocus::Brightness));
    press(&mut a, Btn::Down);
    assert_eq!(a.home_focus(), Some(HomeFocus::Volume));
    press(&mut a, Btn::Down);
    assert_eq!(a.home_focus(), Some(HomeFocus::Tile(HomeTile::Screenshot)));
    press(&mut a, Btn::Left);
    assert_eq!(a.home_focus(), Some(HomeFocus::Tile(HomeTile::Power)));
}

#[test]
fn left_and_right_on_the_brightness_row_move_the_backlight() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    press(&mut a, Btn::Down);
    let before = a.brightness();
    press(&mut a, Btn::Left);
    assert_eq!(a.brightness(), before.saturating_sub(1));
    press(&mut a, Btn::Right);
    assert_eq!(a.brightness(), before);
    assert!(a.home_open());
}

#[test]
fn the_rumble_tile_flips_rumble() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    let before = a.rumble_enabled();
    press(&mut a, Btn::Home);
    walk_to(&mut a, HomeTile::Rumble);
    press(&mut a, Btn::A);
    assert_eq!(a.rumble_enabled(), !before);
    assert!(a.home_open(), "a toggle closed the panel");
}

#[test]
fn the_sound_tile_mutes() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    walk_to(&mut a, HomeTile::Sound);
    press(&mut a, Btn::A);
    assert!(a.muted());
    let lines = a.home_lines().unwrap();
    assert_eq!(lines[slot_ui::home_slider_value_slot(1)].0, "Muted");
}

#[test]
fn the_power_tile_hands_over_to_the_power_menu() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    walk_to(&mut a, HomeTile::Power);
    press(&mut a, Btn::A);
    assert!(!a.home_open());
    assert!(a.power_menu().is_some());
}

#[test]
fn menu_from_the_panel_on_the_shelf_opens_settings() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    a.apply(Action::QuickMenu);
    assert!(!a.home_open());
    assert!(matches!(a.phase(), Phase::QuickMenu { .. }));
}

#[test]
fn home_in_a_game_opens_the_panel_over_it() {
    let d = tmp_root_with_carts(&["Fusion"]);
    let mut a = app_playing_in(d.path(), "Fusion");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    press(&mut a, Btn::Home);
    assert!(a.home_open());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    let lines = a.home_lines().unwrap();
    assert_eq!(lines[slot_ui::HOME_TITLE].0, "Fusion");
    assert_eq!(lines[slot_ui::home_tile_state_slot(0)].0, "Take");
    press(&mut a, Btn::B);
    assert!(!a.home_open());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn menu_and_y_open_the_panel_where_there_is_no_home_button() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    a.apply(Action::HomePanel);
    assert!(a.home_open());
    assert!(matches!(a.phase(), Phase::Shelf), "Y's search opened too");
    a.apply(Action::HomePanel);
    assert!(!a.home_open());
}
