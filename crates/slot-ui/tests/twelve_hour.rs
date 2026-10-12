use slot_ui::{clock_text, hhmm, set_twelve_hour};

#[test]
fn twelve_hour_time_reads_with_am_and_pm() {
    let at = |h: i64, m: i64| h * 3600 + m * 60;
    assert_eq!(clock_text(at(21, 7), false), "21:07");
    assert_eq!(clock_text(at(21, 7), true), "9:07 PM");
    assert_eq!(clock_text(at(0, 5), true), "12:05 AM");
    assert_eq!(clock_text(at(12, 0), true), "12:00 PM");
    assert_eq!(clock_text(at(11, 59), true), "11:59 AM");
    set_twelve_hour(true);
    assert_eq!(hhmm(at(13, 30)), "1:30 PM");
    set_twelve_hour(false);
    assert_eq!(hhmm(at(13, 30)), "13:30");
}
