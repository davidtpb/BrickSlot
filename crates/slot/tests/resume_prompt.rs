mod common;

use common::{app_booting_at, app_playing_with, tmp_root_with_carts, StubSnapshot, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform, StateRing};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn left_at(d: &tempfile::TempDir, stem: &str, state: &[u8]) {
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, stem)
        .write_resume(state)
        .unwrap();
}

fn shelf(fresh: bool, resume: bool) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    s.start_fresh = fresh;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    if resume {
        left_at(&d, a.selected_stem().unwrap(), b"left here");
    }
    (d, a)
}

#[test]
fn a_game_goes_in_fresh_and_keeps_where_it_was_left_to_load() {
    let (_d, mut a) = shelf(true, true);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    assert!(a.starting_clean(), "the game picked up where it was left");
    assert!(a.can_load_left_off());
}

#[test]
fn a_game_never_left_mid_play_has_nothing_to_load() {
    let (_d, mut a) = shelf(true, false);
    press(&mut a, Btn::A);
    assert!(a.starting_clean());
    assert!(!a.can_load_left_off());
}

#[test]
fn with_start_fresh_off_a_tap_picks_up_where_it_was_left() {
    let (_d, mut a) = shelf(false, true);
    press(&mut a, Btn::A);
    assert!(!a.starting_clean());
    assert!(!a.can_load_left_off());
}

#[test]
fn menu_in_the_game_offers_where_it_was_left_and_loads_it() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    left_at(&d, "Fusion", b"left here");
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Fusion", snapshot);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert!(
        a.can_load_left_off(),
        "powering on lost where the game was left"
    );
    a.apply(Action::QuickMenu);
    assert!(a.resume_popup_open());
    let lines = a.settings_lines().expect("menu text");
    assert_eq!(lines[slot_ui::option_slot(0)].0, "Resume");
    assert!(lines[slot_ui::option_slot(1)].0.starts_with("Load where"));
    press(&mut a, Btn::A);
    assert!(!a.resume_popup_open());
    assert_eq!(
        loaded.lock().unwrap().as_deref(),
        Some(&b"left here"[..]),
        "where the game was left was not loaded"
    );
}

#[test]
fn resume_loads_nothing() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    left_at(&d, "Fusion", b"left here");
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Fusion", snapshot);
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Up);
    press(&mut a, Btn::A);
    assert!(!a.resume_popup_open());
    assert!(loaded.lock().unwrap().is_none());
}

#[test]
fn menu_in_a_game_with_nothing_to_load_starts_on_resume() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut a = app_playing_with(d.path(), "Fusion", StubSnapshot::boxed());
    a.apply(Action::QuickMenu);
    assert!(a.resume_popup_open());
    let lines = a.settings_lines().expect("menu text");
    assert_eq!(lines[slot_ui::option_slot(0)].0, "Resume");
    assert_eq!(lines[slot_ui::option_slot(1)].0, "Save State");
    press(&mut a, Btn::A);
    assert!(!a.resume_popup_open());
}
