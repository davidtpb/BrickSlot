use std::path::Path;

use slot_store::{atomic_write, read_slot_state, write_slot_state, Core, Platform, StateRing};

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
    if let Some(state) = state {
        StateRing::new(root, platform, core, stem).write_resume(state)?;
    }
    if let Some(sav) = sav {
        write_sav(root, platform, stem, sav)?;
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
    flush(root, platform, core, stem, state, sav)?;
    let mut slot = read_slot_state(root);
    slot.cart = None;
    slot.cart_platform = None;
    write_slot_state(root, &slot)
}

/// Battery saves are written the way RetroArch writes them: the cartridge RAM
/// as `<game>.srm` and, for carts with a clock, the clock as `<game>.rtc`.
/// The core hands both over as one blob, RAM first.
pub fn write_sav(root: &Path, platform: Platform, stem: &str, sav: &[u8]) -> std::io::Result<bool> {
    if read_sav(root, platform, stem).as_deref() == Some(sav) {
        return Ok(false);
    }
    let dir = crate::root::saves_dir(root).join(platform.dir_name());
    std::fs::create_dir_all(&dir)?;
    let (ram, rtc) = split_rtc(sav);
    atomic_write(&dir.join(format!("{stem}.{SRM}")), ram)?;
    let rtc_path = dir.join(format!("{stem}.{RTC}"));
    if rtc.is_empty() {
        if rtc_path.exists() {
            std::fs::remove_file(&rtc_path)?;
        }
    } else {
        atomic_write(&rtc_path, rtc)?;
    }
    let old = dir.join(format!("{stem}.{SAV}"));
    if old.is_file() {
        std::fs::rename(&old, dir.join(format!("{stem}.{SAV}.bak")))?;
    }
    Ok(true)
}

/// The `.srm` (with its `.rtc` after it, if there is one), or else a `.sav`
/// from before saves were written RetroArch's way.
pub fn read_sav(root: &Path, platform: Platform, stem: &str) -> Option<Vec<u8>> {
    let dir = crate::root::saves_dir(root).join(platform.dir_name());
    if let Ok(mut ram) = std::fs::read(dir.join(format!("{stem}.{SRM}"))) {
        if let Ok(rtc) = std::fs::read(dir.join(format!("{stem}.{RTC}"))) {
            ram.extend_from_slice(&rtc);
        }
        return Some(ram);
    }
    std::fs::read(dir.join(format!("{stem}.{SAV}"))).ok()
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
