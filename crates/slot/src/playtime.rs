use std::path::Path;
use std::sync::Mutex;

use slot_store::{ini, Platform};

pub const PLAYTIME_FILE: &str = "Config/playtime.txt";

static WRITING: Mutex<()> = Mutex::new(());

pub fn key(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name())
}

pub fn read(root: &Path, key: &str) -> u64 {
    ini::value(root, PLAYTIME_FILE, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub fn add(root: &Path, key: &str, secs: u64) -> std::io::Result<u64> {
    let _held = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let total = read(root, key).saturating_add(secs);
    ini::write(root, PLAYTIME_FILE, key, &total.to_string())?;
    Ok(total)
}

pub fn format(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs / 60 % 60);
    match (h, m) {
        (0, 0) => "under a minute".to_string(),
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h {m:02}m"),
    }
}
