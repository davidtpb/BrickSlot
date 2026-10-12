use std::path::Path;

use slot_store::{
    atomic_write, read_slot_state, write_slot_state, Core, Platform, SaveFormat, StateRing,
};

pub trait Snapshot {
    fn state(&self) -> Option<Vec<u8>>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn thumb(&self) -> Option<Vec<u8>>;
    fn load(&self, state: Vec<u8>);

    fn resume_trusted(&self) -> bool {
        true
    }

    fn save_ram_trusted(&self) -> bool {
        true
    }
}

pub fn flush(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    flush_as(
        root,
        platform,
        core,
        stem,
        state,
        sav,
        SaveFormat::default(),
    )
}

pub fn flush_as(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
    format: SaveFormat,
) -> std::io::Result<()> {
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav_as(root, platform, stem, sav, format)?;
    }
    Ok(())
}

pub fn eject(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
) -> std::io::Result<()> {
    eject_as(
        root,
        platform,
        core,
        stem,
        state,
        sav,
        SaveFormat::default(),
    )
}

pub fn eject_as(
    root: &Path,
    platform: Platform,
    core: Core,
    stem: &str,
    state: Option<&[u8]>,
    sav: Option<&[u8]>,
    format: SaveFormat,
) -> std::io::Result<()> {
    flush_as(root, platform, core, stem, state, sav, format)?;
    let mut slot = read_slot_state(root);
    slot.cart = None;
    slot.cart_platform = None;
    write_slot_state(root, &slot)
}

pub fn write_sav(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    write_sav_as(root, platform, stem, sav, SaveFormat::default())
}

/// Writes a battery save in the chosen format: RetroArch's `<game>.srm` with
/// a clock in `<game>.rtc`, or slot's `<game>.sav` with the clock on the end.
/// The core hands both over as one blob, RAM first. Files in the other
/// format are moved aside to `.bak`, so the two can never disagree.
pub fn write_sav_as(
    root: &Path,
    platform: Platform,
    stem: &str,
    sav: &[u8],
    format: SaveFormat,
) -> std::io::Result<bool> {
    let wrote = write_sav_files(root, platform, stem, sav, format)?;
    if let Err(e) = note_save(root, platform, stem) {
        eprintln!("slot: save: could not note {stem}'s save: {e}");
    }
    Ok(wrote)
}

/// Where slot notes how each save looked when it last wrote or checked it,
/// so a save put on the card from elsewhere can be told apart.
fn mark_path(root: &Path, platform: Platform, stem: &str) -> std::path::PathBuf {
    root.join("Config")
        .join("save-marks")
        .join(platform.dir_name())
        .join(format!("{stem}.mark"))
}

/// The size and time of every save file the game has on the card.
fn save_fingerprint(root: &Path, platform: Platform, stem: &str) -> String {
    let dir = crate::root::saves_dir(root).join(platform.dir_name());
    let mut out = String::new();
    for ext in [SRM, RTC, SAV] {
        let Ok(meta) = std::fs::metadata(dir.join(format!("{stem}.{ext}"))) else {
            continue;
        };
        let when = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        out.push_str(&format!("{ext} {} {when}\n", meta.len()));
    }
    out
}

/// Notes the game's save as slot now leaves it.
pub fn note_save(root: &Path, platform: Platform, stem: &str) -> std::io::Result<()> {
    let path = mark_path(root, platform, stem);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, save_fingerprint(root, platform, stem).as_bytes())
}

/// Whether the game's save on the card has changed since slot last wrote
/// it: copied over from another device, or put back from a backup. A game
/// slot has never noted reads as unchanged.
pub fn save_replaced(root: &Path, platform: Platform, stem: &str) -> bool {
    match std::fs::read_to_string(mark_path(root, platform, stem)) {
        Ok(noted) => noted != save_fingerprint(root, platform, stem),
        Err(_) => false,
    }
}

