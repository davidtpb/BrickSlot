use slot::labels::{candidates, fetch_all, url, Wanted};
use slot_store::Platform;

#[test]
fn names_are_tried_as_they_are_then_without_accents_then_without_a_revision() {
    assert_eq!(
        candidates("Pokémon - FireRed Version (USA, Europe) (Rev 1)"),
        [
            "Pokémon - FireRed Version (USA, Europe) (Rev 1)",
            "Pokemon - FireRed Version (USA, Europe) (Rev 1)",
            "Pokémon - FireRed Version (USA, Europe)",
            "Pokemon - FireRed Version (USA, Europe)",
        ]
    );
    assert_eq!(
        candidates("Legend of Zelda, The - A Link to the Past & Four Swords (USA)"),
        ["Legend of Zelda, The - A Link to the Past _ Four Swords (USA)"],
        "& is stored as _"
    );
}

#[test]
fn urls_point_at_the_systems_boxarts_with_everything_escaped() {
    assert_eq!(
        url(Platform::Gba, "Metroid Fusion (USA)"),
        "https://thumbnails.libretro.com/Nintendo%20-%20Game%20Boy%20Advance/Named_Boxarts/Metroid%20Fusion%20%28USA%29.png"
    );
}

#[test]
fn found_labels_are_saved_missing_ones_counted_and_existing_ones_left_alone() {
    let d = tempfile::tempdir().unwrap();
    let dest = |s: &str| d.path().join("Labels/GBA").join(format!("{s}.png"));
    std::fs::create_dir_all(d.path().join("Labels/GBA")).unwrap();
    std::fs::write(dest("Kept"), b"mine").unwrap();
    let wanted: Vec<Wanted> = ["Pokémon Emerald (Rev 1)", "Unknown Hack", "Kept"]
        .iter()
        .map(|s| Wanted {
            platform: Platform::Gba,
            stem: s.to_string(),
            dest: dest(s),
        })
        .collect();
    let png = b"\x89PNG\r\n\x1a\nart".to_vec();
    let tally = fetch_all(&wanted, |u| {
        (u.ends_with("/Pokemon%20Emerald.png")).then(|| png.clone())
    });
    assert_eq!((tally.found, tally.missing), (1, 1));
    assert_eq!(std::fs::read(dest("Pokémon Emerald (Rev 1)")).unwrap(), png);
    assert!(!dest("Unknown Hack").exists());
    assert_eq!(std::fs::read(dest("Kept")).unwrap(), b"mine");
}

#[test]
fn an_error_page_is_not_saved_as_a_label() {
    let d = tempfile::tempdir().unwrap();
    let w = Wanted {
        platform: Platform::Gb,
        stem: "Tetris".into(),
        dest: d.path().join("Tetris.png"),
    };
    let tally = fetch_all(&[w.clone()], |_| Some(b"<html>404</html>".to_vec()));
    assert_eq!(tally.missing, 1);
    assert!(!w.dest.exists());
}

#[test]
fn the_github_mirror_is_tried_when_the_server_has_nothing() {
    let d = tempfile::tempdir().unwrap();
    let w = Wanted {
        platform: Platform::Gba,
        stem: "Metroid Fusion (USA)".into(),
        dest: d.path().join("Metroid Fusion (USA).png"),
    };
    let tally = fetch_all(&[w.clone()], |u| {
        (u == "https://raw.githubusercontent.com/libretro-thumbnails/Nintendo_-_Game_Boy_Advance/master/Named_Boxarts/Metroid%20Fusion%20%28USA%29.png")
            .then(|| b"\x89PNG\r\n\x1a\nart".to_vec())
    });
    assert_eq!(tally.found, 1);
}
