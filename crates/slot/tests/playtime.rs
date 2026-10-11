mod common;

use std::time::{Duration, Instant};

use common::{app_playing_in, tmp_root_with_carts};
use slot::playtime;
use slot_store::Platform;

fn total(root: &std::path::Path, want: u64) -> u64 {
    let key = playtime::key(Platform::Gba, "Emerald");
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let secs = playtime::read(root, &key);
        if secs >= want || Instant::now() > until {
            return secs;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn time_spent_playing_is_added_up_on_the_card() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for ms in (0..=90_000).step_by(500) {
        a.tick_ms(ms);
    }
    a.settle_saves();
    let secs = total(d.path(), 89);
    assert!(
        (89..=91).contains(&secs),
        "counted {secs}s of a 90s session"
    );
}

#[test]
fn a_gap_in_ticks_such_as_a_sleep_is_not_counted_as_play() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.tick_ms(1_000);
    a.tick_ms(3_600_000);
    a.settle_saves();
    assert!(total(d.path(), 0) <= 2);
}

#[test]
fn a_game_played_before_says_how_long_when_it_starts() {
    let d = tmp_root_with_carts(&["Emerald"]);
    playtime::add(
        d.path(),
        &playtime::key(Platform::Gba, "Emerald"),
        3 * 3600 + 5 * 60,
    )
    .unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    a.tick_ms(10);
    assert_eq!(
        a.take_message(),
        Some(("Played 3h 05m".to_string(), "Emerald".to_string()))
    );
}

#[test]
fn play_time_reads_in_hours_and_minutes() {
    assert_eq!(playtime::format(30), "under a minute");
    assert_eq!(playtime::format(125), "2m");
    assert_eq!(playtime::format(3600 * 12 + 60 * 7), "12h 07m");
}
