//! A small web page for moving games, saves and art on and off the card
//! over Wi-Fi. It only ever reaches inside the folders in `ROOTS`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PORT: u16 = 8080;

/// The folders the page shows. Nothing outside them can be read or written.
pub const ROOTS: [&str; 7] = [
    "Games",
    "Saves",
    "States",
    "Labels",
    "Screenshots",
    "BIOS",
    "Cheats",
];

const UPLOAD_MAX: u64 = 128 << 20;

/// How many requests are served at once; more are turned away.
pub const CONNECTIONS_MAX: usize = 8;
/// The longest request line or header line taken.
pub const LINE_MAX: usize = 8 * 1024;
/// The most header lines taken.
const HEADERS_MAX: usize = 64;
/// How long a client has to send its whole request head.
const HEAD_WITHIN: Duration = Duration::from_secs(10);

/// Wrong PINs allowed before the page locks.
pub const PIN_TRIES: u32 = 5;
/// How long the page stays locked after too many wrong PINs.
const PIN_LOCK: Duration = Duration::from_secs(60);

/// Wrong PINs so far, and when a lock after too many ends.
#[derive(Default)]
struct PinGuard {
    wrong: u32,
    locked_until: Option<Instant>,
}

/// A PIN no one can work out from the clock: four digits from the
/// randomly seeded hasher every process gets.
pub fn random_pin() -> u16 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    1000 + (h.finish() % 9000) as u16
}

/// Counts a request being served while it lives.
struct Slot(Arc<AtomicUsize>);

impl Slot {
    fn take(active: &Arc<AtomicUsize>) -> Option<Slot> {
        let mut now = active.load(Ordering::Acquire);
        loop {
            if now >= CONNECTIONS_MAX {
                return None;
            }
            match active.compare_exchange(now, now + 1, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return Some(Slot(active.clone())),
                Err(seen) => now = seen,
            }
        }
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub struct FileServer {
    stop: Arc<AtomicBool>,
    pub port: u16,
}

impl FileServer {
    /// Listens on `port` (0 for any free port) until dropped.
    pub fn start(root: &Path, port: u16) -> std::io::Result<FileServer> {
        Self::start_pin(root, port, None)
    }

    /// Same as [`start`], but every page needs `pin` (the four digits shown
    /// on the handheld).
    pub fn start_pin(root: &Path, port: u16, pin: Option<u16>) -> std::io::Result<FileServer> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let root = root.to_path_buf();
        let active = Arc::new(AtomicUsize::new(0));
        let guard = Arc::new(std::sync::Mutex::new(PinGuard::default()));
        std::thread::Builder::new()
            .name("slot-files".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let Some(slot) = Slot::take(&active) else {
                                let mut stream = stream;
                                let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                                let _ = Reply::status(503, "Busy, try again").write(&mut stream);
                                continue;
                            };
                            let root = root.clone();
                            let guard = guard.clone();
                            let _ = std::thread::Builder::new()
                                .name("slot-files-req".into())
                                .spawn(move || {
                                    serve(&root, stream, pin, &guard);
                                    drop(slot);
                                });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(100))
                        }
                        Err(e) => {
                            eprintln!("slot: files: {e}");
                            std::thread::sleep(Duration::from_millis(500));
                        }
                    }
                }
            })?;
        Ok(FileServer { stop, port })
    }
}

impl Drop for FileServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    length: u64,
    cookie: String,
}

