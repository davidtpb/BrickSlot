//! Bluetooth, through System/bt.sh on a thread of its own: a scan holds the
//! radio for ten seconds and pairing for longer, which Wi-Fi need not wait on.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BtJob {
    Up,
    Down,
    /// Looks for devices, then lists every device known.
    Scan,
    /// Lists the devices known without looking for new ones.
    List,
    /// Pairs with, trusts and connects a device.
    Connect {
        mac: String,
        name: String,
    },
    Disconnect {
        mac: String,
    },
    Forget {
        mac: String,
    },
    Status,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BtDevice {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
    pub audio: bool,
}

/// Reads bt.sh's `address<TAB>flags<TAB>name` lines: connected devices
/// first, then paired ones, then the rest by name.
pub fn parse_devices(text: &str) -> Vec<BtDevice> {
    let mut out: Vec<BtDevice> = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(mac), Some(flags), Some(name)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let (mac, name) = (mac.trim(), name.trim());
        if mac.len() != 17 || name.is_empty() || out.iter().any(|d| d.mac == mac) {
            continue;
        }
        out.push(BtDevice {
            mac: mac.to_string(),
            name: name.to_string(),
            paired: flags.contains('p'),
            connected: flags.contains('c'),
            audio: flags.contains('a'),
        });
    }
    out.sort_by(|a, b| {
        (!a.connected, !a.paired, a.name.to_lowercase()).cmp(&(
            !b.connected,
            !b.paired,
            b.name.to_lowercase(),
        ))
    });
    out
}

/// The headphones audio should go to, from a device list: the first
/// connected device that plays audio.
pub fn audio_device(list: &[BtDevice]) -> Option<&BtDevice> {
    list.iter().find(|d| d.connected && d.audio)
}

pub struct Bt {
    jobs: Option<Sender<BtJob>>,
    said: Option<Receiver<(String, String)>>,
    found: Option<Receiver<Vec<BtDevice>>>,
    sent: Vec<BtJob>,
}

impl Bt {
    pub fn idle() -> Self {
        Bt {
            jobs: None,
            said: None,
            found: None,
            sent: Vec::new(),
        }
    }

    pub fn spawn(root: &Path) -> Self {
        if !cfg!(feature = "device") {
            return Bt::idle();
        }
        let (jobs, rx) = std::sync::mpsc::channel();
        let (tell, said) = std::sync::mpsc::channel();
        let (list, found) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("slot-bt".into())
            .spawn(move || work(&root, &rx, &tell, &list));
        if let Err(e) = spawned {
            eprintln!("slot: bt: {e}");
            return Bt::idle();
        }
        Bt {
            jobs: Some(jobs),
            said: Some(said),
            found: Some(found),
            sent: Vec::new(),
        }
    }

    pub fn send(&mut self, job: BtJob) {
        self.sent.push(job.clone());
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(job);
        }
    }

    pub fn sent(&self) -> &[BtJob] {
        &self.sent
    }

    /// The devices a scan or list found, once it has finished.
    pub fn devices(&self) -> Option<Vec<BtDevice>> {
        self.found.as_ref()?.try_recv().ok()
    }

    pub fn heard(&self) -> Option<(String, String)> {
        self.said.as_ref()?.try_recv().ok()
    }
}

fn script(root: &Path) -> PathBuf {
    root.join("System").join("bt.sh")
}

fn bt_sh(root: &Path, verb: &str, mac: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("/bin/sh");
    cmd.arg(script(root)).arg(verb).env("SLOT_ROOT", root);
    if let Some(mac) = mac {
        cmd.env("BT_MAC", mac);
    }
    match cmd.output() {
        Ok(o) => o
            .status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).into_owned()),
        Err(e) => {
            eprintln!("slot: bt: {verb}: {e}");
            None
        }
    }
}

/// Lists the devices and sends audio to connected headphones, or back to
/// the speaker when there are none.
fn refresh(root: &Path, list: &Sender<Vec<BtDevice>>, scan: bool) -> Vec<BtDevice> {
    let verb = if scan { "scan" } else { "list" };
    let devices = parse_devices(&bt_sh(root, verb, None).unwrap_or_default());
    crate::audio::route_bluetooth(audio_device(&devices).map(|d| d.mac.as_str()));
    let _ = list.send(devices.clone());
    devices
}

fn work(
    root: &Path,
    jobs: &Receiver<BtJob>,
    tell: &Sender<(String, String)>,
    list: &Sender<Vec<BtDevice>>,
) {
    let say = |title: &str, detail: String| {
        let _ = tell.send((title.to_string(), detail));
    };
    while let Ok(job) = jobs.recv() {
        match job {
            BtJob::Up => {
                bt_sh(root, "up", None);
                let devices = refresh(root, list, false);
                if let Some(d) = audio_device(&devices) {
                    say("Bluetooth audio", d.name.clone());
                }
            }
            BtJob::Down => {
                crate::audio::route_bluetooth(None);
                bt_sh(root, "down", None);
            }
            BtJob::Scan => {
                refresh(root, list, true);
            }
            BtJob::List => {
                refresh(root, list, false);
            }
            BtJob::Connect { mac, name } => {
                let joined = bt_sh(root, "connect", Some(&mac)).is_some();
                let devices = refresh(root, list, false);
                match joined {
                    true if audio_device(&devices).is_some_and(|d| d.mac == mac) => say(
                        &format!("Connected {name}"),
                        "Sound plays through it".into(),
                    ),
                    true => say(&format!("Connected {name}"), String::new()),
                    false => say(
                        &format!("Could not connect {name}"),
                        "Put it in pairing mode, or see slot-bt.log".into(),
                    ),
                }
            }
            BtJob::Disconnect { mac } => {
                bt_sh(root, "disconnect", Some(&mac));
                refresh(root, list, false);
            }
            BtJob::Forget { mac } => {
                bt_sh(root, "forget", Some(&mac));
                refresh(root, list, false);
            }
            BtJob::Status => {
                let said = bt_sh(root, "status", None)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Could not tell, see slot-bt.log".into());
                say("Bluetooth status", said);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_read_connected_first_and_drop_broken_lines() {
        let text = "AA:BB:CC:DD:EE:03\t-\tpad\n\
                    AA:BB:CC:DD:EE:01\tpca\tBuds Pro\n\
                    garbage\n\
                    AA:BB:CC:DD:EE:02\tp\tAlpha Speaker\n\
                    AA:BB:CC:DD:EE:01\tpca\tBuds Pro\n";
        let list = parse_devices(text);
        let names: Vec<&str> = list.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Buds Pro", "Alpha Speaker", "pad"]);
        assert!(list[0].connected && list[0].audio && list[0].paired);
        assert!(!list[2].paired);
        assert_eq!(
            audio_device(&list).map(|d| d.mac.as_str()),
            Some("AA:BB:CC:DD:EE:01")
        );
    }
}
