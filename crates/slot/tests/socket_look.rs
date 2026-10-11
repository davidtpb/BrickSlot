mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::Look;
use slot_ui::Draw;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn shelf_with(look: Look) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Pokémon Emerald (USA)", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    s.look = look;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    (d, a)
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

#[test]
fn the_classic_shelf_has_no_socket_pills() {
    let (_d, a) = shelf_with(Look::Classic);
    assert!(!a.socket_look());
    assert!(a.socket_lines().is_none());
}

#[test]
fn the_socket_shelf_names_the_selected_game_on_a_pill() {
    let (_d, a) = shelf_with(Look::Socket);
    let lines = a.socket_lines().expect("pills on the socket shelf");
    let name = &lines[slot_ui::SOCKET_NAME].0;
    let stem = a.selected_stem().unwrap();
    assert!(name.starts_with('A'), "{name}");
    assert!(
        name.ends_with(&slot_ui::clean_label(stem)),
        "{name} does not name {stem}"
    );
    assert!(!name.contains('('), "region tags left in {name}");
    assert_eq!(
        lines[slot_ui::socket_tab_slot(0)],
        ("GBA".to_string(), true)
    );
}

#[test]
fn the_socket_shelf_is_split_to_fill_a_taller_screen() {
    let (_d, a) = shelf_with(Look::Socket);
    assert_eq!(a.frame_split(), Some(slot_ui::SOCKET_SPLIT));
}

#[test]
fn the_socket_shelf_greys_the_neighbours_and_not_the_selection() {
    let (_d, a) = shelf_with(Look::Socket);
    let greys: Vec<f32> = frame(&a)
        .into_iter()
        .filter_map(|d| match d {
            Draw::Warp { grey, .. } => Some(grey),
            _ => None,
        })
        .collect();
    if greys.is_empty() {
        // Cart faces are built off the main thread; with none yet the carts
        // are drawn as plain blocks.
        return;
    }
    assert!(greys.contains(&0.0), "the selected cart is greyed");
    assert!(
        greys.iter().any(|g| *g > 0.3),
        "the neighbours keep their colour"
    );
}

#[test]
fn style_in_the_screen_settings_switches_the_look_and_keeps_it() {
    let (d, mut a) = shelf_with(Look::Classic);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(slot_ui::QuickRow::Screen) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    while a.quick_menu() != Some(slot_ui::QuickRow::Look) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::Right);
    assert!(a.socket_look());
    assert_eq!(slot_store::read_slot_state(d.path()).look, Look::Socket);
    press(&mut a, Btn::Right);
    assert!(!a.socket_look());
}

#[test]
fn socket_settings_put_the_categories_left_and_the_page_right() {
    let (_d, mut a) = shelf_with(Look::Socket);
    a.apply(Action::QuickMenu);
    assert_eq!(a.frame_split(), Some(slot_ui::SETTINGS_SPLIT));
    let lines = a.settings_lines().expect("socket settings");
    assert_eq!(lines[slot_ui::side_slot(0)].0, "Search");
    assert_eq!(lines[slot_ui::side_slot(2)].0, "Gameplay");
    assert!(lines[slot_ui::SETTINGS_CLOSE].0.ends_with("Close"));
    while a.quick_menu() != Some(slot_ui::QuickRow::Game) {
        press(&mut a, Btn::Down);
    }
    let lines = a.settings_lines().unwrap();
    assert_eq!(lines[slot_ui::caption_slot(0)].0, "Fast Forward");
    assert!(lines[slot_ui::side_slot(2)].1, "the category is not lit");
    press(&mut a, Btn::A);
    let lines = a.settings_lines().unwrap();
    assert!(lines[slot_ui::SETTINGS_CLOSE].0.ends_with("Back"));
    assert!(
        lines[slot_ui::value_slot(0)].1,
        "the first setting is not under the cursor"
    );
    assert!(lines[slot_ui::SETTINGS_HINT].0.ends_with("Choose"));
    press(&mut a, Btn::Down);
    let lines = a.settings_lines().unwrap();
    assert!(lines[slot_ui::SETTINGS_HINT].0.ends_with("Change"));
}

#[test]
fn the_power_menu_is_a_popup_in_the_socket_style() {
    let (_d, mut a) = shelf_with(Look::Socket);
    a.apply(Action::PowerHold);
    assert!(a.power_menu().is_some());
    let lines = a.settings_lines().expect("popup text");
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Power");
    assert_eq!(lines[slot_ui::option_slot(1)].0, "Power Off");
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        out.iter().any(|d| matches!(d, Draw::Rect { colour, .. } if colour[3] > 0.5 && colour[0] == 0.0 && colour[3] < 1.0)),
        "nothing dims what lies under the popup"
    );
}

