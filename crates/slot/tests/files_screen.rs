mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::App;
use slot_input::{Action, Btn};
use slot_ui::QuickRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn files() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    std::fs::write(d.path().join("Saves/GBA/Fusion.srm"), b"save").unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(QuickRow::Tools) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::Files));
    press(&mut a, Btn::A);
    assert!(a.files_open());
    (d, a)
}

fn names(a: &App) -> Vec<String> {
    a.files_explorer()
        .unwrap()
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect()
}

/// Puts the cursor on a name and goes in if it is a folder.
fn open(a: &mut App, name: &str) {
    while a.files_explorer().unwrap().current().unwrap().name != name {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
}

fn finish(a: &mut App) {
    for _ in 0..500 {
        if !a.files_busy() {
            return;
        }
        a.update(0.01);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    panic!("the job never ended");
}

fn popup_rows(a: &App) -> Vec<String> {
    let lines = a.settings_lines().expect("popup text");
    (0..slot_ui::POPUP_OPTIONS)
        .map(|i| lines[slot_ui::option_slot(i)].0.clone())
        .collect()
}

#[test]
fn files_shows_the_card_and_b_walks_back_out() {
    let (_d, mut a) = files();
    let lines = a.files_lines().unwrap();
    assert_eq!(lines[slot_ui::FILES_TITLE].0, "SD card");
    assert!(names(&a).contains(&"Games".to_string()));
    open(&mut a, "Saves");
    open(&mut a, "GBA");
    let lines = a.files_lines().unwrap();
    assert_eq!(lines[slot_ui::FILES_TITLE].0, "SD card/Saves/GBA");
    assert!(lines[slot_ui::files_name_slot(0)].0.ends_with("Fusion.srm"));
    assert_eq!(lines[slot_ui::files_size_slot(0)].0, "4 B");
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    assert!(a.files_open());
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Files));
}

#[test]
fn copy_and_paste_puts_a_save_in_another_folder() {
    let (d, mut a) = files();
    open(&mut a, "Saves");
    open(&mut a, "GBA");
    press(&mut a, Btn::X);
    assert_eq!(popup_rows(&a)[0], "Copy");
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    open(&mut a, "Games");
    press(&mut a, Btn::Start);
    assert!(a.files_busy());
    finish(&mut a);
    assert_eq!(
        std::fs::read(d.path().join("Games/Fusion.srm")).unwrap(),
        b"save"
    );
    assert!(d.path().join("Saves/GBA/Fusion.srm").exists());
    assert!(names(&a).contains(&"Fusion.srm".to_string()));
}

#[test]
fn delete_asks_first_and_cancel_keeps_the_file() {
    let (d, mut a) = files();
    open(&mut a, "Saves");
    open(&mut a, "GBA");
    press(&mut a, Btn::X);
    let at = popup_rows(&a)
        .iter()
        .position(|r| r == "Delete")
        .expect("no Delete");
    for _ in 0..at {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    let lines = a.settings_lines().unwrap();
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Delete Fusion.srm?");
    press(&mut a, Btn::A);
    assert!(
        d.path().join("Saves/GBA/Fusion.srm").exists(),
        "Cancel deleted it"
    );
    press(&mut a, Btn::X);
    for _ in 0..at {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    press(&mut a, Btn::Up);
    press(&mut a, Btn::A);
    finish(&mut a);
    assert!(!d.path().join("Saves/GBA/Fusion.srm").exists());
}

#[test]
fn rename_types_a_new_name_on_the_keyboard() {
    let (d, mut a) = files();
    open(&mut a, "Saves");
    open(&mut a, "GBA");
    press(&mut a, Btn::X);
    let at = popup_rows(&a)
        .iter()
        .position(|r| r == "Rename")
        .expect("no Rename");
    for _ in 0..at {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    let lines = a.search_lines().expect("the keyboard");
    assert_eq!(lines[slot_ui::SEARCH_QUERY].0, "Rename");
    assert_eq!(lines[slot_ui::search_hit_slot(1)].0, "Fusion.srm_");
    // Take off ".srm" and put a 2 on the end.
    for _ in 0..4 {
        press(&mut a, Btn::B);
    }
    press(&mut a, Btn::A); // "1", the first key
    press(&mut a, Btn::Start);
    assert!(a.search_lines().is_none(), "the keyboard stayed up");
    assert!(d.path().join("Saves/GBA/Fusion1").exists());
}

#[test]
fn system_offers_no_way_to_change_it() {
    let (_d, mut a) = files();
    open(&mut a, "System");
    if a.files_explorer().unwrap().entries.is_empty() {
        return;
    }
    press(&mut a, Btn::X);
    let rows = popup_rows(&a);
    for bad in ["Cut", "Rename", "Delete", "New Folder", "Paste Here"] {
        assert!(
            !rows.iter().any(|r| r == bad),
            "{bad} offered in System: {rows:?}"
        );
    }
}
