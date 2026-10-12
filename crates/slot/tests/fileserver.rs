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

#[cfg(unix)]
#[test]
fn a_symlink_cannot_lead_outside_the_allowed_folders() {
    use std::os::unix::fs::symlink;
    let d = card();
    // A folder link and a file link, both into System.
    symlink(d.path().join("System"), d.path().join("Saves/out")).unwrap();
    symlink(
        d.path().join("System/slot"),
        d.path().join("Saves/GBA/evil.srm"),
    )
    .unwrap();
    let srv = FileServer::start(d.path(), 0).unwrap();
    for target in [
        "/get?path=Saves/out/slot",
        "/get?path=Saves/GBA/evil.srm",
        "/browse?dir=Saves/out",
    ] {
        let (code, body) = http(srv.port, "GET", target, b"");
        assert_ne!(code, 200, "{target} was served");
        assert_ne!(body, b"binary");
    }
    for target in [
        "/put?dir=Saves/out&name=slot",
        "/put?dir=Saves/GBA&name=evil.srm",
    ] {
        let (code, _) = http(srv.port, "PUT", target, b"evil");
        assert_ne!(code, 200, "{target} was written");
    }
    // A link where the upload's temporary file goes is not written through.
    symlink(
        d.path().join("System/slot"),
        d.path().join("Saves/GBA/.new.srm.part"),
    )
    .unwrap();
    let (code, _) = http(srv.port, "PUT", "/put?dir=Saves/GBA&name=new.srm", b"evil");
    assert_ne!(code, 200);
    assert_eq!(
        std::fs::read(d.path().join("System/slot")).unwrap(),
        b"binary",
        "a link let a file outside be overwritten"
    );
    let (_, page) = http(srv.port, "GET", "/browse?dir=Saves", b"");
    let page = String::from_utf8_lossy(&page);
    assert!(!page.contains("out/"), "the folder link is listed");
    let (code, _) = http(srv.port, "GET", "/get?path=Saves/GBA/Emerald.srm", b"");
    assert_eq!(code, 200, "a real file stopped being served");
}

#[test]
fn an_oversized_or_endless_request_head_is_turned_away() {
    let d = card();
    let srv = FileServer::start(d.path(), 0).unwrap();
    let long = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(64 * 1024));
    let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    let _ = s.write_all(long.as_bytes());
    let mut reply = Vec::new();
    let _ = s.read_to_end(&mut reply);
    assert!(
        !reply.starts_with(b"HTTP/1.1 200"),
        "a 64 KB line was taken"
    );

    let mut many = String::from("GET / HTTP/1.1\r\n");
    for i in 0..200 {
        many.push_str(&format!("X-{i}: y\r\n"));
    }
    many.push_str("\r\n");
    let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    let _ = s.write_all(many.as_bytes());
    let mut reply = Vec::new();
    let _ = s.read_to_end(&mut reply);
    assert!(
        !reply.starts_with(b"HTTP/1.1 200"),
        "200 headers were taken"
    );

    let (code, _) = http(srv.port, "GET", "/", b"");
    assert_eq!(code, 200, "the server stopped answering");
}

#[test]
fn idle_connections_cannot_take_every_worker() {
    use slot::fileserver::CONNECTIONS_MAX;
    let d = card();
    let srv = FileServer::start(d.path(), 0).unwrap();
    let idle: Vec<TcpStream> = (0..CONNECTIONS_MAX)
        .map(|_| TcpStream::connect(("127.0.0.1", srv.port)).unwrap())
        .collect();
    std::thread::sleep(std::time::Duration::from_millis(400));
    // Past the limit a request is answered busy, or the connection is
    // simply closed on it.
    let mut s = TcpStream::connect(("127.0.0.1", srv.port)).unwrap();
    let _ = s.write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n");
    let mut reply = Vec::new();
    let _ = s.read_to_end(&mut reply);
    assert!(
        !reply.starts_with(b"HTTP/1.1 200"),
        "one more connection was served past the limit"
    );
    drop(idle);
    std::thread::sleep(std::time::Duration::from_millis(400));
    let (code, _) = http(srv.port, "GET", "/", b"");
    assert_eq!(code, 200, "the workers were not given back");
}
