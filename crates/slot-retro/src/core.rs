use std::fmt;
use std::path::Path;

use crate::link::Link;
use crate::rumble::Rumble;

pub const GBA_W: u32 = 240;
pub const GBA_H: u32 = 160;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct ButtonMask(pub u16);

impl ButtonMask {
    pub const B: u16 = 1 << 0;
    pub const Y: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const UP: u16 = 1 << 4;
    pub const DOWN: u16 = 1 << 5;
    pub const LEFT: u16 = 1 << 6;
    pub const RIGHT: u16 = 1 << 7;
    pub const A: u16 = 1 << 8;
    pub const X: u16 = 1 << 9;
    pub const L: u16 = 1 << 10;
    pub const R: u16 = 1 << 11;

    /// A and B the other way round.
    pub fn swap_ab(self) -> ButtonMask {
        let a = self.0 & Self::A != 0;
        let b = self.0 & Self::B != 0;
        let mut m = self.0 & !(Self::A | Self::B);
        if a {
            m |= Self::B;
        }
        if b {
            m |= Self::A;
        }
        ButtonMask(m)
    }

    pub fn without_turbo(self) -> ButtonMask {
        ButtonMask(self.0 & !(Self::X | Self::Y))
    }

    pub fn turbo(self, frame: u32) -> ButtonMask {
        let mut mask = self.0 & !(Self::X | Self::Y);
        if (frame / 3).is_multiple_of(2) {
            if self.0 & Self::X != 0 {
                mask |= Self::A;
            }
            if self.0 & Self::Y != 0 {
                mask |= Self::B;
            }
        }
        ButtonMask(mask)
    }
}

/// A block of the emulated console's memory, by its address on the
/// console's own bus. `ptr` points into the core and is only good while the
/// game it came from is loaded.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct MemoryRegion {
    pub start: u32,
    pub len: usize,
    pub ptr: usize,
}

impl MemoryRegion {
    /// Copies from `address` on, as far as this block goes. Returns how many
    /// bytes were copied, 0 if the address is not in this block.
    ///
    /// # Safety
    /// `ptr` must still point at `len` live bytes.
    pub unsafe fn read(&self, address: u32, buf: &mut [u8]) -> usize {
        let Some(at) = address.checked_sub(self.start).map(|a| a as usize) else {
            return 0;
        };
        if at >= self.len || self.ptr == 0 {
            return 0;
        }
        let n = buf.len().min(self.len - at);
        std::ptr::copy_nonoverlapping((self.ptr as *const u8).add(at), buf.as_mut_ptr(), n);
        n
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AvInfo {
    pub fps: f64,
    pub sample_rate: f64,
}

#[derive(Debug)]
pub enum CoreError {
    Io(std::io::Error),
    Load(String),
    Unsupported(String),
    State(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::Io(e) => write!(f, "io: {e}"),
            CoreError::Load(m) => write!(f, "load: {m}"),
            CoreError::Unsupported(m) => write!(f, "unsupported: {m}"),
            CoreError::State(m) => write!(f, "state: {m}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e)
    }
}

pub trait RetroCore: Send {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError>;
    /// Load already-read ROM bytes. Cores that only look at the path can
    /// ignore `bytes`.
    fn load_bytes(&mut self, rom: &Path, _bytes: Vec<u8>) -> Result<(), CoreError> {
        self.load(rom)
    }
    fn run_frame(&mut self, input: ButtonMask);
    fn run_frame_linked(&mut self, p1: ButtonMask, _p2: ButtonMask) {
        self.run_frame(p1);
    }
    fn set_option(&mut self, _key: &str, _value: &str) {}
    /// Every option the core offers: key, name, values with the default
    /// first.
    fn option_list(&self) -> Vec<(String, String, Vec<String>)> {
        Vec::new()
    }
    /// Turns the given cheats on, and every other off: each is its codes,
    /// several joined by `+`.
    fn set_cheats(&mut self, _codes: &[String]) {}
    /// The value an option has now.
    fn option_value(&self, _key: &str) -> Option<String> {
        None
    }
    fn set_frame_skip(&mut self, _skip: bool) {}
    fn video_xrgb8888(&self) -> &[u8];
    fn take_audio(&mut self) -> Vec<i16>;
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError>;
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError>;
    fn save_ram(&self) -> Option<Vec<u8>>;
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError>;
    fn av_info(&self) -> AvInfo;
    fn rumble(&self) -> Rumble {
        Rumble::default()
    }
    fn net(&self) -> Link {
        Link::default()
    }
    fn memory_regions(&self) -> Vec<MemoryRegion> {
        Vec::new()
    }
    fn start_link(&mut self, _client_id: u16) {}
    fn pump_link(&mut self) {}
    fn stop_link(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Recorder(Vec<ButtonMask>);

    impl RetroCore for Recorder {
        fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
            Ok(())
        }
        fn run_frame(&mut self, input: ButtonMask) {
            self.0.push(input);
        }
        fn video_xrgb8888(&self) -> &[u8] {
            &[]
        }
        fn take_audio(&mut self) -> Vec<i16> {
            Vec::new()
        }
        fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
            Ok(Vec::new())
        }
        fn unserialize(&mut self, _data: &[u8]) -> Result<(), CoreError> {
            Ok(())
        }
        fn save_ram(&self) -> Option<Vec<u8>> {
            None
        }
        fn load_save_ram(&mut self, _data: &[u8]) -> Result<(), CoreError> {
            Ok(())
        }
        fn av_info(&self) -> AvInfo {
            AvInfo {
                fps: 60.0,
                sample_rate: 48_000.0,
            }
        }
    }

    #[test]
    fn a_core_without_link_mode_runs_player_1_alone() {
        let mut core = Recorder::default();
        core.run_frame_linked(ButtonMask(ButtonMask::A), ButtonMask(ButtonMask::B));
        assert_eq!(core.0, vec![ButtonMask(ButtonMask::A)]);
    }
}
