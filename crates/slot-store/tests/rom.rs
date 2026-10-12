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
    assert_eq!(rom::open(&rom_path).unwrap(), b"HXYLO");
}

#[test]
fn a_zipped_rom_is_the_file_inside() {
    let d = tmp_root();
    let zip = slot_store::zip::store("inside.gba", b"from-zip");
    let path = d.path().join("Games/GBA/inside.zip");
    std::fs::write(&path, zip).unwrap();
    assert_eq!(rom::open(&path).unwrap(), b"from-zip");
}