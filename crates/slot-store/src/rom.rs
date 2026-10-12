//! Bytes the core should see: a loose ROM or the Game Boy file inside a
//! ZIP, with a sibling IPS or BPS applied.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::platform::Platform;
use crate::{patch, zip};

/// The ROM bytes, unzipped and patched.
pub fn open(path: &Path) -> io::Result<Vec<u8>> {
    open_inner(path, None).map(|(_, rom)| rom)
}

/// Like [`open`], but the zip must hold a ROM for `platform`.
pub fn open_for(path: &Path, platform: Platform) -> io::Result<Vec<u8>> {
    open_inner(path, Some(platform)).map(|(_, rom)| rom)
}

/// The file name of the ROM itself, and its bytes unzipped and patched.
fn open_inner(path: &Path, platform: Option<Platform>) -> io::Result<(String, Vec<u8>)> {
    let (name, mut rom) = if zip::is_zip(path) {
        let entry = rom_entry(path, platform).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "no Game Boy ROM in the zip")
        })?;
        let rom = zip::extract_path(path, &entry).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "the zip could not be read")
        })?;
        let name = Path::new(&entry.name)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (name, rom)
    } else {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        (name, std::fs::read(path)?)
    };
    apply_sibling(path, &mut rom);
    Ok((name, rom))
}

/// Whether the core would see different bytes from the file on the card:
/// the ROM is zipped, or has a patch beside it.
pub fn is_prepared(path: &Path) -> bool {
    zip::is_zip(path) || patch_label(path).is_some()
}

/// The ROM bytes, and a path a core can open to find the same bytes. For a
/// plain ROM that is the ROM itself; for a zipped or patched one, the bytes
/// are written to a scratch file, because some cores (gpSP) only read by
/// path.
pub fn stage(path: &Path) -> io::Result<(PathBuf, Vec<u8>)> {
    if !is_prepared(path) {
        return Ok((path.to_path_buf(), std::fs::read(path)?));
    }
    let (name, rom) = open_inner(path, None)?;
    let dir = std::env::temp_dir().join("slot-rom");
    // Only the game being played is kept: last session's goes.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = Path::new(&name)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gba".into());
    let staged = dir.join(format!("{stem}.{ext}"));
    std::fs::write(&staged, &rom)?;
    Ok((staged, rom))
}

/// The cartridge header of `path`, reading only its first `n` bytes where
/// it can: a loose ROM's start, or the inflated start of a zipped one. A
/// patched ROM is read whole, since a patch may change its header.
pub fn head(path: &Path, platform: Platform, n: usize) -> Option<Vec<u8>> {
    if patch_label(path).is_some() {
        return open_for(path, platform).ok();
    }
    if zip::is_zip(path) {
        let entry = rom_entry(path, Some(platform))?;
        return zip::head_path(path, &entry, n);
    }
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = Vec::with_capacity(n);
    f.by_ref().take(n as u64).read_to_end(&mut buf).ok()?;
    Some(buf)
}

/// The first ROM in the zip, for `platform` or any we play.
fn rom_entry(path: &Path, platform: Option<Platform>) -> Option<zip::Entry> {
    zip::catalog_path(path).into_iter().find(|e| {
        let p = Path::new(&e.name);
        match platform {
            Some(platform) => platform.accepts(p),
            None => Platform::ALL.iter().any(|plat| plat.accepts(p)),
        }
    })
}

/// The file name of the ROM inside a zip, if it holds one we play.
pub fn zip_rom_name(path: &Path) -> Option<String> {
    rom_entry(path, None).map(|e| e.name)
}

pub fn zip_has(path: &Path, platform: Platform) -> bool {
    rom_entry(path, Some(platform)).is_some()
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
