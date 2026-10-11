use std::fs;
use std::path::PathBuf;

use slot_power::{Board, DevicePlatform, LedState, Lights, LightsEffect, LightsMode, Platform};
use tempfile::TempDir;

fn brick_pro_tree() -> (TempDir, TempDir) {
    let sys = tempfile::tempdir().unwrap();
    let dev = tempfile::tempdir().unwrap();
    let battery = sys.path().join("class/power_supply/axp2202-battery");
    fs::create_dir_all(&battery).unwrap();
    fs::write(battery.join("type"), "Battery\n").unwrap();
    fs::write(battery.join("capacity"), "64\n").unwrap();
    fs::write(battery.join("status"), "Discharging\n").unwrap();
    let gpio = sys.path().join("class/gpio/gpio227");
    fs::create_dir_all(&gpio).unwrap();
    fs::write(gpio.join("value"), "0").unwrap();
    let motor = sys.path().join("class/motor");
    fs::create_dir_all(&motor).unwrap();
    fs::write(motor.join("voltage"), "0").unwrap();
    let anim = sys.path().join("class/led_anim");
    fs::create_dir_all(&anim).unwrap();
    fs::write(anim.join("max_scale"), "0").unwrap();
    (sys, dev)
}

fn brick_pro(sys: &TempDir, dev: &TempDir) -> DevicePlatform {
    DevicePlatform::probe_board(
        sys.path(),
        dev.path(),
        PathBuf::from("/mnt/SDCARD"),
        Board::TrimUiBrickPro,
    )
}

