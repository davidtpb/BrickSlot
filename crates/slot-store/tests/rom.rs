mod common;

use common::tmp_root;
use slot_store::rom;

#[test]
fn a_sibling_ips_is_applied_when_the_rom_is_opened() {
    let d = tmp_root();
    let rom_path = d.path().join("Games/GBA/PatchMe.gba");
    std::fs::write(&rom_path, b"HELLO").unwrap();
    let mut patch = b"PATCH".to_vec();
    patch.extend_from_slice(&[0, 0, 1, 0, 3]);
    patch.extend_from_slice(b"XYZ");
    patch.extend_from_slice(b"EOF");
    std::fs::write(d.path().join("Games/GBA/PatchMe.ips"), patch).unwrap();
    assert_eq!(rom::open(&rom_path).unwrap(), b"HXYZO");
}

#[test]
fn a_zipped_rom_is_the_file_inside() {
    let d = tmp_root();
    let zip = slot_store::zip::store("inside.gba", b"from-zip");
    let path = d.path().join("Games/GBA/inside.zip");
    std::fs::write(&path, zip).unwrap();
    assert_eq!(rom::open(&path).unwrap(), b"from-zip");
}

#[test]
fn a_zipped_rom_is_staged_to_a_file_a_path_loading_core_can_open() {
    let d = tempfile::tempdir().unwrap();
    let zipped = d.path().join("Emerald.zip");
    std::fs::write(&zipped, slot_store::zip::store("Emerald.gba", b"rom-bytes")).unwrap();
    let (path, bytes) = slot_store::rom::stage(&zipped).unwrap();
    assert_eq!(bytes, b"rom-bytes");
    assert_eq!(path.extension().unwrap(), "gba");
    assert_eq!(std::fs::read(&path).unwrap(), b"rom-bytes");

    let loose = d.path().join("Fusion.gba");
    std::fs::write(&loose, b"plain").unwrap();
    let (path, bytes) = slot_store::rom::stage(&loose).unwrap();
    assert_eq!(path, loose, "a plain ROM was copied for nothing");
    assert_eq!(bytes, b"plain");
}
