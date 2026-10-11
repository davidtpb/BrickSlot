mod device;
mod motor;
mod power;
mod sim;

pub use device::{
    has_bit, hue_rgb, motor_change, read_only, read_only_in, record_first_frame, rumble_node,
    trace_first_frame, uptime_seconds, Board, DevicePlatform, BOARD_VAR,
};
pub use motor::Motor;
pub use power::Power;
pub use sim::SimPlatform;

use std::path::Path;
use std::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Charge {
    Unknown,
    Discharging,
    Charging,
    Full,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Battery {
    pub percent: u8,
    pub charge: Charge,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum LedState {
    Off,
    Running,
    Low,
    Charging,
    Charged,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum LightsMode {
    #[default]
    Battery,
    Colour,
    Off,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum LightsEffect {
    #[default]
    Static,
    Breathe,
    Blink,
    Rainbow,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Lights {
    pub mode: LightsMode,
    pub rgb: u32,
    pub effect: LightsEffect,
    pub percent: u8,
}

impl Default for Lights {
    fn default() -> Self {
        Lights {
            mode: LightsMode::Battery,
            rgb: 0xFFFFFF,
            effect: LightsEffect::Static,
            percent: 50,
        }
    }
}

pub trait Platform: Send {
    fn set_backlight(&mut self, step: u8);
    fn battery(&self) -> Option<Battery>;
    fn charge(&self) -> Charge;
    fn set_led(&mut self, state: LedState);
    fn poweroff(&mut self) -> !;
    fn restart(&mut self) -> !;
    fn root(&self) -> &Path;
    fn now(&self) -> i64;
    fn set_clock(&mut self, secs: i64);
    fn set_rumble(&mut self, strength: u16);

    fn relink_adb(&mut self) -> bool {
        false
    }

    fn set_lights(&mut self, _lights: Lights) {}

    /// Whether the handheld can suspend to RAM instead of powering off
    /// after sleeping a while.
    fn can_deep_sleep(&self) -> bool {
        false
    }

    /// Suspends to RAM and returns once woken. False if it never slept.
    fn deep_sleep(&mut self) -> bool {
        false
    }

    fn headphones(&self) -> bool {
        false
    }

    fn card_read_only(&self) -> bool {
        false
    }
}

pub trait LidPolicy {
    fn on_close(&mut self);
    fn on_open(&mut self);
    fn timeout(&self) -> Duration;
}
