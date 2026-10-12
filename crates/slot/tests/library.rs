mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::library;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn boot(d: &tempfile::TempDir) -> App {
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    a
}

fn carts() -> tempfile::TempDir {
    tmp_root_with_carts(&["Fusion", "Pokémon Emerald", "Zero Mission"])
}

#[test]
fn x_opens_inspect_with_the_games_details_and_b_closes_it() {
    let d = carts();
    let mut a = boot(&d);
    press(&mut a, Btn::X);
    assert!(a.inspect_open());
    assert!(!a.customize_open());
    let lines = a.settings_lines().expect("inspect text");
    assert_eq!(lines[slot_ui::inspect_caption_slot(0)].0, "System");
    assert_eq!(lines[slot_ui::inspect_value_slot(0)].0, "Game Boy Advance");
    assert_eq!(lines[slot_ui::inspect_value_slot(2)].0, "Not played yet");
    press(&mut a, Btn::B);
    assert!(!a.inspect_open());
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn y_in_inspect_stars_the_game_and_a_fav_shelf_appears() {
    let d = carts();
    let mut a = boot(&d);
    let stem = a.selected_stem().unwrap().to_string();
    press(&mut a, Btn::X);
    press(&mut a, Btn::Y);
    let saved = library::read_favorites(d.path());
    assert!(saved.contains(&library::game_key(slot_store::Platform::Gba, &stem)));
    press(&mut a, Btn::B);

    let a = boot(&d);
    assert!(a.shelf_platform_name().is_some(), "no second shelf");
    let mut a = a;
    a.apply(Action::GbaDown(Btn::R1));
    a.apply(Action::GbaUp(Btn::R1));
    assert!(a.on_favorites());
    assert_eq!(a.shelf_platform_name(), Some("FAVORITES"));
    assert_eq!(a.selected_stem(), Some(stem.as_str()));
}

#[test]
fn unstarring_the_last_favourite_leaves_the_fav_shelf() {
    let d = carts();
    let mut a = boot(&d);
    press(&mut a, Btn::X);
    press(&mut a, Btn::Y);
    press(&mut a, Btn::B);
    press(&mut a, Btn::R1);
    assert!(a.on_favorites());
    press(&mut a, Btn::X);
    press(&mut a, Btn::Y);
    assert!(!a.on_favorites());
    assert!(library::read_favorites(d.path()).is_empty());
}

#[test]
fn a_in_inspect_inserts_the_game() {
    let d = carts();
    let mut a = boot(&d);
    press(&mut a, Btn::X);
    press(&mut a, Btn::A);
    assert!(!a.inspect_open());
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
}

#[test]
fn inserting_notes_the_game_as_recently_played() {
    let d = carts();
    let mut a = boot(&d);
    let stem = a.selected_stem().unwrap().to_string();
    press(&mut a, Btn::A);
    let recent = library::read_recent(d.path());
    assert_eq!(recent.first().map(|p| p.stem.as_str()), Some(stem.as_str()));
}

#[test]
fn select_lists_recent_games_and_a_starts_the_picked_one() {
    let d = carts();
    library::note_played(d.path(), slot_store::Platform::Gba, "Zero Mission", 10).unwrap();
    library::note_played(d.path(), slot_store::Platform::Gba, "Fusion", 20).unwrap();
    let mut a = boot(&d);
    press(&mut a, Btn::Select);
    assert!(a.recent_open());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(!a.recent_open());
    assert_eq!(a.selected_stem(), Some("Zero Mission"));
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
}
