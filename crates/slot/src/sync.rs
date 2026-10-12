//! Device Sync, after NX Redux's: two handhelds on the same Wi-Fi find each
//! other and swap progress, the newer copy of each file winning, so either
//! can be picked up where the other left off.
//!
//! Each device says it is syncing in a UDP broadcast once a second. When two
//! hear each other, the one with the lower id connects to the other over
//! TCP. Each sends a list of its files (path, size, time changed), then
//! every file it has newer or the other lacks; nothing is ever deleted.
//! Messages are a little-endian u32 length and that many bytes.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const SYNC_PORT: u16 = 7213;
const HELLO: &str = "SLOTSYNC1";
/// Clocks two seconds apart are taken to agree.
const SLACK_S: u64 = 2;
const FILE_MAX: u64 = 1 << 30;
const LOOK_FOR: Duration = Duration::from_secs(90);

/// The folders and files that travel. Settings tied to one device (its
/// state, its Wi-Fi) stay put.
const SHARED: [&str; 6] = [
    "Saves",
    "States",
    "Cheats",
    "Config/emulator",
    "Config/favorites.txt",
    "Config/cart_shell.txt",
];
const ROMS: &str = "Games";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub mtime: u64,
}

/// How a sync went, or is going.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncNews {
    Looking,
    Found(String),
    Moving { sent: usize, got: usize },
    Done { sent: usize, got: usize },
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct SyncOptions {
    pub roms: bool,
    /// What this device is called on the other's screen.
    pub name: String,
    /// The UDP port listened on, and where hellos go.
    pub listen: u16,
    pub announce: SocketAddr,
}

impl SyncOptions {
    pub fn lan(roms: bool, name: String) -> Self {
        SyncOptions {
            roms,
            name,
            listen: SYNC_PORT,
            announce: SocketAddr::from(([255, 255, 255, 255], SYNC_PORT)),
        }
    }
}

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

/// Whether a path from the other device may be written here.
pub fn allowed(path: &str, roms: bool) -> bool {
    let p = Path::new(path);
    if path.is_empty()
        || p.is_absolute()
        || p.components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return false;
    }
    let shared = SHARED
        .iter()
        .any(|root| path == *root || path.starts_with(&format!("{root}/")));
    shared || (roms && path.starts_with(&format!("{ROMS}/")))
}

/// Every file that travels, with card-relative paths.
pub fn manifest(root: &Path, roms: bool) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut tops: Vec<&str> = SHARED.to_vec();
    if roms {
        tops.push(ROMS);
    }
    for top in tops {
        walk(root, &root.join(top), &mut out);
    }
    out.retain(|e| allowed(&e.path, roms) && !e.path.ends_with(".tmp"));
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn walk(root: &Path, at: &Path, out: &mut Vec<Entry>) {
    let Ok(meta) = std::fs::metadata(at) else {
        return;
    };
    if meta.is_file() {
        let Ok(rel) = at.strip_prefix(root) else {
            return;
        };
        let path = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        out.push(Entry {
            path,
            size: meta.len(),
            mtime: mtime_of(&meta),
        });
        return;
    }
    let Ok(dir) = std::fs::read_dir(at) else {
        return;
    };
    for item in dir.flatten() {
        let name = item.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        walk(root, &item.path(), out);
    }
}

/// What to send: files the other lacks or has older.
pub fn to_send(mine: &[Entry], theirs: &[Entry]) -> Vec<Entry> {
    let theirs: HashMap<&str, &Entry> = theirs.iter().map(|e| (e.path.as_str(), e)).collect();
    mine.iter()
        .filter(|e| match theirs.get(e.path.as_str()) {
            None => true,
            Some(t) => e.mtime > t.mtime + SLACK_S,
        })
        .cloned()
        .collect()
}

fn render(list: &[Entry]) -> Vec<u8> {
    list.iter()
        .map(|e| format!("{}\t{}\t{}\n", e.path, e.size, e.mtime))
        .collect::<String>()
        .into_bytes()
}

fn parse(bytes: &[u8]) -> Vec<Entry> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|l| {
            let mut p = l.rsplitn(3, '\t');
            let mtime = p.next()?.parse().ok()?;
            let size = p.next()?.parse().ok()?;
            let path = p.next()?.to_string();
            Some(Entry { path, size, mtime })
        })
        .collect()
}

fn send_msg(w: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    w.write_all(&(bytes.len() as u32).to_le_bytes())?;
    w.write_all(bytes)
}

