//! Bytes the core should see: a loose ROM or the Game Boy file inside a
//! ZIP, with a sibling IPS or BPS applied.

use std::io;
use std::path::Path;

use crate::platform::Platform;
use crate::{patch, zip};

/// The ROM bytes, unzipped and patched.
pub fn open(path: &Path) -> io::Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    let mut rom = if zip::is_zip(path) {
        unzip(&data, None).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "no Game Boy ROM in the zip")
        })?
    } else {
        data
    };
    apply_sibling(path, &mut rom);
    Ok(rom)
}

/// Like [`open`], but the zip must hold a ROM for `platform`.
pub fn open_for(path: &Path, platform: Platform) -> io::Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    let mut rom = if zip::is_zip(path) {
        unzip(&data, Some(platform)).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "no matching ROM in the zip")
        })?
    } else {
        data
    };
    apply_sibling(path, &mut rom);
    Ok(rom)
}

pub fn zip_has(path: &Path, platform: Platform) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    zip::catalog(&data)
        .iter()
        .any(|e| platform.accepts(Path::new(&e.name)))
}

/// What patch sits beside the ROM, if any.
pub fn patch_label(path: &Path) -> Option<&'static str> {
    let stem = path.file_stem()?.to_str()?;
    let dir = path.parent()?;
    if dir.join(format!("{stem}.bps")).is_file() {
        Some("BPS")
    } else if dir.join(format!("{stem}.ips")).is_file() {
        Some("IPS")
    } else {
        None
    }
}

fn unzip(data: &[u8], platform: Option<Platform>) -> Option<Vec<u8>> {
    let entries = zip::catalog(data);
    let entry = entries.iter().find(|e| {
        let p = Path::new(&e.name);
        match platform {
            Some(platform) => platform.accepts(p),
            None => Platform::ALL.iter().any(|plat| plat.accepts(p)),
        }
    })?;
    zip::extract(data, entry)
}

fn apply_sibling(path: &Path, rom: &mut Vec<u8>) {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    let ips = dir.join(format!("{stem}.ips"));
    if let Ok(bytes) = std::fs::read(&ips) {
        if let Err(e) = patch::apply_ips(rom, &bytes) {
            eprintln!("slot: {}: {e}", ips.display());
        }
    }
    let bps = dir.join(format!("{stem}.bps"));
    if let Ok(bytes) = std::fs::read(&bps) {
        match patch::apply_bps(rom, &bytes) {
            Ok(out) => *rom = out,
            Err(e) => eprintln!("slot: {}: {e}", bps.display()),
        }
    }
}
