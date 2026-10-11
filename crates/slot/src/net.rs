use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NetJob {
    WifiUp,
    WifiDown,
    SyncthingStart,
    SyncthingStop,
    SyncClock,
    Labels(Vec<crate::labels::Wanted>),
    /// Says where a page served on this port can be reached.
    Announce {
        title: String,
        port: u16,
    },
    /// Looks for networks in range.
    Scan,
    /// Joins a network, remembering it for next time. An empty password
    /// joins an open network.
    Join {
        ssid: String,
        psk: String,
    },
}

/// A network in range.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Network {
    pub ssid: String,
    /// Signal strength in dBm: nearer zero is stronger.
    pub signal: i32,
    pub secured: bool,
}

impl Network {
    /// Signal as bars from 1 to 4.
    pub fn bars(&self) -> u8 {
        match self.signal {
            s if s >= -55 => 4,
            s if s >= -67 => 3,
            s if s >= -78 => 2,
            _ => 1,
        }
    }
}

/// The networks `net.sh scan` printed, strongest first.
pub fn parse_scan(text: &str) -> Vec<Network> {
    let mut out: Vec<Network> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let signal = parts.next()?.trim().parse().ok()?;
            let flags = parts.next()?;
            let ssid = parts.next()?.trim_end_matches('\r');
            (!ssid.is_empty()).then(|| Network {
                ssid: ssid.to_string(),
                signal,
                secured: ["WPA", "WEP", "RSN", "SAE"]
                    .iter()
                    .any(|k| flags.contains(k)),
            })
        })
        .collect();
    out.sort_by_key(|n| std::cmp::Reverse(n.signal));
    let mut seen = std::collections::HashSet::new();
    out.retain(|n| seen.insert(n.ssid.clone()));
    out
}

pub const SYNCTHING_PORT: u16 = 8384;

pub struct Net {
    jobs: Option<Sender<NetJob>>,
    said: Option<Receiver<(String, String)>>,
    sent: Vec<NetJob>,
    online: Arc<AtomicBool>,
    clock: Option<Receiver<i64>>,
    networks: Option<Receiver<Vec<Network>>>,
}

impl Net {
    pub fn idle() -> Self {
        Net {
            jobs: None,
            said: None,
            sent: Vec::new(),
            online: Arc::new(AtomicBool::new(false)),
            clock: None,
            networks: None,
        }
    }

    pub fn spawn(root: &Path) -> Self {
        if !cfg!(feature = "device") {
            return Net::idle();
        }
        let (jobs, rx) = std::sync::mpsc::channel();
        let (tell, said) = std::sync::mpsc::channel();
        let (set, clock) = std::sync::mpsc::channel();
        let (found, networks) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        let online = Arc::new(AtomicBool::new(false));
        let up = online.clone();
        let spawned = std::thread::Builder::new()
            .name("slot-net".into())
            .spawn(move || work(&root, &rx, &tell, &up, &set, &found));
        if let Err(e) = spawned {
            eprintln!("slot: net: {e}");
            return Net::idle();
        }
        Net {
            jobs: Some(jobs),
            said: Some(said),
            sent: Vec::new(),
            online,
            clock: Some(clock),
            networks: Some(networks),
        }
    }

    pub fn send(&mut self, job: NetJob) {
        self.sent.push(job.clone());
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    pub fn sent(&self) -> &[NetJob] {
        &self.sent
    }

    pub fn online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }

    /// Seconds since 1970 (UTC) from a time server, once one has answered.
    pub fn clock(&self) -> Option<i64> {
        self.clock.as_ref()?.try_recv().ok()
    }

    /// The networks a scan found, once it has finished.
    pub fn networks(&self) -> Option<Vec<Network>> {
        self.networks.as_ref()?.try_recv().ok()
    }

    pub fn heard(&self) -> Option<(String, String)> {
        self.said.as_ref()?.try_recv().ok()
    }
}

fn script(root: &Path, name: &str) -> PathBuf {
    root.join("System").join(name)
}

fn run(root: &Path, name: &str, verb: &str) -> bool {
    let status = std::process::Command::new("/bin/sh")
        .arg(script(root, name))
        .arg(verb)
        .env("SLOT_ROOT", root)
        .status();
    match status {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("slot: net: {name} {verb}: {s}");
            false
        }
        Err(e) => {
            eprintln!("slot: net: {name} {verb}: {e}");
            false
        }
    }
}

fn address(root: &Path) -> Option<String> {
    let out = std::process::Command::new("/bin/sh")
        .arg(script(root, "net.sh"))
        .arg("ip")
        .env("SLOT_ROOT", root)
        .output()
        .ok()?;
    let ip = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !ip.is_empty()).then_some(ip)
}

const ADDRESS_WAIT_S: u32 = 30;

const TIME_SERVERS: [&str; 3] = ["pool.ntp.org", "time.google.com", "time.cloudflare.com"];

/// Seconds between the NTP epoch (1900) and the Unix one (1970).
const NTP_TO_UNIX: i64 = 2_208_988_800;

fn network_time() -> Option<i64> {
    TIME_SERVERS.iter().find_map(|server| match sntp(server) {
        Ok(secs) => Some(secs),
        Err(e) => {
            eprintln!("slot: net: {server}: {e}");
            None
        }
    })
}

/// One SNTP (RFC 4330) query: a 48-byte client packet out, the server's
/// transmit time back.
fn sntp(server: &str) -> std::io::Result<i64> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    socket.connect((server, 123))?;
    let mut request = [0u8; 48];
    request[0] = 0x23;
    socket.send(&request)?;
    let mut reply = [0u8; 48];
    let n = socket.recv(&mut reply)?;
    sntp_reply(&reply[..n]).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "not a usable time reply")
    })
}

