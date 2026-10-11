mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::bt::{BtDevice, BtJob};
use slot_input::{Action, Btn};
use slot_ui::QuickRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn shelf() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    (d, a)
}

fn to_row(a: &mut App, target: QuickRow) {
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(QuickRow::Network) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    while a.quick_menu() != Some(target) {
        press(a, Btn::Down);
    }
}

fn device(mac: &str, name: &str, paired: bool, connected: bool) -> BtDevice {
    BtDevice {
        mac: mac.into(),
        name: name.into(),
        paired,
        connected,
        audio: true,
    }
}

const BUDS: &str = "AA:BB:CC:DD:EE:01";
const PAD: &str = "AA:BB:CC:DD:EE:02";

#[test]
fn bluetooth_switches_on_and_off_from_the_network_page() {
    let (d, mut a) = shelf();
    to_row(&mut a, QuickRow::Bluetooth);
    press(&mut a, Btn::Right);
    assert!(slot_store::read_slot_state(d.path()).bluetooth);
    press(&mut a, Btn::Right);
    assert!(!slot_store::read_slot_state(d.path()).bluetooth);
    assert_eq!(a.bt_jobs(), [BtJob::Up, BtJob::Down]);
}

#[test]
fn devices_turns_bluetooth_on_and_scans() {
    let (d, mut a) = shelf();
    to_row(&mut a, QuickRow::BtDevices);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Bluetooth { scanning: true, .. }));
    assert_eq!(a.bt_jobs(), [BtJob::Up, BtJob::Scan]);
    assert!(slot_store::read_slot_state(d.path()).bluetooth);
    let lines = a.settings_lines().expect("popup text");
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Bluetooth Devices");
    assert!(lines[slot_ui::option_slot(0)].0.starts_with("Looking"));
}

#[test]
fn a_connects_a_device_and_disconnects_a_connected_one() {
    let (_d, mut a) = shelf();
    to_row(&mut a, QuickRow::BtDevices);
    press(&mut a, Btn::A);
    a.bt_found(vec![
        device(BUDS, "Buds Pro", true, true),
        device(PAD, "8BitDo Pad", false, false),
    ]);
    let lines = a.settings_lines().unwrap();
    assert_eq!(
        lines[slot_ui::option_slot(0)].0,
        "Buds Pro   \u{2713} Connected"
    );
    assert_eq!(lines[slot_ui::option_slot(1)].0, "8BitDo Pad");
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    let jobs = &a.bt_jobs()[2..];
    assert_eq!(
        jobs,
        [
            BtJob::Disconnect { mac: BUDS.into() },
            BtJob::Connect {
                mac: PAD.into(),
                name: "8BitDo Pad".into()
            }
        ]
    );
}

#[test]
fn x_forgets_only_a_paired_device_and_b_goes_back() {
    let (_d, mut a) = shelf();
    to_row(&mut a, QuickRow::BtDevices);
    press(&mut a, Btn::A);
    a.bt_found(vec![
        device(BUDS, "Buds Pro", true, false),
        device(PAD, "8BitDo Pad", false, false),
    ]);
    press(&mut a, Btn::X);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::X);
    assert_eq!(&a.bt_jobs()[2..], [BtJob::Forget { mac: BUDS.into() }]);
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::BtDevices));
}

#[test]
fn a_new_list_keeps_the_cursor_on_its_device() {
    let (_d, mut a) = shelf();
    to_row(&mut a, QuickRow::BtDevices);
    press(&mut a, Btn::A);
    a.bt_found(vec![
        device(BUDS, "Buds Pro", false, false),
        device(PAD, "8BitDo Pad", false, false),
    ]);
    press(&mut a, Btn::Down);
    a.bt_found(vec![
        device(PAD, "8BitDo Pad", true, true),
        device(BUDS, "Buds Pro", false, false),
    ]);
    assert!(matches!(a.phase(), Phase::Bluetooth { at: 0, .. }));
}
