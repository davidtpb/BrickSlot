mod common;

use common::{app_booting_at, app_playing_with, tmp_root_with_carts, StubSnapshot, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::cheats::Cheat;
use slot_store::emulator::{self, CoreOption, Scope};
use slot_store::{Core, Platform};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn options() -> Vec<CoreOption> {
    vec![
        CoreOption {
            key: "mgba_skip_bios".into(),
            desc: "Skip BIOS Intro".into(),
            values: vec!["OFF".into(), "ON".into()],
        },
        CoreOption {
            key: "mgba_idle_optimization".into(),
            desc: "Idle Loop Removal".into(),
            values: vec![
                "Remove Known".into(),
                "Detect and Remove".into(),
                "Don't Remove".into(),
            ],
        },
    ]
}

fn playing() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut a = app_playing_with(d.path(), "Fusion", StubSnapshot::boxed());
    a.core_opened(
        options(),
        vec!["OFF".into(), "Remove Known".into()],
        Vec::new(),
        vec![Cheat {
            desc: "Infinite missiles".into(),
            code: "82025BC4+FFFF".into(),
            enabled: false,
        }],
    );
    assert!(a.take_cheats().is_none(), "no cheat was on");
    (d, a)
}

fn rows(a: &App) -> Vec<String> {
    let lines = a.settings_lines().expect("menu text");
    (0..slot_ui::POPUP_OPTIONS)
        .map(|i| lines[slot_ui::option_slot(i)].0.clone())
        .collect()
}

/// Opens the pause menu and goes into Options.
fn options_menu(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..3 {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Options"
    );
}

#[test]
fn the_pause_menu_lists_what_nx_redux_s_does_and_holds_the_game() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert!(a.menu_open());
    let lines = a.settings_lines().unwrap();
    assert_eq!(lines[slot_ui::option_slot(0)].0, "Resume");
    assert_eq!(lines[slot_ui::option_slot(1)].0, "Save State");
    assert_eq!(lines[slot_ui::option_slot(2)].0, "Load State");
    assert_eq!(lines[slot_ui::option_slot(3)].0, "Options");
    press(&mut a, Btn::B);
    assert!(!a.menu_open());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn quit_game_ejects() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Up);
    assert!(rows(&a).iter().any(|r| r == "Quit Game"));
    press(&mut a, Btn::A);
    assert!(!a.menu_open());
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
}

#[test]
fn an_emulator_option_changes_live_and_save_changes_keeps_it_for_the_game() {
    let (d, mut a) = playing();
    options_menu(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Emulator Options"
    );
    assert!(rows(&a)[0].starts_with("Skip BIOS Intro"));
    assert!(rows(&a)[0].ends_with("OFF"));
    press(&mut a, Btn::Right);
    assert!(rows(&a)[0].ends_with("ON"));
    assert_eq!(
        a.take_option_changes(),
        [("mgba_skip_bios".to_string(), "ON".to_string())]
    );
    press(&mut a, Btn::B);
    for _ in 0..4 {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Save Changes"
    );
    press(&mut a, Btn::A);
    assert!(!a.menu_open());
    let game = Scope::Game(Platform::Gba, "Fusion".into());
    assert_eq!(
        emulator::read(d.path(), &game),
        [("mgba_skip_bios".to_string(), "ON".to_string())]
    );
}

#[test]
fn saving_for_the_system_moves_the_game_s_settings_to_every_game() {
    let (d, mut a) = playing();
    options_menu(&mut a);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::B);
    for _ in 0..4 {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_eq!(
        emulator::read(d.path(), &Scope::System(Platform::Gba)),
        [(
            "mgba_idle_optimization".to_string(),
            "Detect and Remove".to_string()
        )]
    );
    assert!(emulator::read(d.path(), &Scope::Game(Platform::Gba, "Fusion".into())).is_empty());
}

#[test]
fn the_shader_picked_in_the_game_shows_at_once() {
    let (_d, mut a) = playing();
    let before = a.screen_shader();
    options_menu(&mut a);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Shader"
    );
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_ne!(a.screen_shader(), before);
}

#[test]
fn a_cheat_turned_on_goes_to_the_emulator_and_into_its_file() {
    let (d, mut a) = playing();
    options_menu(&mut a);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Cheats"
    );
    assert!(rows(&a)[0].ends_with("Infinite missiles"));
    press(&mut a, Btn::A);
    assert!(rows(&a)[0].starts_with('\u{2713}'));
    assert_eq!(a.take_cheats(), Some(vec!["82025BC4+FFFF".to_string()]));
    let kept = slot_store::cheats::read(d.path(), Platform::Gba, "Fusion");
    assert!(kept[0].enabled);
}

#[test]
fn controls_swap_a_and_b() {
    let (_d, mut a) = playing();
    options_menu(&mut a);
    for _ in 0..3 {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "Controls"
    );
    assert!(!a.swap_ab());
    press(&mut a, Btn::A);
    assert!(a.swap_ab());
    assert!(rows(&a)[0].ends_with("On"));
}

#[test]
fn saved_settings_come_back_when_the_game_starts() {
    let (_d, mut a) = playing();
    a.core_opened(
        options(),
        vec!["ON".into(), "Remove Known".into()],
        vec![
            ("mgba_skip_bios".into(), "ON".into()),
            ("slot_swap_ab".into(), "true".into()),
            ("slot_shader".into(), "grid".into()),
        ],
        Vec::new(),
    );
    assert!(a.swap_ab());
    assert_eq!(a.screen_shader(), slot_store::Shader::Grid);
}

#[test]
fn emulator_settings_edit_a_system_s_defaults_from_settings() {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    emulator::remember_options(d.path(), Core::Mgba, &options()).unwrap();
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(slot_ui::QuickRow::Tools) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    while a.quick_menu() != Some(slot_ui::QuickRow::EmulatorSettings) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert!(a.menu_open());
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "GBA Emulator"
    );
    press(&mut a, Btn::Right);
    assert_eq!(
        emulator::read(d.path(), &Scope::System(Platform::Gba)),
        [("mgba_skip_bios".to_string(), "ON".to_string())]
    );
    press(&mut a, Btn::Y);
    assert!(emulator::read(d.path(), &Scope::System(Platform::Gba)).is_empty());
    press(&mut a, Btn::R1);
    assert_eq!(
        a.settings_lines().unwrap()[slot_ui::POPUP_TITLE_SLOT].0,
        "GB Emulator"
    );
    press(&mut a, Btn::B);
    assert!(!a.menu_open());
    assert_eq!(a.quick_menu(), Some(slot_ui::QuickRow::EmulatorSettings));
}
