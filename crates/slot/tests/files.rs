use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use slot::files::{run, Explorer, FileJob, FileNews};

fn card() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for dir in ["Games/GBA", "Games/GB", "Saves/GBA", "System"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    std::fs::write(d.path().join("Games/GBA/Emerald.gba"), vec![7u8; 300_000]).unwrap();
    std::fs::write(d.path().join("Games/GBA/Fusion.gba"), b"fusion").unwrap();
    std::fs::write(d.path().join("Saves/GBA/Emerald.srm"), b"save").unwrap();
    std::fs::write(d.path().join("System/slot"), b"binary").unwrap();
    std::fs::write(d.path().join(".hidden"), b"x").unwrap();
    d
}

/// Goes into the named folder of the one shown.
fn go(e: &mut Explorer, name: &str) {
    e.at = e
        .entries
        .iter()
        .position(|x| x.name == name)
        .unwrap_or_else(|| panic!("no {name}"));
    assert!(e.enter(), "could not go into {name}");
}

fn job(root: &Path, job: FileJob) -> (usize, Vec<String>, Vec<FileNews>) {
    let (tx, rx) = std::sync::mpsc::channel();
    run(root, &job, &tx, &AtomicBool::new(false));
    let news: Vec<FileNews> = rx.try_iter().collect();
    match news.last() {
        Some(FileNews::Done { items, errors }) => (*items, errors.clone(), news),
        other => panic!("no end: {other:?}"),
    }
}

#[test]
fn the_card_lists_folders_first_and_hides_dot_files() {
    let d = card();
    let mut e = Explorer::new(d.path());
    let names: Vec<&str> = e.entries.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Games", "Saves", "System"]);
    e.toggle_hidden();
    assert!(e.entries.iter().any(|x| x.name == ".hidden"));
    e.toggle_hidden();
    go(&mut e, "Games");
    assert_eq!(e.cwd, PathBuf::from("Games"));
    go(&mut e, "GBA");
    let names: Vec<&str> = e.entries.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, ["Emerald.gba", "Fusion.gba"]);
    assert_eq!(e.entries[0].size, 300_000);
    assert!(!e.enter(), "a file was gone into");
    assert!(e.up());
    assert_eq!(e.current().unwrap().name, "GBA", "up lost its place");
    assert!(e.up());
    assert!(!e.up(), "went above the card");
}

#[test]
fn copy_paste_makes_a_numbered_copy_and_reports_progress() {
    let d = card();
    let (items, errors, news) = job(
        d.path(),
        FileJob::Paste {
            items: vec!["Games/GBA/Emerald.gba".into()],
            cut: false,
            into: "Games/GBA".into(),
        },
    );
    assert_eq!((items, errors.len()), (1, 0), "{errors:?}");
    let copy = std::fs::read(d.path().join("Games/GBA/Emerald (2).gba")).unwrap();
    assert_eq!(copy.len(), 300_000);
    assert!(d.path().join("Games/GBA/Emerald.gba").exists());
    assert!(news
        .iter()
        .any(|n| matches!(n, FileNews::Progress { total, .. } if *total == 300_000)));
}

#[test]
fn cut_paste_moves_a_folder_with_what_is_in_it() {
    let d = card();
    let (items, errors, _) = job(
        d.path(),
        FileJob::Paste {
            items: vec!["Saves/GBA".into()],
            cut: true,
            into: "Games".into(),
        },
    );
    assert_eq!((items, errors.len()), (1, 0), "{errors:?}");
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA (2)/Emerald.srm")).unwrap(),
        b"save"
    );
    assert!(!d.path().join("Saves/GBA").exists());
}

#[test]
fn system_and_the_top_folders_cannot_be_changed() {
    let d = card();
    let (items, errors, _) = job(
        d.path(),
        FileJob::Delete {
            items: vec!["System/slot".into(), "Games".into(), "Games/GB".into()],
        },
    );
    assert_eq!(items, 1, "{errors:?}");
    assert_eq!(errors.len(), 2);
    assert!(d.path().join("System/slot").exists());
    assert!(d.path().join("Games").exists());
    assert!(!d.path().join("Games/GB").exists());
    let (items, errors, _) = job(
        d.path(),
        FileJob::Paste {
            items: vec!["Games/GBA/Fusion.gba".into()],
            cut: false,
            into: "System".into(),
        },
    );
    assert_eq!(items, 0);
    assert!(!errors.is_empty());
    assert!(!d.path().join("System/Fusion.gba").exists());
}

#[test]
fn a_folder_cannot_be_pasted_inside_itself() {
    let d = card();
    let (items, errors, _) = job(
        d.path(),
        FileJob::Paste {
            items: vec!["Games/GBA".into()],
            cut: false,
            into: "Games/GBA".into(),
        },
    );
    assert_eq!(items, 0);
    assert_eq!(errors.len(), 1);
}

#[test]
fn rename_and_new_folder_refuse_bad_and_taken_names() {
    let d = card();
    let mut e = Explorer::new(d.path());
    go(&mut e, "Games");
    go(&mut e, "GBA");
    assert!(e.rename("Fusion.gba").is_err(), "a taken name was used");
    assert!(e.rename("a/b").is_err());
    e.rename("Ruby.gba").unwrap();
    assert_eq!(e.current().unwrap().name, "Ruby.gba");
    assert!(d.path().join("Games/GBA/Ruby.gba").exists());
    e.new_folder("Hacks").unwrap();
    assert!(d.path().join("Games/GBA/Hacks").is_dir());
    assert_eq!(e.current().unwrap().name, "Hacks");
    e.up();
    e.up();
    go(&mut e, "System");
    assert!(e.new_folder("x").is_err(), "a folder was made in System");
    assert!(e.rename("x").is_err());
}

#[test]
fn ticked_items_are_what_an_action_works_on() {
    let d = card();
    let mut e = Explorer::new(d.path());
    go(&mut e, "Games");
    go(&mut e, "GBA");
    assert_eq!(e.chosen(), [PathBuf::from("Games/GBA/Emerald.gba")]);
    e.tick();
    e.step(1);
    e.tick();
    assert_eq!(e.chosen().len(), 2);
    assert_eq!(e.copy(true), 2);
    assert!(e.ticked.is_empty());
    assert!(e.clip.as_ref().unwrap().cut);
}

#[cfg(unix)]
#[test]
fn links_are_listed_but_never_followed() {
    let d = card();
    std::os::unix::fs::symlink(d.path().join("System"), d.path().join("Games/out")).unwrap();
    let mut e = Explorer::new(d.path());
    go(&mut e, "Games");
    let link = e.entries.iter().position(|x| x.name == "out").unwrap();
    assert!(e.entries[link].link);
    e.at = link;
    assert!(!e.enter(), "a link was gone into");
    let (_, _, _) = job(
        d.path(),
        FileJob::Paste {
            items: vec!["Games/out".into()],
            cut: false,
            into: "Saves".into(),
        },
    );
    assert!(
        !d.path().join("Saves/out/slot").exists(),
        "a link was copied through"
    );
}
