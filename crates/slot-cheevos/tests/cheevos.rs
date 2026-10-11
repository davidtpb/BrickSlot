use std::time::{Duration, Instant};

use slot_cheevos::{Console, Event};

#[test]
fn achievement_addresses_land_on_the_consoles_own_ram() {
    slot_cheevos::start("brickslot-test/0");
    assert_eq!(
        slot_cheevos::real_address(Console::Gba, 0x0010),
        Some((0x0300_0010, 0x7FF0))
    );
    assert_eq!(
        slot_cheevos::real_address(Console::Gba, 0x8000),
        Some((0x0200_0000, 0x40000))
    );
    assert_eq!(
        slot_cheevos::real_address(Console::Gb, 0xC123),
        Some((0xC123, 0x1000 - 0x123))
    );
}

#[test]
fn a_read_spanning_two_blocks_is_stitched_from_both() {
    slot_cheevos::start("brickslot-test/0");
    slot_cheevos::set_memory(Console::Gba, |real, buf| {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((real as usize + i) >> 24) as u8;
        }
        buf.len()
    });
    let mut buf = [0u8; 4];
    assert_eq!(slot_cheevos::read(0x7FFE, &mut buf), 4);
    assert_eq!(buf, [3, 3, 2, 2], "IWRAM then EWRAM");
    slot_cheevos::clear_memory();
    assert_eq!(slot_cheevos::read(0x7FFE, &mut buf), 0);
}

#[test]
#[ignore = "talks to retroachievements.org"]
fn a_wrong_password_comes_back_from_the_server_as_a_failed_login() {
    assert!(slot_cheevos::start("brickslot-test/0"));
    slot_cheevos::login_password("brickslot-no-such-user", "not-a-password");
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        let events = slot_cheevos::take_events();
        if let Some(e) = events.iter().find(|e| matches!(e, Event::LoginFailed(_))) {
            eprintln!("{e:?}");
            return;
        }
        assert!(
            !events.iter().any(|e| matches!(e, Event::LoggedIn { .. })),
            "logged in with a made-up password"
        );
        assert!(Instant::now() < until, "the server never answered");
        std::thread::sleep(Duration::from_millis(100));
    }
}
