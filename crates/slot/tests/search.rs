mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::search::KEYS;
use slot_input::{Action, Btn};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn type_text(a: &mut App, text: &str) {
    for c in text.chars() {
        let key = c.to_ascii_uppercase().to_string();
        let (row, col) = KEYS
            .iter()
            .enumerate()
            .find_map(|(r, keys)| keys.iter().position(|k| *k == key).map(|c| (r, c)))
            .expect("a key for every letter typed");
        while a.search().row != row {
            press(a, Btn::Down);
        }
        while a.search().col != col {
            press(a, Btn::Right);
        }
        press(a, Btn::A);
    }
}

fn shelf_of(stems: &[&str]) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(stems);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    (d, a)
}

#[test]
fn y_on_the_shelf_opens_search_and_typing_narrows_the_games() {
    let (_d, mut a) = shelf_of(&["Fusion", "Pokémon Emerald", "Zero Mission"]);
    press(&mut a, Btn::Y);
    assert!(matches!(a.phase(), Phase::Search));
    type_text(&mut a, "poke");
    assert_eq!(a.search().query, "POKE");
    let names: Vec<&str> = a.search().hits.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, ["Pokémon Emerald"], "accents are ignored");
    let lines = a.search_lines().expect("search lines while searching");
    assert_eq!(lines[slot_ui::SEARCH_QUERY].0, "Search: POKE_");
    assert_eq!(
        lines[slot_ui::search_hit_slot(0)].0,
        "Pokémon Emerald  (GBA)"
    );
}

#[test]
fn start_goes_to_the_picked_game_on_the_shelf() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3", "Zero Mission"]);
    assert_eq!(a.selected_stem(), Some("Fusion"));
    press(&mut a, Btn::Y);
    type_text(&mut a, "zero");
    press(&mut a, Btn::Start);
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(a.selected_stem(), Some("Zero Mission"));
}

#[test]
fn b_deletes_and_backs_out_once_the_search_is_empty() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Y);
    type_text(&mut a, "fu");
    press(&mut a, Btn::B);
    assert_eq!(a.search().query, "F");
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Search));
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn nothing_found_says_so() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Y);
    type_text(&mut a, "qq");
    let lines = a.search_lines().unwrap();
    assert_eq!(lines[slot_ui::search_hit_slot(0)].0, "No games found");
    press(&mut a, Btn::Start);
    assert!(
        matches!(a.phase(), Phase::Search),
        "START with nothing found went somewhere"
    );
}

#[test]
fn home_opens_and_closes_search_too() {
    let (_d, mut a) = shelf_of(&["Fusion", "Mother 3"]);
    press(&mut a, Btn::Home);
    assert!(matches!(a.phase(), Phase::Search));
    press(&mut a, Btn::Home);
    assert!(matches!(a.phase(), Phase::Shelf));
}
