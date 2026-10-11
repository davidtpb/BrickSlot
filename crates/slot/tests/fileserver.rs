use std::io::{Read, Write};
use std::net::TcpStream;

use slot::fileserver::{allowed, decode, FileServer};

fn http(port: u16, method: &str, target: &str, body: &[u8]) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        s,
        "{method} {target} HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    s.write_all(body).unwrap();
    let mut reply = Vec::new();
    s.read_to_end(&mut reply).unwrap();
    let head_end = reply.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let code = std::str::from_utf8(&reply[9..12]).unwrap().parse().unwrap();
    (code, reply[head_end + 4..].to_vec())
}

fn card() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for dir in ["Games/GBA", "Saves/GBA", "System"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    std::fs::write(d.path().join("Saves/GBA/Emerald.srm"), b"save").unwrap();
    std::fs::write(d.path().join("System/slot"), b"binary").unwrap();
    d
}

#[test]
fn the_page_lists_the_cards_folders_and_their_files() {
    let d = card();
    let srv = FileServer::start(d.path(), 0).unwrap();
    let (code, body) = http(srv.port, "GET", "/", b"");
    let body = String::from_utf8(body).unwrap();
    assert_eq!(code, 200);
    assert!(body.contains("/browse?dir=Games/GBA"), "{body}");
    assert!(!body.contains("System"), "the system folder is on the page");
    let (_, body) = http(srv.port, "GET", "/browse?dir=Saves/GBA", b"");
    assert!(String::from_utf8(body).unwrap().contains("Emerald.srm"));
}

#[test]
fn files_go_up_come_down_and_can_be_deleted() {
    let d = card();
    let srv = FileServer::start(d.path(), 0).unwrap();
    let rom = vec![0x5Au8; 300_000];
    let (code, _) = http(
        srv.port,
        "PUT",
        "/put?dir=Games%2FGBA&name=Pok%C3%A9mon%20Emerald.gba",
        &rom,
    );
    assert_eq!(code, 200);
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA/Pokémon Emerald.gba")).unwrap(),
        rom
    );
    let (code, body) = http(
        srv.port,
        "GET",
        "/get?path=Games/GBA/Pok%C3%A9mon%20Emerald.gba",
        b"",
    );
    assert_eq!((code, body.len()), (200, rom.len()));
    let (code, _) = http(
        srv.port,
        "POST",
        "/delete?path=Games/GBA/Pok%C3%A9mon%20Emerald.gba",
        b"",
    );
    assert_eq!(code, 200);
    assert!(!d.path().join("Games/GBA/Pokémon Emerald.gba").exists());
}

#[test]
fn nothing_outside_the_allowed_folders_can_be_reached() {
    let d = card();
    let srv = FileServer::start(d.path(), 0).unwrap();
    for target in [
        "/get?path=System/slot",
        "/get?path=Saves/../System/slot",
        "/get?path=/etc/passwd",
        "/browse?dir=System",
    ] {
        let (code, body) = http(srv.port, "GET", target, b"");
        assert_ne!(code, 200, "{target} was served");
        assert_ne!(body, b"binary");
    }
    let (code, _) = http(
        srv.port,
        "PUT",
        "/put?dir=Games/GBA&name=..%2F..%2FSystem%2Fslot",
        b"evil",
    );
    assert_eq!(code, 400);
    assert_eq!(
        std::fs::read(d.path().join("System/slot")).unwrap(),
        b"binary"
    );
    let (code, _) = http(srv.port, "POST", "/delete?path=Saves", b"");
    assert_ne!(code, 200, "a whole top folder was deletable");
    assert_eq!(
        allowed("Games/GBA/x.gba").unwrap().to_string_lossy(),
        "Games/GBA/x.gba"
    );
    assert!(allowed("Games/.hidden").is_none());
    assert!(allowed("Config/slot.state").is_none());
}

#[test]
fn url_decoding_survives_odd_input() {
    assert_eq!(decode("a%20b+c"), "a b c");
    assert_eq!(decode("Pok%C3%A9mon"), "Pokémon");
    assert_eq!(decode("100%"), "100%");
    assert_eq!(decode("%zz%é"), "%zz%é");
}
