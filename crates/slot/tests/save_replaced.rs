use slot::persist::{note_save, save_replaced, write_sav_as};
use slot_store::{Platform, SaveFormat};

fn saves(root: &std::path::Path) -> std::path::PathBuf {
    slot::root::saves_dir(root).join(Platform::Gba.dir_name())
}

#[test]
fn a_game_slot_never_noted_reads_as_unchanged() {
    let d = tempfile::tempdir().unwrap();
    assert!(!save_replaced(d.path(), Platform::Gba, "Emerald"));
}

#[test]
fn a_save_slot_wrote_is_not_taken_for_a_new_one() {
    let d = tempfile::tempdir().unwrap();
    write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &[1; 1024],
        SaveFormat::RetroArch,
    )
    .unwrap();
    assert!(!save_replaced(d.path(), Platform::Gba, "Emerald"));
    write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &[2; 1024],
        SaveFormat::RetroArch,
    )
    .unwrap();
    assert!(!save_replaced(d.path(), Platform::Gba, "Emerald"));
}

#[test]
fn a_save_copied_over_the_one_slot_wrote_is_seen() {
    let d = tempfile::tempdir().unwrap();
    write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &[1; 1024],
        SaveFormat::RetroArch,
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(saves(d.path()).join("Emerald.srm"), [9u8; 2048]).unwrap();
    assert!(save_replaced(d.path(), Platform::Gba, "Emerald"));
    note_save(d.path(), Platform::Gba, "Emerald").unwrap();
    assert!(!save_replaced(d.path(), Platform::Gba, "Emerald"));
}

#[test]
fn a_save_dropped_in_beside_the_one_slot_wrote_is_seen() {
    let d = tempfile::tempdir().unwrap();
    write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &[1; 1024],
        SaveFormat::RetroArch,
    )
    .unwrap();
    std::fs::write(saves(d.path()).join("Emerald.sav"), [9u8; 1024]).unwrap();
    assert!(save_replaced(d.path(), Platform::Gba, "Emerald"));
}
