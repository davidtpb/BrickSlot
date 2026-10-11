use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum NetJob {
    WifiUp,
    WifiDown,
    SyncthingStart,
    SyncthingStop,
}

pub const SYNCTHING_PORT: u16 = 8384;

pub struct Net {
    jobs: Option<Sender<NetJob>>,
    said: Option<Receiver<(String, String)>>,
    sent: Vec<NetJob>,
    online: Arc<AtomicBool>,
}

impl Net {
    pub fn idle() -> Self {
        Net {
            jobs: None,
            said: None,
            sent: Vec::new(),
            online: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn spawn(root: &Path) -> Self {
        if !cfg!(feature = "device") {
            return Net::idle();
        }
        let (jobs, rx) = std::sync::mpsc::channel();
        let (tell, said) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        let online = Arc::new(AtomicBool::new(false));
        let up = online.clone();
        let spawned = std::thread::Builder::new()
            .name("slot-net".into())
            .spawn(move || work(&root, &rx, &tell, &up));
        if let Err(e) = spawned {
            eprintln!("slot: net: {e}");
            return Net::idle();
        }
        Net {
            jobs: Some(jobs),
            said: Some(said),
            sent: Vec::new(),
            online,
        }
    }

    pub fn send(&mut self, job: NetJob) {
        self.sent.push(job);
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
                    None => say("Wi-Fi did not connect", "Check Config/wifi.txt".into()),
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
        }
    }
}
