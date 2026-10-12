mod common;

use common::{app_booting_at, app_playing_with, tmp_root_with_carts, StubSnapshot, CLOCK_IS_SET};
use slot_input::{Action, Btn};

fn wait(a: &mut slot::app::App, secs: u32) {
    for _ in 0..secs * 4 {
        a.update(0.25);
    }
}

#[test]
fn the_shelf_dims_after_half_a_minute_and_a_press_brings_it_back() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    wait(&mut a, 20);
    assert!(!a.idle_dimmed(), "dimmed too soon");
    wait(&mut a, 15);
    assert!(a.idle_dimmed(), "the shelf never dimmed");
    a.apply(Action::GbaDown(Btn::Right));
    assert!(!a.idle_dimmed(), "a press did not bring it back");
}

#[test]
fn a_game_is_never_dimmed_however_long_between_presses() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut a = app_playing_with(d.path(), "Fusion", StubSnapshot::boxed());
    wait(&mut a, 90);
    assert!(!a.idle_dimmed(), "the game was dimmed mid-play");
}
