mod common;

use common::{app_booting_at, tmp_root_with_carts};
use slot::app::Phase;
use slot::net::sntp_reply;
use slot_store::read_slot_state;

fn reply(first: u8, stratum: u8, ntp_secs: u32, fraction: u32) -> [u8; 48] {
    let mut p = [0u8; 48];
    p[0] = first;
    p[1] = stratum;
    p[40..44].copy_from_slice(&ntp_secs.to_be_bytes());
    p[44..48].copy_from_slice(&fraction.to_be_bytes());
    p
}

#[test]
fn a_server_reply_gives_unix_seconds() {
    // 2026-10-11 12:00:00 UTC is 1791720000 Unix, 4000708800 NTP.
    let r = reply(0x24, 2, 4_000_708_800, 0);
    assert_eq!(sntp_reply(&r), Some(1_791_720_000));
    let r = reply(0x24, 2, 4_000_708_800, 0x9000_0000);
    assert_eq!(
        sntp_reply(&r),
        Some(1_791_720_001),
        "half a second rounds up"
    );
}

#[test]
fn a_reply_past_the_2036_wrap_still_lands_in_the_right_era() {
    // NTP seconds 1000 in era 1 is 2036-02-07 06:44:56 UTC.
    assert_eq!(sntp_reply(&reply(0x24, 2, 1000, 0)), Some(2_085_979_496));
}

#[test]
fn unusable_replies_are_refused() {
    assert_eq!(
        sntp_reply(&reply(0x24, 0, 4_000_708_800, 0)),
        None,
        "kiss-o'-death"
    );
    assert_eq!(
        sntp_reply(&reply(0xE4, 2, 4_000_708_800, 0)),
        None,
        "unsynchronised"
    );
    assert_eq!(
        sntp_reply(&reply(0x23, 2, 4_000_708_800, 0)),
        None,
        "a client packet"
    );
    assert_eq!(sntp_reply(&[0u8; 20]), None, "too short");
}

#[test]
fn network_time_sets_the_clock_and_skips_the_clock_screen() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, clock) = app_booting_at(d.path(), 0);
    assert!(matches!(a.phase(), Phase::SetClock { .. }));
    a.set_network_time(1_791_720_000);
    assert_eq!(clock.get(), 1_791_720_000);
    assert!(read_slot_state(d.path()).clock_set);
    assert!(
        !matches!(a.phase(), Phase::SetClock { .. }),
        "{:?}",
        a.phase()
    );
    let (title, _) = a.take_message().expect("no banner");
    assert_eq!(title, "Clock set from the internet");
}
