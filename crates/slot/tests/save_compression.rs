mod common;

use slot::persist::{read_sav, write_sav_as};
use slot_store::{rzip, Platform, SaveFormat};

fn sram() -> Vec<u8> {
    (0..32 * 1024).map(|i| (i * 13 % 256) as u8).collect()
}

fn srm(d: &tempfile::TempDir, stem: &str, ext: &str) -> std::path::PathBuf {
    slot::root::saves_dir(d.path())
        .join(Platform::Gba.dir_name())
        .join(format!("{stem}.{ext}"))
}

fn put(d: &tempfile::TempDir, stem: &str, ext: &str, bytes: &[u8]) {
    let path = srm(d, stem, ext);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_compressed_retroarch_save_is_read_inflated() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    put(&d, "Emerald", "srm", &rzip::compress(&sram()));
    assert_eq!(read_sav(d.path(), Platform::Gba, "Emerald"), Some(sram()));
}

#[test]
fn a_compressed_clock_file_is_read_inflated_too() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let rtc = [7u8; 16];
    put(&d, "Emerald", "srm", &rzip::compress(&sram()));
    put(&d, "Emerald", "rtc", &rzip::compress(&rtc));
    let mut want = sram();
    want.extend_from_slice(&rtc);
    assert_eq!(read_sav(d.path(), Platform::Gba, "Emerald"), Some(want));
}

#[test]
fn the_compressed_format_writes_rzip_and_the_plain_one_does_not() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &sram(),
        SaveFormat::RetroArchCompressed,
    )
    .unwrap();
    let on_card = std::fs::read(srm(&d, "Emerald", "srm")).unwrap();
    assert!(rzip::is_rzip(&on_card));
    assert!(on_card.len() < sram().len());
    assert_eq!(read_sav(d.path(), Platform::Gba, "Emerald"), Some(sram()));

    let wrote = write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &sram(),
        SaveFormat::RetroArch,
    )
    .unwrap();
    assert!(wrote, "the same save was not rewritten plain");
    assert_eq!(std::fs::read(srm(&d, "Emerald", "srm")).unwrap(), sram());
}

#[test]
fn an_unchanged_compressed_save_is_not_rewritten() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    put(&d, "Emerald", "srm", &rzip::compress(&sram()));
    let wrote = write_sav_as(
        d.path(),
        Platform::Gba,
        "Emerald",
        &sram(),
        SaveFormat::RetroArchCompressed,
    )
    .unwrap();
    assert!(!wrote);
}

#[test]
fn a_broken_compressed_save_is_kept_aside_not_fed_to_the_game() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let mut broken = rzip::compress(&sram());
    broken.truncate(broken.len() / 2);
    put(&d, "Emerald", "srm", &broken);
    assert_eq!(read_sav(d.path(), Platform::Gba, "Emerald"), None);
    assert_eq!(
        std::fs::read(srm(&d, "Emerald", "srm.broken")).unwrap(),
        broken
    );
}
