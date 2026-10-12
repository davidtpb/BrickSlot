mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::ShellFinish;
use slot_ui::{CustomFocus, Material, SWATCHES};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn shelf() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Pokémon Emerald", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    (d, a)
}

fn saved(d: &tempfile::TempDir) -> std::collections::HashMap<String, slot_store::ShellChoice> {
    let text =
        std::fs::read_to_string(d.path().join(slot_store::CART_SHELL_FILE)).unwrap_or_default();
    slot_store::cart_shell::choices(&text)
}

#[test]
fn x_in_inspect_opens_customize_and_b_closes_it() {
    let (_d, mut a) = shelf();
    press(&mut a, Btn::X);
    press(&mut a, Btn::X);
    assert!(a.customize_open());
    assert_eq!(a.frame_split(), Some(slot_ui::SOCKET_SPLIT));
    let lines = a.settings_lines().expect("panel text");
    assert_eq!(lines[slot_ui::CUSTOM_TITLE].0, "Customize");
    assert_eq!(lines[slot_ui::CUSTOM_TYPE_VALUE].0, "Default");
    press(&mut a, Btn::B);
    assert!(!a.customize_open());
}

#[test]
fn a_picked_colour_shows_at_once_and_is_kept_for_that_game() {
    let (d, mut a) = shelf();
    let stem = a.selected_stem().unwrap().to_string();
    let generation = a.shell_generation();
    press(&mut a, Btn::X);
    press(&mut a, Btn::X);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    let shell = a.selected_shell().expect("no colour on the cart");
    assert_eq!(shell.colour, SWATCHES[7]);
    assert_eq!(shell.finish, ShellFinish::Solid);
    assert_ne!(
        a.shell_generation(),
        generation,
        "the cart's face is not rebuilt"
    );
    assert!(saved(&d).is_empty(), "saved before the panel closed");
    press(&mut a, Btn::B);
    let kept = saved(&d);
    assert_eq!(kept.get(&slot_store::cart_shell::key(&stem)), Some(&shell));
}

#[test]
fn the_material_list_picks_clear_and_default_forgets_the_colour() {
    let (d, mut a) = shelf();
    press(&mut a, Btn::X);
    press(&mut a, Btn::X);
    press(&mut a, Btn::A);
    let lines = a.settings_lines().unwrap();
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Type");
    assert_eq!(lines[slot_ui::option_slot(2)].0, "Clear");
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_eq!(Material::of(a.selected_shell()), Material::Clear);
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Left);
    assert_eq!(Material::of(a.selected_shell()), Material::Default);
    press(&mut a, Btn::B);
    assert!(saved(&d).is_empty(), "an unchanged cart was written back");
}

#[test]
fn the_shelf_stays_put_under_the_panel() {
    let (_d, mut a) = shelf();
    let before = a.selected_stem().map(str::to_owned);
    press(&mut a, Btn::X);
    press(&mut a, Btn::X);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Left);
    press(&mut a, Btn::B);
    assert_eq!(a.selected_stem().map(str::to_owned), before);
    let _ = CustomFocus::Material;
}
