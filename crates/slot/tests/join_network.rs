mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::net::{NetJob, Network};
use slot_input::{Action, Btn};
use slot_ui::QuickRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn at_join() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(QuickRow::Network) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    while a.quick_menu() != Some(QuickRow::JoinNetwork) {
        press(&mut a, Btn::Down);
    }
    (d, a)
}

fn nets() -> Vec<Network> {
    vec![
        Network {
            ssid: "Home Net".into(),
            signal: -50,
            secured: true,
        },
        Network {
            ssid: "Cafe".into(),
            signal: -70,
            secured: false,
        },
    ]
}

#[test]
fn join_network_scans_turns_wifi_on_and_lists_what_it_finds() {
    let (d, mut a) = at_join();
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Networks { scanning: true, .. }));
    assert!(a.net_jobs().contains(&NetJob::Scan));
    assert!(slot_store::read_slot_state(d.path()).wifi);
    let lines = a.settings_lines().expect("a popup");
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("Looking"));
    a.networks_found(nets());
    let lines = a.settings_lines().unwrap();
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("Home Net"));
    assert!(lines[slot_ui::option_slot(1)].0.ends_with("Open"));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::JoinNetwork));
}

#[test]
fn an_open_network_is_joined_at_once() {
    let (_d, mut a) = at_join();
    press(&mut a, Btn::A);
    a.networks_found(nets());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert!(a.net_jobs().contains(&NetJob::Join {
        ssid: "Cafe".into(),
        psk: String::new()
    }));
    assert_eq!(a.quick_menu(), Some(QuickRow::JoinNetwork));
}

#[test]
fn a_secured_network_asks_for_its_password_in_mixed_case() {
    let (_d, mut a) = at_join();
    press(&mut a, Btn::A);
    a.networks_found(nets());
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Password { .. }));
    assert!(a.search_light(), "the password would be set in capitals");
    assert_eq!(a.frame_split(), Some(slot_ui::SEARCH_SPLIT));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Y);
    press(&mut a, Btn::A);
    press(&mut a, Btn::X);
    let lines = a.search_lines().unwrap();
    assert_eq!(lines[slot_ui::search_hit_slot(1)].0, "qQ _");
    assert_eq!(lines[slot_ui::SEARCH_QUERY].0, "Join Home Net");
    press(&mut a, Btn::B);
    press(&mut a, Btn::Start);
    assert!(a.net_jobs().contains(&NetJob::Join {
        ssid: "Home Net".into(),
        psk: "qQ".into()
    }));
    assert_eq!(a.quick_menu(), Some(QuickRow::JoinNetwork));
}

#[test]
fn b_on_an_empty_password_goes_back_to_the_list() {
    let (_d, mut a) = at_join();
    press(&mut a, Btn::A);
    a.networks_found(nets());
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(matches!(
        a.phase(),
        Phase::Networks {
            scanning: false,
            ..
        }
    ));
    let lines = a.settings_lines().unwrap();
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("Home Net"));
}

#[test]
fn wifi_status_asks_what_wifi_is_doing() {
    let (_d, mut a) = at_join();
    while a.quick_menu() != Some(QuickRow::WifiStatus) {
        press(&mut a, Btn::Up);
    }
    press(&mut a, Btn::A);
    assert!(a.net_jobs().contains(&NetJob::Status));
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::WifiStatus),
        "the menu was left"
    );
}