fn read(sys: &TempDir, file: &str) -> String {
    fs::read_to_string(sys.path().join(file))
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn the_board_comes_from_the_launcher_before_cpuinfo() {
    assert_eq!(
        Board::from_hints(Some("brickpro"), "Hardware : TG3040"),
        Board::TrimUiBrickPro
    );
    assert_eq!(
        Board::from_hints(None, "Hardware\t: TG4040"),
        Board::TrimUiBrickPro
    );
    assert_eq!(
        Board::from_hints(None, "Hardware\t: TG3040"),
        Board::TrimUiBrick
    );
    assert_eq!(Board::from_hints(Some(""), "sun50iw9"), Board::Anbernic);
    assert!(Board::TrimUiBrickPro.is_trimui());
    assert!(!Board::Anbernic.is_trimui());
}

#[test]
fn the_brick_pro_rumbles_through_its_gpio_capped_at_two_and_a_half_volts() {
    let (sys, dev) = brick_pro_tree();
    let mut p = brick_pro(&sys, &dev);
    assert!(p.report().contains("motor gpio227"), "{}", p.report());
    p.set_rumble(u16::MAX);
    assert_eq!(read(&sys, "class/gpio/gpio227/value"), "1");
    assert_eq!(read(&sys, "class/motor/voltage"), "2500000");
    p.set_rumble(0);
    assert_eq!(read(&sys, "class/gpio/gpio227/value"), "0");
}

#[test]
fn the_top_bar_stays_dark_while_running_and_lights_when_low() {
    let (sys, dev) = brick_pro_tree();
    let mut p = brick_pro(&sys, &dev);
    p.set_led(LedState::Running);
    assert_eq!(read(&sys, "class/led_anim/max_scale"), "0");
    p.set_led(LedState::Low);
    assert_eq!(read(&sys, "class/led_anim/effect_rgb_hex_m"), "FF0000");
    assert_ne!(read(&sys, "class/led_anim/max_scale"), "0");
    p.set_led(LedState::Off);
    assert_eq!(read(&sys, "class/led_anim/max_scale"), "0");
}

#[test]
fn the_brick_pro_reads_its_battery_like_the_anbernics() {
    let (sys, dev) = brick_pro_tree();
    let p = brick_pro(&sys, &dev);
    assert_eq!(p.battery().map(|b| b.percent), Some(64));
    assert!(p.report().starts_with("TrimUiBrickPro"), "{}", p.report());
}

#[test]
fn without_dev_disp_a_backlight_class_still_dims_the_panel() {
    let (sys, dev) = brick_pro_tree();
    let bl = sys.path().join("class/backlight/backlight0");
    fs::create_dir_all(&bl).unwrap();
    fs::write(bl.join("max_brightness"), "255\n").unwrap();
    fs::write(bl.join("brightness"), "0\n").unwrap();
    let mut p = brick_pro(&sys, &dev);
    p.set_backlight(9);
    assert_eq!(read(&sys, "class/backlight/backlight0/brightness"), "255");
}

#[test]
fn colour_mode_lights_every_zone_and_off_darkens_them_all() {
    let (sys, dev) = brick_pro_tree();
    let mut p = brick_pro(&sys, &dev);
    p.set_led(LedState::Running);
    p.set_lights(Lights {
        mode: LightsMode::Colour,
        rgb: 0x8A2BE2,
        effect: LightsEffect::Breathe,
        percent: 100,
    });
    for zone in ["m", "lr", "f1", "f2", "rear"] {
        assert_eq!(
            read(&sys, &format!("class/led_anim/effect_rgb_hex_{zone}")),
            "8A2BE2"
        );
        assert_eq!(read(&sys, &format!("class/led_anim/effect_{zone}")), "2");
        assert_eq!(
            read(&sys, &format!("class/led_anim/effect_cycles_{zone}")),
            "-1"
        );
    }
    for scale in [
        "max_scale",
        "max_scale_lr",
        "max_scale_f1f2",
        "max_scale_rear",
    ] {
        assert_eq!(read(&sys, &format!("class/led_anim/{scale}")), "100");
    }
    p.set_lights(Lights {
        mode: LightsMode::Off,
        ..Lights::default()
    });
    for scale in [
        "max_scale",
        "max_scale_lr",
        "max_scale_f1f2",
        "max_scale_rear",
    ] {
        assert_eq!(read(&sys, &format!("class/led_anim/{scale}")), "0");
    }
}

#[test]
fn rainbow_walks_the_lights_round_the_colour_wheel_and_stops_when_changed() {
    let (sys, dev) = brick_pro_tree();
    let mut p = brick_pro(&sys, &dev);
    p.set_led(LedState::Running);
    p.set_lights(Lights {
        mode: LightsMode::Colour,
        rgb: 0xFFFFFF,
        effect: LightsEffect::Rainbow,
        percent: 50,
    });
    std::thread::sleep(std::time::Duration::from_millis(50));
    let first = read(&sys, "class/led_anim/effect_rgb_hex_m");
    std::thread::sleep(std::time::Duration::from_millis(400));
    let later = read(&sys, "class/led_anim/effect_rgb_hex_m");
    assert_ne!(first, later, "the colour never moved");
    assert_eq!(
        read(&sys, "class/led_anim/effect_m"),
        "4",
        "rainbow rides a static light"
    );
    assert_eq!(read(&sys, "class/led_anim/effect_rgb_hex_lr"), later);
    p.set_lights(Lights {
        mode: LightsMode::Colour,
        rgb: 0x0000FF,
        effect: LightsEffect::Static,
        percent: 50,
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_eq!(
        read(&sys, "class/led_anim/effect_rgb_hex_m"),
        "0000FF",
        "the rainbow kept going"
    );
}

#[test]
fn the_colour_wheel_lands_on_its_primaries() {
    assert_eq!(slot_power::hue_rgb(0.0), 0xFF0000);
    assert_eq!(slot_power::hue_rgb(120.0), 0x00FF00);
    assert_eq!(slot_power::hue_rgb(240.0), 0x0000FF);
    assert_eq!(slot_power::hue_rgb(360.0), 0xFF0000);
}

#[test]
fn deep_sleep_writes_mem_to_the_kernels_power_state() {
    let (sys, dev) = brick_pro_tree();
    let p = brick_pro(&sys, &dev);
    assert!(!p.can_deep_sleep(), "no /sys/power/state, no deep sleep");
    fs::create_dir_all(sys.path().join("power")).unwrap();
    fs::write(sys.path().join("power/state"), "").unwrap();
    let mut p = brick_pro(&sys, &dev);
    assert!(p.can_deep_sleep());
    assert!(p.deep_sleep());
    assert_eq!(read(&sys, "power/state"), "mem");
}

#[test]
fn an_anbernic_still_powers_off_instead_of_deep_sleeping() {
    let (sys, _dev) = brick_pro_tree();
    fs::create_dir_all(sys.path().join("power")).unwrap();
    fs::write(sys.path().join("power/state"), "").unwrap();
    let p = DevicePlatform::probe(sys.path(), PathBuf::from("/mnt/sdcard"));
    assert!(!p.can_deep_sleep());
}

#[test]
fn the_new_effects_draw_what_their_names_say() {
    use slot_power::effect_colour as at;
    let red = |c: u32| c >> 16;
    let green = |c: u32| (c >> 8) & 0xFF;
    let blue = |c: u32| c & 0xFF;
    // Rainbow Wave puts each light at its own spot on the wheel.
    let wave: Vec<u32> = (0..4)
        .map(|z| at(LightsEffect::RainbowWave, 0, 0, z, 0))
        .collect();
    assert_eq!(wave[0], 0xFF0000);
    assert!(wave.windows(2).all(|w| w[0] != w[1]), "{wave:06X?}");
    // Ocean stays blue-green, never red.
    for ms in (0..6000).step_by(250) {
        let c = at(LightsEffect::Ocean, 0, ms, 0, 0);
        assert!(red(c) < 0x20 && blue(c) > 0x60, "{c:06X} at {ms}");
    }
    // Fire stays red-orange-yellow, never blue.
    for noise in [0u32, 0x1234_5678, 0xFFFF_FFFF, 0x00FF_00FF] {
        let c = at(LightsEffect::Fire, 0, 0, 0, noise);
        assert!(
            red(c) > 0x80 && blue(c) == 0 && green(c) <= red(c),
            "{c:06X}"
        );
    }
    // Police: neighbouring lights flash opposite colours, then swap.
    let (a, b) = (
        at(LightsEffect::Police, 0, 10, 0, 0),
        at(LightsEffect::Police, 0, 10, 1, 0),
    );
    assert_eq!((a, b), (0xFF0000, 0x0030FF));
    assert_eq!(at(LightsEffect::Police, 0, 410, 0, 0), 0x0030FF);
    // Heartbeat beats in the chosen colour: bright on the beat, dim between.
    let beat = at(LightsEffect::Heartbeat, 0x00FF00, 120, 0, 0);
    let rest = at(LightsEffect::Heartbeat, 0x00FF00, 900, 0, 0);
    assert!(green(beat) > 0xF0 && green(rest) < 0x30 && red(beat) == 0);
}

#[test]
fn the_new_effects_play_over_a_static_light() {
    let (sys, dev) = brick_pro_tree();
    let mut p = brick_pro(&sys, &dev);
    p.set_led(LedState::Running);
    p.set_lights(Lights {
        mode: LightsMode::Colour,
        rgb: 0xFFFFFF,
        effect: LightsEffect::Aurora,
        percent: 50,
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert_eq!(read(&sys, "class/led_anim/effect_m"), "4");
    let m = read(&sys, "class/led_anim/effect_rgb_hex_m");
    let lr = read(&sys, "class/led_anim/effect_rgb_hex_lr");
    assert_ne!(m, "FFFFFF", "Aurora never drew");
    assert_ne!(m, lr, "the lights are not offset along the palette");
}

fn cpufreq(sys: &TempDir) -> std::path::PathBuf {
    let dir = sys.path().join("devices/system/cpu/cpu0/cpufreq");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("scaling_available_frequencies"),
        "408000 600000 816000 1008000 1200000 1416000 1608000 1800000\n",
    )
    .unwrap();
    fs::write(
        dir.join("scaling_available_governors"),
        "ondemand userspace powersave conservative performance\n",
    )
    .unwrap();
    fs::write(dir.join("scaling_governor"), "ondemand\n").unwrap();
    fs::write(dir.join("scaling_min_freq"), "1008000\n").unwrap();
    fs::write(dir.join("scaling_max_freq"), "1800000\n").unwrap();
    dir
}

#[test]
fn performance_profiles_set_the_clock_range_and_default_puts_it_back() {
    use slot_power::CpuProfile;
    let (sys, dev) = brick_pro_tree();
    let dir = cpufreq(&sys);
    let mut p = brick_pro(&sys, &dev);
    let now = |f: &str| fs::read_to_string(dir.join(f)).unwrap().trim().to_string();
    p.set_cpu(CpuProfile::Saver);
    assert_eq!(
        (
            now("scaling_governor"),
            now("scaling_min_freq"),
            now("scaling_max_freq")
        ),
        ("conservative".into(), "408000".into(), "1200000".into())
    );
    p.set_cpu(CpuProfile::Performance);
    assert_eq!(
        (
            now("scaling_governor"),
            now("scaling_min_freq"),
            now("scaling_max_freq")
        ),
        ("performance".into(), "1800000".into(), "1800000".into())
    );
    p.set_cpu(CpuProfile::Stock);
    assert_eq!(
        (
            now("scaling_governor"),
            now("scaling_min_freq"),
            now("scaling_max_freq")
        ),
        ("ondemand".into(), "1008000".into(), "1800000".into()),
        "Default did not put back what the system had"
    );
}