fn read_msg(r: &mut impl Read, max: usize) -> std::io::Result<Vec<u8>> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > max {
        return Err(std::io::Error::other("message too long"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Sends each file as a header (path, size, time) and its bytes, then an
/// empty header to end.
fn send_files(root: &Path, w: &mut impl Write, files: &[Entry]) -> std::io::Result<usize> {
    let mut sent = 0;
    for e in files {
        let Ok(mut f) = std::fs::File::open(root.join(&e.path)) else {
            continue;
        };
        let size = f.metadata()?.len();
        send_msg(w, format!("{}\t{size}\t{}", e.path, e.mtime).as_bytes())?;
        let copied = std::io::copy(&mut (&mut f).take(size), w)?;
        if copied != size {
            return Err(std::io::Error::other(format!("{} shrank", e.path)));
        }
        sent += 1;
    }
    send_msg(w, b"")?;
    w.flush()?;
    Ok(sent)
}

fn receive_files(
    root: &Path,
    r: &mut impl Read,
    roms: bool,
    mut each: impl FnMut(usize),
) -> std::io::Result<usize> {
    let mut got = 0;
    loop {
        let head = read_msg(r, 64 * 1024)?;
        if head.is_empty() {
            return Ok(got);
        }
        let Some(e) = parse(&head).pop() else {
            return Err(std::io::Error::other("bad file header"));
        };
        if e.size > FILE_MAX {
            return Err(std::io::Error::other(format!("{} is too big", e.path)));
        }
        let ok = allowed(&e.path, roms);
        let dest = root.join(&e.path);
        let tmp = PathBuf::from(format!("{}.sync.tmp", dest.display()));
        let mut body = r.take(e.size);
        if ok {
            if let Some(dir) = dest.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut out = std::fs::File::create(&tmp)?;
            let copied = std::io::copy(&mut body, &mut out)?;
            if copied != e.size {
                let _ = std::fs::remove_file(&tmp);
                return Err(std::io::Error::other("the other device stopped"));
            }
            let when = UNIX_EPOCH + Duration::from_secs(e.mtime);
            out.set_modified(when)?;
            drop(out);
            std::fs::rename(&tmp, &dest)?;
            got += 1;
            each(got);
        } else {
            eprintln!("slot: sync: refused {}", e.path);
            std::io::copy(&mut body, &mut std::io::sink())?;
        }
    }
}

/// Swaps files with the other device over a connected stream.
pub fn exchange(
    root: &Path,
    stream: TcpStream,
    roms: bool,
    news: &Sender<SyncNews>,
) -> std::io::Result<(usize, usize)> {
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    let mut reader = stream.try_clone()?;
    let mut writer = std::io::BufWriter::new(stream);
    let mine = manifest(root, roms);
    send_msg(&mut writer, &render(&mine))?;
    writer.flush()?;
    let theirs = parse(&read_msg(&mut reader, 64 << 20)?);
    let outgoing = to_send(&mine, &theirs);
    eprintln!(
        "slot: sync: {} files here, {} there, sending {}",
        mine.len(),
        theirs.len(),
        outgoing.len()
    );
    let root_out = root.to_path_buf();
    let sender = std::thread::Builder::new()
        .name("slot-sync-send".into())
        .spawn(move || send_files(&root_out, &mut writer, &outgoing))?;
    let got = receive_files(root, &mut reader, roms, |got| {
        let _ = news.send(SyncNews::Moving { sent: 0, got });
    });
    let sent = sender
        .join()
        .map_err(|_| std::io::Error::other("sending stopped"))??;
    let got = got?;
    Ok((sent, got))
}

fn hello(id: u64, port: u16, name: &str) -> String {
    format!("{HELLO} {id} {port} {}", name.replace('\n', " "))
}

fn parse_hello(text: &str) -> Option<(u64, u16, String)> {
    let mut p = text.splitn(4, ' ');
    (p.next()? == HELLO).then_some(())?;
    let id = p.next()?.parse().ok()?;
    let port = p.next()?.parse().ok()?;
    Some((id, port, p.next().unwrap_or("").to_string()))
}

fn new_id() -> u64 {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    t ^ (u64::from(std::process::id()) << 32) | 1
}

/// Looks for the other device and syncs with it, telling `news` how it
/// goes, until done, failed, or `stop` is set.
pub fn run(root: &Path, opts: &SyncOptions, news: &Sender<SyncNews>, stop: &AtomicBool) {
    let said = |n: SyncNews| {
        let _ = news.send(n);
    };
    said(SyncNews::Looking);
    match find_and_sync(root, opts, news, stop) {
        Ok(Some((sent, got))) => said(SyncNews::Done { sent, got }),
        Ok(None) => {}
        Err(e) => {
            eprintln!("slot: sync: {e}");
            said(SyncNews::Failed(e.to_string()))
        }
    }
}

fn find_and_sync(
    root: &Path,
    opts: &SyncOptions,
    news: &Sender<SyncNews>,
    stop: &AtomicBool,
) -> std::io::Result<Option<(usize, usize)>> {
    let id = new_id();
    let listener = TcpListener::bind(("0.0.0.0", 0))?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let udp = UdpSocket::bind(("0.0.0.0", opts.listen))?;
    udp.set_broadcast(true)?;
    udp.set_read_timeout(Some(Duration::from_millis(200)))?;
    let started = Instant::now();
    let mut last_hello = None::<Instant>;
    let mut buf = [0u8; 512];
    while !stop.load(Ordering::Relaxed) {
        if started.elapsed() > LOOK_FOR {
            return Err(std::io::Error::other(
                "No other device found. Start Device Sync on both",
            ));
        }
        if last_hello.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
            let _ = udp.send_to(hello(id, port, &opts.name).as_bytes(), opts.announce);
            last_hello = Some(Instant::now());
        }
        if let Ok((stream, from)) = listener.accept() {
            stream.set_nonblocking(false)?;
            let _ = news.send(SyncNews::Found(from.ip().to_string()));
            return exchange(root, stream, opts.roms, news).map(Some);
        }
        let Ok((n, from)) = udp.recv_from(&mut buf) else {
            continue;
        };
        let Some((other, their_port, name)) = parse_hello(&String::from_utf8_lossy(&buf[..n]))
        else {
            continue;
        };
        if other <= id {
            // Our own hello, or the other connects to us.
            continue;
        }
        let addr = SocketAddr::new(from.ip(), their_port);
        let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
        let shown = if name.is_empty() {
            from.ip().to_string()
        } else {
            name
        };
        let _ = news.send(SyncNews::Found(shown));
        return exchange(root, stream, opts.roms, news).map(Some);
    }
    Ok(None)
}

/// Runs a sync on a thread of its own.
pub struct SyncJob {
    pub news: std::sync::mpsc::Receiver<SyncNews>,
    stop: Arc<AtomicBool>,
}

impl SyncJob {
    pub fn start(root: &Path, opts: SyncOptions) -> SyncJob {
        let (tell, news) = std::sync::mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let root = root.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("slot-sync".into())
            .spawn(move || run(&root, &opts, &tell, &flag));
        if let Err(e) = spawned {
            eprintln!("slot: sync: {e}");
        }
        SyncJob { news, stop }
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for SyncJob {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_shared_card_paths_may_come_in() {
        assert!(allowed("Saves/GBA/Emerald.srm", false));
        assert!(allowed("Config/emulator/GBA.ini", false));
        assert!(allowed("Config/favorites.txt", false));
        assert!(!allowed("Config/slot.state", false));
        assert!(!allowed("Games/GBA/Emerald.gba", false));
        assert!(allowed("Games/GBA/Emerald.gba", true));
        assert!(!allowed("Saves/../System/slot", false));
        assert!(!allowed("/etc/passwd", true));
        assert!(!allowed("", true));
    }

    #[test]
    fn the_newer_copy_goes_and_a_near_tie_stays() {
        let e = |p: &str, t: u64| Entry {
            path: p.into(),
            size: 1,
            mtime: t,
        };
        let mine = [e("a", 100), e("b", 100), e("c", 100), e("d", 101)];
        let theirs = [e("a", 50), e("b", 200), e("d", 100)];
        let paths: Vec<String> = to_send(&mine, &theirs)
            .into_iter()
            .map(|e| e.path)
            .collect();
        assert_eq!(paths, ["a", "c"]);
    }

    #[test]
    fn a_hello_reads_back() {
        let h = hello(42, 9000, "Brick Pro");
        assert_eq!(parse_hello(&h), Some((42, 9000, "Brick Pro".into())));
        assert_eq!(parse_hello("HELLO 1 2"), None);
    }

    #[test]
    fn a_path_with_tabs_survives_the_list() {
        let list = vec![Entry {
            path: "Saves/GBA/Odd\tName.srm".into(),
            size: 5,
            mtime: 9,
        }];
        assert_eq!(parse(&render(&list)), list);
    }
}