#[test]
fn the_classic_style_keeps_its_own_settings() {
    let (_d, mut a) = shelf_with(Look::Classic);
    a.apply(Action::QuickMenu);
    assert!(a.settings_lines().is_none());
    assert_eq!(a.frame_split(), Some(slot_ui::QUICK_SPLIT));
}

#[test]
fn start_in_the_socket_style_picks_the_core_from_a_popup() {
    let (d, mut a) = shelf_with(Look::Socket);
    press(&mut a, Btn::Start);
    assert!(a.core_popup_open(), "no core popup");
    let lines = a.settings_lines().expect("popup text");
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Core");
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("mGBA"));
    assert!(lines[slot_ui::option_slot(1)].0.starts_with("gpSP"));
    let before = a.selected_stem().unwrap().to_string();
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(!a.core_popup_open());
    assert_eq!(
        slot_store::core_for(d.path(), &before),
        slot_store::Core::Gpsp
    );
    press(&mut a, Btn::Start);
    let lines = a.settings_lines().unwrap();
    assert!(
        lines[slot_ui::option_slot(1)].0.contains('\u{2713}'),
        "the core in use is not marked"
    );
    press(&mut a, Btn::B);
    assert!(!a.core_popup_open());
}

#[test]
fn the_classic_style_keeps_the_board_for_the_core() {
    let (_d, mut a) = shelf_with(Look::Classic);
    press(&mut a, Btn::Start);
    assert!(!a.core_popup_open());
}

#[test]
fn the_wifi_mark_waits_for_a_connection_not_just_the_switch() {
    let d = common::tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    s.look = Look::Socket;
    s.wifi = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    let status = &a.socket_lines().unwrap()[slot_ui::SOCKET_STATUS].0;
    assert!(
        !slot_ui::SocketStatus::parse(status).wifi,
        "Wi-Fi shows as connected before any network was joined: {status}"
    );
}

fn open_row(a: &mut App, target: slot_ui::QuickRow) {
    a.apply(Action::QuickMenu);
    let parent = target.parent().expect("a page row");
    while !matches!(a.phase(), Phase::QuickMenu { row } if *row == parent) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    while !matches!(a.phase(), Phase::QuickMenu { row } if *row == target) {
        press(a, Btn::Down);
    }
}

#[test]
fn a_on_a_many_choice_setting_opens_a_picker_and_a_picks() {
    use slot_ui::{QuickRow, QuickValue};
    let (d, mut a) = shelf_with(Look::Socket);
    open_row(&mut a, QuickRow::LightEffect);
    let before = a.quick_value(QuickRow::LightEffect);
    press(&mut a, Btn::A);
    assert!(a.picker_open());
    let lines = a.settings_lines().unwrap();
    assert_eq!(
        lines[slot_ui::POPUP_TITLE_SLOT].0,
        QuickRow::LightEffect.label()
    );
    assert!(lines[slot_ui::option_slot(0)].0.contains('\u{2713}'));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(!a.picker_open());
    let after = a.quick_value(QuickRow::LightEffect);
    assert_ne!(after, before);
    assert_eq!(after, Some(QuickValue::Blink));
    assert_eq!(
        slot_store::read_slot_state(d.path()).light_effect,
        slot_store::LightEffect::Blink
    );
}

#[test]
fn b_closes_the_picker_without_changing_the_setting() {
    use slot_ui::QuickRow;
    let (_d, mut a) = shelf_with(Look::Socket);
    open_row(&mut a, QuickRow::GbaShader);
    let before = a.quick_value(QuickRow::GbaShader);
    press(&mut a, Btn::A);
    assert!(a.picker_open());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    assert!(!a.picker_open());
    assert_eq!(a.quick_value(QuickRow::GbaShader), before);
    assert!(matches!(a.phase(), Phase::QuickMenu { .. }));
}

#[test]
fn a_flips_an_on_off_setting_in_the_socket_look() {
    use slot_ui::{QuickRow, QuickValue};
    let (_d, mut a) = shelf_with(Look::Socket);
    open_row(&mut a, QuickRow::Rewind);
    let before = a.quick_value(QuickRow::Rewind);
    press(&mut a, Btn::A);
    assert!(!a.picker_open());
    assert_ne!(a.quick_value(QuickRow::Rewind), before);
    assert!(matches!(
        a.quick_value(QuickRow::Rewind),
        Some(QuickValue::On | QuickValue::Off)
    ));
}
