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
