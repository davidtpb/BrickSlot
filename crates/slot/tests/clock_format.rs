mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_ui::QuickRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

#[test]
fn the_clock_setting_switches_to_twelve_hour_time_and_keeps_it() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(QuickRow::Screen) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    while a.quick_menu() != Some(QuickRow::ClockFormat) {
        press(&mut a, Btn::Down);
    }
    assert_eq!(
        a.quick_value(QuickRow::ClockFormat),
        Some(slot_ui::QuickValue::Hour24)
    );
    press(&mut a, Btn::Right);
    assert_eq!(
        a.quick_value(QuickRow::ClockFormat),
        Some(slot_ui::QuickValue::Hour12)
    );
    assert!(slot_store::read_slot_state(d.path()).clock_12h);
    assert!(slot_ui::hhmm(21 * 3600).ends_with("PM"));
}
