mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform, StateRing};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn shelf(ask: bool, resume: bool) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    s.ask_resume = ask;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    if resume {
        let stem = a.selected_stem().unwrap().to_string();
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, &stem)
            .write_resume(b"state")
            .unwrap();
    }
    (d, a)
}

#[test]
fn a_game_left_mid_play_asks_whether_to_continue() {
    let (_d, mut a) = shelf(true, true);
    press(&mut a, Btn::A);
    assert!(a.resume_popup_open(), "no question asked");
    assert!(matches!(a.phase(), Phase::Shelf));
    let lines = a.settings_lines().expect("popup text");
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("Continue"));
    assert!(lines[slot_ui::option_slot(1)].0.starts_with("Start fresh"));
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    assert!(!a.starting_clean(), "continue started fresh");
}

#[test]
fn start_fresh_skips_where_it_was_left() {
    let (_d, mut a) = shelf(true, true);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(a.starting_clean());
}

#[test]
fn b_puts_the_question_away_and_leaves_the_game_out() {
    let (_d, mut a) = shelf(true, true);
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(!a.resume_popup_open());
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn a_game_with_nothing_to_continue_just_goes_in() {
    let (_d, mut a) = shelf(true, false);
    press(&mut a, Btn::A);
    assert!(!a.resume_popup_open());
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
}

#[test]
fn turning_the_question_off_continues_without_asking() {
    let (_d, mut a) = shelf(false, true);
    press(&mut a, Btn::A);
    assert!(!a.resume_popup_open());
    assert!(!a.starting_clean());
}