/// The server's transmit time from an SNTP reply, as Unix seconds. Refuses
/// a reply that is short, not from a server, unsynchronised or a kiss-o'-death.
pub fn sntp_reply(reply: &[u8]) -> Option<i64> {
    let packet: &[u8; 48] = reply.get(..48)?.try_into().ok()?;
    let (leap, mode, stratum) = (packet[0] >> 6, packet[0] & 0x7, packet[1]);
    if leap == 3 || !(4..=5).contains(&mode) || stratum == 0 {
        return None;
    }
    let secs = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]);
    let fraction = u32::from_be_bytes([packet[44], packet[45], packet[46], packet[47]]);
    let round = i64::from(fraction >= 0x8000_0000);
    // NTP seconds wrap in 2036; a value with the top bit clear is past it.
    let era = if secs & 0x8000_0000 == 0 {
        1i64 << 32
    } else {
        0
    };
    let unix = i64::from(secs) + era - NTP_TO_UNIX + round;
    (unix > 0).then_some(unix)
}

fn wait_address(root: &Path) -> Option<String> {
    for _ in 0..ADDRESS_WAIT_S {
        if let Some(ip) = address(root) {
            return Some(ip);
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    None
}

fn work(
    root: &Path,
    jobs: &Receiver<NetJob>,
    tell: &Sender<(String, String)>,
    online: &AtomicBool,
    clock: &Sender<i64>,
    found: &Sender<Vec<Network>>,
) {
    let say = |title: &str, detail: String| {
        let _ = tell.send((title.to_string(), detail));
    };
    while let Ok(job) = jobs.recv() {
        match job {
            NetJob::WifiUp => {
                run(root, "net.sh", "up");
                match wait_address(root) {
                    Some(ip) => {
                        online.store(true, Ordering::Relaxed);
                        say("Wi-Fi connected", ip);
                    }
                    None => say(
                        "Wi-Fi did not connect",
                        "Join a network under Network".into(),
                    ),
                }
            }
            NetJob::WifiDown => {
                online.store(false, Ordering::Relaxed);
                run(root, "syncthing.sh", "stop");
                run(root, "net.sh", "down");
            }
            NetJob::SyncthingStart => {
                if !run(root, "syncthing.sh", "start") {
                    say("Syncthing is missing", "Copy the full card again".into());
                    continue;
                }
                match wait_address(root) {
                    Some(ip) => {
                        online.store(true, Ordering::Relaxed);
                        say("Syncthing on", format!("http://{ip}:{SYNCTHING_PORT}"));
                    }
                    None => say("Syncthing on, no Wi-Fi", "Check Config/wifi.txt".into()),
                }
            }
            NetJob::SyncthingStop => {
                run(root, "syncthing.sh", "stop");
            }
            NetJob::Announce { title, port } => match address(root) {
                Some(ip) => say(&title, format!("http://{ip}:{port}")),
                None => say(&title, format!("Port {port}, but Wi-Fi has no address")),
            },
            NetJob::Labels(wanted) => {
                let tally = crate::labels::fetch_all(&wanted, crate::labels::http_get);
                say(
                    "Labels downloaded",
                    format!(
                        "{} found, {} not found. Restart to see them",
                        tally.found, tally.missing
                    ),
                );
            }
            NetJob::Scan => {
                let out = std::process::Command::new("/bin/sh")
                    .arg(script(root, "net.sh"))
                    .arg("scan")
                    .env("SLOT_ROOT", root)
                    .output();
                let networks = match out {
                    Ok(o) => parse_scan(&String::from_utf8_lossy(&o.stdout)),
                    Err(e) => {
                        eprintln!("slot: net: scan: {e}");
                        Vec::new()
                    }
                };
                eprintln!("slot: net: scan found {} networks", networks.len());
                let _ = found.send(networks);
            }
            NetJob::Join { ssid, psk } => {
                online.store(false, Ordering::Relaxed);
                let joined = std::process::Command::new("/bin/sh")
                    .arg(script(root, "net.sh"))
                    .arg("join")
                    .env("SLOT_ROOT", root)
                    .env("NET_SSID", &ssid)
                    .env("NET_PSK", &psk)
                    .status();
                if let Err(e) = joined {
                    eprintln!("slot: net: join: {e}");
                }
                match wait_address(root) {
                    Some(ip) => {
                        online.store(true, Ordering::Relaxed);
                        say(&format!("Joined {ssid}"), ip);
                    }
                    None => say(
                        &format!("Could not join {ssid}"),
                        "Check the password, or see slot-net.log".into(),
                    ),
                }
            }
            NetJob::SyncClock => match network_time() {
                Some(secs) => {
                    let _ = clock.send(secs);
                }
                None => eprintln!("slot: net: no time server answered"),
            },
        }
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    #[test]
    fn a_scan_reads_strongest_first_without_repeats_or_hidden_networks() {
        let text = "-70\t[WPA2-PSK-CCMP][ESS]\tHome Net\n-48\t[WPA2-PSK-CCMP][ESS]\tHome Net\n-60\t[ESS]\tCafe\n-80\t[WPA2]\t\nnonsense\n";
        let n = parse_scan(text);
        assert_eq!(
            n.iter().map(|n| n.ssid.as_str()).collect::<Vec<_>>(),
            ["Home Net", "Cafe"]
        );
        assert_eq!(n[0].signal, -48);
        assert!(n[0].secured);
        assert!(!n[1].secured);
        assert_eq!(n[0].bars(), 4);
        assert_eq!(n[1].bars(), 3);
    }

    #[test]
    fn a_name_with_tabs_or_spaces_is_kept_whole() {
        let n = parse_scan("-50\t[WPA2]\tMy  Home\tNet\n");
        assert_eq!(n[0].ssid, "My  Home\tNet");
    }
}