fn write_sav_files(
    root: &Path,
    platform: Platform,
    stem: &str,
    sav: &[u8],
    format: SaveFormat,
) -> std::io::Result<bool> {
    let dir = crate::root::saves_dir(root).join(platform.dir_name());
    let file = |ext: &str| dir.join(format!("{stem}.{ext}"));
    let compress = format == SaveFormat::RetroArchCompressed;
    let current =
        file(main_ext(format)).is_file() && is_compressed(&file(main_ext(format))) == compress;
    if current && read_sav(root, platform, stem).as_deref() == Some(sav) {
        return Ok(false);
    }
    std::fs::create_dir_all(&dir)?;
    let packed = |data: &[u8]| match compress {
        true => slot_store::rzip::compress(data),
        false => data.to_vec(),
    };
    match format {
        SaveFormat::RetroArch | SaveFormat::RetroArchCompressed => {
            let (ram, rtc) = split_rtc(sav);
            atomic_write(&file(SRM), &packed(ram))?;
            if rtc.is_empty() {
                if file(RTC).exists() {
                    std::fs::remove_file(file(RTC))?;
                }
            } else {
                atomic_write(&file(RTC), &packed(rtc))?;
            }
        }
        SaveFormat::Slot => atomic_write(&file(SAV), sav)?,
    }
    let others: &[&str] = match format {
        SaveFormat::RetroArch | SaveFormat::RetroArchCompressed => &[SAV],
        SaveFormat::Slot => &[SRM, RTC],
    };
    for ext in others {
        if file(ext).is_file() {
            std::fs::rename(file(ext), dir.join(format!("{stem}.{ext}.bak")))?;
        }
    }
    Ok(true)
}

fn main_ext(format: SaveFormat) -> &'static str {
    match format {
        SaveFormat::RetroArch | SaveFormat::RetroArchCompressed => SRM,
        SaveFormat::Slot => SAV,
    }
}

/// Whether a save file is RetroArch's compressed RZIP.
fn is_compressed(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 20];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|_| slot_store::rzip::is_rzip(&head))
}

/// Reads a RetroArch save file, inflating it if RetroArch compressed it.
/// One that will not inflate is copied aside as `.broken` first, so the
/// fresh save written over it later does not lose it.
fn read_retroarch(path: &Path) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    match slot_store::rzip::plain(data) {
        Ok(plain) => Some(plain),
        Err(e) => {
            eprintln!("slot: save: {}: {e}", path.display());
            let mut aside = path.as_os_str().to_owned();
            aside.push(".broken");
            if !Path::new(&aside).exists() {
                let _ = std::fs::copy(path, &aside);
            }
            None
        }
    }
}

/// Whichever save is on the card: an `.srm` (with its `.rtc` after it) or a
/// `.sav`. If both are there, say one copied over from another device, the
/// newer one wins.
pub fn read_sav(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    let dir = crate::root::saves_dir(root).join(platform.dir_name());
    let file = |ext: &str| dir.join(format!("{stem}.{ext}"));
    let modified = |ext: &str| std::fs::metadata(file(ext)).and_then(|m| m.modified()).ok();
    let srm = || {
        let mut ram = read_retroarch(&file(SRM))?;
        if let Some(rtc) = read_retroarch(&file(RTC)) {
            ram.extend_from_slice(&rtc);
        }
        Some(ram)
    };
    let sav = || std::fs::read(file(SAV)).ok();
    match (modified(SRM), modified(SAV)) {
        (Some(a), Some(b)) if b > a => sav().or_else(srm),
        _ => srm().or_else(sav),
    }
}

const SRM: &str = "srm";
const RTC: &str = "rtc";
const SAV: &str = "sav";

/// The longest clock any core appends to its save RAM (mGBA's is 48 bytes).
const RTC_MAX: usize = 64;

/// The smallest cartridge RAM a core reports (MBC7's EEPROM).
const RAM_MIN: usize = 256;

/// Cartridge RAM comes in powers of two, so anything past the largest power
/// of two that fits is the clock.
pub fn split_rtc(sav: &[u8]) -> (&[u8], &[u8]) {
    let n = sav.len();
    if n == 0 || n.is_power_of_two() {
        return (sav, &[]);
    }
    let ram = 1usize << (usize::BITS - 1 - n.leading_zeros());
    match n - ram {
        tail if tail <= RTC_MAX && ram >= RAM_MIN => sav.split_at(ram),
        _ => (sav, &[]),
    }
}

pub fn read_resume(root: &Path, platform: Platform, core: Core, stem: &str) -> Option<Vec<u8>> {
    StateRing::new(root, platform, core, stem)
        .read_resume()
        .ok()
        .flatten()
}