impl Request {
    fn param(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// One line of the request head, refused past `LINE_MAX` or the deadline.
fn read_line(reader: &mut BufReader<TcpStream>, by: Instant) -> Option<String> {
    if Instant::now() > by {
        return None;
    }
    let mut buf = Vec::new();
    let n = reader
        .by_ref()
        .take(LINE_MAX as u64 + 1)
        .read_until(b'\n', &mut buf)
        .ok()?;
    if n > LINE_MAX || (n > 0 && !buf.ends_with(b"\n")) {
        return None;
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Option<Request> {
    let by = Instant::now() + HEAD_WITHIN;
    let line = read_line(reader, by)?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut length = 0;
    let mut cookie = String::new();
    let mut headers = 0;
    loop {
        let header = read_line(reader, by)?;
        if header.is_empty() {
            break;
        }
        headers += 1;
        if headers > HEADERS_MAX {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                length = v.trim().parse().unwrap_or(0);
            }
            if k.trim().eq_ignore_ascii_case("cookie") {
                cookie = v.trim().to_string();
            }
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (decode(k), decode(v))
        })
        .collect();
    Some(Request {
        method,
        path: decode(path),
        query,
        length,
        cookie,
    })
}

pub fn decode(s: &str) -> String {
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (
            bytes[i],
            bytes.get(i + 1).copied(),
            bytes.get(i + 2).copied(),
        ) {
            (b'+', _, _) => out.push(b' '),
            (b'%', Some(h), Some(l)) => match (hex(h), hex(l)) {
                (Some(h), Some(l)) => {
                    out.push(h << 4 | l);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            (b, _, _) => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// A card-relative path the page may touch: inside one of `ROOTS`, with no
/// `..`, no absolute parts and nothing hidden.
pub fn allowed(rel: &str) -> Option<PathBuf> {
    let rel = rel.trim_matches('/');
    let path = Path::new(rel);
    let mut parts = Vec::new();
    for c in path.components() {
        match c {
            Component::Normal(p) => {
                let p = p.to_str()?;
                if p.starts_with('.') {
                    return None;
                }
                parts.push(p);
            }
            _ => return None,
        }
    }
    if !ROOTS.contains(parts.first()?) {
        return None;
    }
    Some(parts.iter().collect())
}

/// Whether every part of `rel` that exists under `root` is a real file or
/// folder, not a symbolic link that could lead outside the allowed folders.
pub fn no_links(root: &Path, rel: &Path) -> bool {
    let mut at = root.to_path_buf();
    for part in rel.components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return false,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => return false,
        }
    }
    true
}

/// `allowed`, and free of symbolic links on the card.
fn reachable(root: &Path, rel: &str) -> Option<PathBuf> {
    allowed(rel).filter(|p| no_links(root, p))
}

fn cookie_pin(cookie: &str) -> Option<u16> {
    cookie.split(';').find_map(|part| {
        let (k, v) = part.trim().split_once('=')?;
        (k == "slotpin").then(|| v.parse().ok())?
    })
}

enum PinCheck {
    Open,
    Ask,
    Locked,
}

/// Whether a request may in: no PIN set or the right one given. A wrong
/// PIN counts against the guard, and too many lock the page for a while.
fn pin_check(req: &Request, pin: Option<u16>, guard: &std::sync::Mutex<PinGuard>) -> PinCheck {
    let Some(want) = pin else {
        return PinCheck::Open;
    };
    let mut g = guard.lock().unwrap_or_else(|e| e.into_inner());
    let now = Instant::now();
    if g.locked_until.is_some_and(|t| now < t) {
        return PinCheck::Locked;
    }
    g.locked_until = None;
    let given = req
        .param("pin")
        .map(|p| p.parse::<u16>().ok())
        .or_else(|| cookie_pin(&req.cookie).map(Some));
    match given {
        None => PinCheck::Ask,
        Some(Some(p)) if p == want => {
            g.wrong = 0;
            PinCheck::Open
        }
        Some(_) => {
            g.wrong += 1;
            if g.wrong >= PIN_TRIES {
                g.wrong = 0;
                g.locked_until = Some(now + PIN_LOCK);
                return PinCheck::Locked;
            }
            PinCheck::Ask
        }
    }
}

fn pin_page() -> String {
    page(
        "PIN",
        "<h1>File transfer</h1><p>Enter the four-digit PIN shown on the handheld.</p>\
         <form method=get><input name=pin inputmode=numeric pattern=[0-9]{4} maxlength=4 autofocus>\
         <button>Open</button></form>",
    )
}

fn serve(root: &Path, stream: TcpStream, pin: Option<u16>, guard: &std::sync::Mutex<PinGuard>) {
    let _ = stream.set_read_timeout(Some(HEAD_WITHIN));
    let Ok(write_half) = stream.try_clone() else {
        return;
    };
    let mut out = write_half;
    let mut reader = BufReader::new(stream);
    let Some(req) = read_request(&mut reader) else {
        return;
    };
    match pin_check(&req, pin, guard) {
        PinCheck::Open => {}
        PinCheck::Ask => {
            let _ = Reply::html(pin_page()).write(&mut out);
            return;
        }
        PinCheck::Locked => {
            let _ = Reply::status(429, "Too many wrong PINs. Wait a minute and try again")
                .write(&mut out);
            return;
        }
    }
    let set_pin = pin.filter(|_| req.param("pin").is_some());
    let reply = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => Reply::html(index_page(root)),
        ("GET", "/browse") => match req.param("dir").and_then(|d| reachable(root, d)) {
            Some(rel) => Reply::html(browse_page(root, &rel)),
            None => Reply::status(404, "No such folder"),
        },
        ("GET", "/get") => match req.param("path").and_then(|p| reachable(root, p)) {
            Some(rel) => return send_file(&mut out, &root.join(rel)),
            None => Reply::status(404, "No such file"),
        },
        ("PUT", "/put") => match (
            req.param("dir").and_then(|d| reachable(root, d)),
            req.param("name"),
        ) {
            (Some(dir), Some(name)) => {
                let _ = reader
                    .get_ref()
                    .set_read_timeout(Some(Duration::from_secs(30)));
                receive(root, &dir, name, req.length, &mut reader)
            }
            _ => Reply::status(400, "Missing folder or name"),
        },
        ("POST", "/delete") => match req.param("path").and_then(|p| reachable(root, p)) {
            Some(rel) if rel.components().count() > 1 => {
                match std::fs::remove_file(root.join(&rel)) {
                    Ok(()) => Reply::status(200, "Deleted"),
                    Err(e) => Reply::status(500, &e.to_string()),
                }
            }
            _ => Reply::status(400, "Not deletable"),
        },
        _ => Reply::status(404, "Not found"),
    };
    let reply = match set_pin {
        Some(n) => reply.cookie(&format!("slotpin={n}; Path=/; SameSite=Strict")),
        None => reply,
    };
    let _ = reply.write(&mut out);
}

struct Reply {
    code: u16,
    kind: &'static str,
    body: Vec<u8>,
    extra: String,
}

impl Reply {
    fn html(body: String) -> Reply {
        Reply {
            code: 200,
            kind: "text/html; charset=utf-8",
            body: body.into_bytes(),
            extra: String::new(),
        }
    }

    fn status(code: u16, text: &str) -> Reply {
        Reply {
            code,
            kind: "text/plain; charset=utf-8",
            body: text.as_bytes().to_vec(),
            extra: String::new(),
        }
    }

    fn cookie(mut self, cookie: &str) -> Reply {
        self.extra = format!("Set-Cookie: {cookie}\r\n");
        self
    }

    fn write(&self, out: &mut TcpStream) -> std::io::Result<()> {
        let reason = match self.code {
            200 => "OK",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            413 => "Too Large",
            429 => "Too Many Requests",
            503 => "Busy",
            _ => "Error",
        };
        write!(
            out,
            "HTTP/1.1 {} {reason}\r\nContent-Type: {}\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
            self.code,
            self.kind,
            self.body.len(),
            self.extra
        )?;
        out.write_all(&self.body)
    }
}

fn send_file(out: &mut TcpStream, path: &Path) {
    let Ok(mut file) = std::fs::File::open(path) else {
        let _ = Reply::status(404, "No such file").write(out);
        return;
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().replace('"', "'"))
        .unwrap_or_default();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {len}\r\nContent-Disposition: attachment; filename=\"{name}\"\r\nConnection: close\r\n\r\n"
    );
    if out.write_all(head.as_bytes()).is_ok() {
        let _ = std::io::copy(&mut file, out);
    }
}

fn receive(
    root: &Path,
    dir: &Path,
    name: &str,
    length: u64,
    body: &mut BufReader<TcpStream>,
) -> Reply {
    let name = Path::new(name);
    let ok_name = name.components().count() == 1
        && matches!(name.components().next(), Some(Component::Normal(_)))
        && !name.to_string_lossy().starts_with('.');
    if !ok_name {
        return Reply::status(400, "Bad file name");
    }
    if length > UPLOAD_MAX {
        return Reply::status(413, "File too large");
    }
    let tmp_name = format!(".{}.part", name.to_string_lossy());
    if !no_links(root, &dir.join(name)) || !no_links(root, &dir.join(&tmp_name)) {
        return Reply::status(403, "Not a plain file");
    }
    let dir = root.join(dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return Reply::status(500, &e.to_string());
    }
    let dest = dir.join(name);
    let tmp = dir.join(tmp_name);
    let written = (|| -> std::io::Result<u64> {
        // A new file only: never one already there, which could be a link
        // put in place since the check.
        let _ = std::fs::remove_file(&tmp);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        let n = std::io::copy(&mut body.take(length), &mut file)?;
        file.sync_all()?;
        Ok(n)
    })();
    match written {
        Ok(n) if n == length => match std::fs::rename(&tmp, &dest) {
            Ok(()) => Reply::status(200, "Saved"),
            Err(e) => Reply::status(500, &e.to_string()),
        },
        Ok(_) => {
            let _ = std::fs::remove_file(&tmp);
            Reply::status(400, "The upload was cut short")
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Reply::status(500, &e.to_string())
        }
    }
}

const STYLE: &str = "body{font-family:system-ui,sans-serif;margin:0 auto;max-width:760px;padding:16px;background:#1d1d22;color:#eee}a{color:#9cf}h1{font-size:1.4em}table{width:100%;border-collapse:collapse}td{padding:6px 4px;border-bottom:1px solid #333;word-break:break-all}td.n{text-align:right;white-space:nowrap;color:#aaa}button{background:#444;color:#eee;border:0;padding:6px 10px;border-radius:6px}.note{color:#aaa;font-size:.9em}#drop{border:2px dashed #555;border-radius:10px;padding:18px;text-align:center;margin:14px 0}";

fn page(title: &str, body: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>{t}</title><style>{STYLE}</style></head><body>{body}<p class=note>brickslot file transfer. Anyone on this Wi-Fi network can use this page while it is on, so turn it off in Network &gt; File Transfer when you are done. New games show on the shelf after a restart.</p></body></html>",
        t = html_escape(title)
    )
}

fn index_page(root: &Path) -> String {
    let mut rows = String::new();
    for top in ROOTS {
        let dir = root.join(top);
        let mut subs: Vec<String> = std::fs::read_dir(&dir)
            .map(|d| {
                d.flatten()
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| !n.starts_with('.'))
                    .collect()
            })
            .unwrap_or_default();
        subs.sort();
        rows.push_str(&format!(
            "<tr><td><a href=\"/browse?dir={}\">{}</a>",
            encode(top),
            html_escape(top)
        ));
        for s in subs {
            let rel = format!("{top}/{s}");
            rows.push_str(&format!(
                " &middot; <a href=\"/browse?dir={}\">{}</a>",
                encode(&rel),
                html_escape(&s)
            ));
        }
        rows.push_str("</td></tr>");
    }
    page(
        "brickslot",
        &format!("<h1>brickslot</h1><table>{rows}</table>"),
    )
}

fn size(n: u64) -> String {
    match n {
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{} KB", n >> 10),
        n => format!("{n} B"),
    }
}

fn browse_page(root: &Path, rel: &Path) -> String {
    let rel_s = rel.to_string_lossy().replace('\\', "/");
    let mut entries: Vec<(bool, String, u64)> = std::fs::read_dir(root.join(rel))
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') {
                        return None;
                    }
                    let meta = std::fs::symlink_metadata(e.path()).ok()?;
                    if meta.file_type().is_symlink() {
                        return None;
                    }
                    Some((meta.is_dir(), name, meta.len()))
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by_key(|(dir, name, _)| (!dir, name.to_lowercase()));
    let mut crumbs = String::from("<a href=\"/\">brickslot</a>");
    let mut walked = String::new();
    for part in rel_s.split('/') {
        if !walked.is_empty() {
            walked.push('/');
        }
        walked.push_str(part);
        crumbs.push_str(&format!(
            " / <a href=\"/browse?dir={}\">{}</a>",
            encode(&walked),
            html_escape(part)
        ));
    }
    let mut rows = String::new();
    for (is_dir, name, len) in &entries {
        let path = format!("{rel_s}/{name}");
        if *is_dir {
            rows.push_str(&format!(
                "<tr><td><a href=\"/browse?dir={}\">{}/</a></td><td></td><td></td></tr>",
                encode(&path),
                html_escape(name)
            ));
        } else {
            rows.push_str(&format!(
                "<tr><td><a href=\"/get?path={p}\">{n}</a></td><td class=n>{s}</td><td class=n><button onclick=\"del('{p}','{j}')\">Delete</button></td></tr>",
                p = encode(&path),
                n = html_escape(name),
                s = size(*len),
                j = html_escape(&name.replace('\\', "\\\\").replace('\'', "\\'")),
            ));
        }
    }
    if entries.is_empty() {
        rows.push_str("<tr><td class=note>Empty</td></tr>");
    }
    let script = format!(
        "<script>const dir='{d}';\
async function up(files){{const s=document.getElementById('status');for(const f of files){{s.textContent='Uploading '+f.name+'…';\
const r=await fetch('/put?dir='+dir+'&name='+encodeURIComponent(f.name),{{method:'PUT',body:f}});\
if(!r.ok){{s.textContent=f.name+': '+await r.text();return;}}}}location.reload();}}\
async function del(p,n){{if(!confirm('Delete '+n+'?'))return;const r=await fetch('/delete?path='+p,{{method:'POST'}});if(r.ok)location.reload();else alert(await r.text());}}\
const z=document.getElementById('drop');z.ondragover=e=>{{e.preventDefault();}};z.ondrop=e=>{{e.preventDefault();up(e.dataTransfer.files);}};</script>",
        d = encode(&rel_s)
    );
    page(
        &rel_s,
        &format!(
            "<h1>{crumbs}</h1><div id=drop>Drop files here, or <input type=file multiple onchange=\"up(this.files)\"><div id=status class=note></div></div><table>{rows}</table>{script}"
        ),
    )
}
