use std::path::Path;

use slot::cheevos::{console_of, remember, words, LOGIN_FILE};
use slot_cheevos::{Console, Event};
use slot_retro::MemoryRegion;

#[test]
fn the_console_comes_from_the_rom_extension() {
    assert_eq!(console_of(Path::new("a/Emerald.GBA")), Some(Console::Gba));
    assert_eq!(console_of(Path::new("Link.gbc")), Some(Console::Gbc));
    assert_eq!(console_of(Path::new("Tetris.gb")), Some(Console::Gb));
    assert_eq!(console_of(Path::new("notes.txt")), None);
}

#[test]
fn an_unlock_says_its_name_points_and_what_it_was_for() {
    let said = words(&Event::Unlocked {
        title: "Champion".into(),
        description: "Beat the Elite Four".into(),
        points: 25,
    });
    assert_eq!(
        said,
        Some((
            "Unlocked: Champion (25)".into(),
            "Beat the Elite Four".into()
        ))
    );
    assert_eq!(
        words(&Event::GameLoaded {
            title: "Pokemon Emerald".into(),
            unlocked: 3,
            total: 80
        }),
        Some(("Pokemon Emerald".into(), "3 of 80 achievements".into()))
    );
    assert_eq!(words(&Event::Reconnected), None);
}

#[test]
fn logging_in_swaps_the_password_on_the_card_for_the_token() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(
        d.path().join(LOGIN_FILE),
        "username = player\npassword = hunter22\n",
    )
    .unwrap();
    remember(d.path(), "player", "abc123").unwrap();
    let text = std::fs::read_to_string(d.path().join(LOGIN_FILE)).unwrap();
    assert!(text.contains("token = abc123"));
    assert!(text.contains("username = player"));
    assert!(
        !text.contains("hunter22"),
        "the password stayed on the card"
    );
}

#[test]
fn a_memory_region_reads_only_what_it_holds() {
    let ram: Vec<u8> = (0..16).collect();
    let r = MemoryRegion {
        start: 0x0300_0000,
        len: ram.len(),
        ptr: ram.as_ptr() as usize,
    };
    let mut buf = [0u8; 4];
    assert_eq!(unsafe { r.read(0x0300_000E, &mut buf) }, 2);
    assert_eq!(&buf[..2], &[14, 15]);
    assert_eq!(unsafe { r.read(0x0200_0000, &mut buf) }, 0);
    assert_eq!(unsafe { r.read(0x0300_0010, &mut buf) }, 0);
}
