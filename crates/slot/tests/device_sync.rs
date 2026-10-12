mod common;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use slot::sync::{run, SyncNews, SyncOptions};

fn put(root: &Path, path: &str, bytes: &[u8], age_s: u64) {
    let p = root.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, bytes).unwrap();
    let f = std::fs::File::options().write(true).open(&p).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(age_s))
        .unwrap();
}

fn free_udp() -> u16 {
    std::net::UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn sync_pair(a: &Path, b: &Path, roms: bool) -> (Vec<SyncNews>, Vec<SyncNews>) {
    let (pa, pb) = (free_udp(), free_udp());
    let opts = |listen: u16, to: u16, name: &str| SyncOptions {
        roms,
        name: name.into(),
        listen,
        announce: SocketAddr::from(([127, 0, 0, 1], to)),
    };
    let (oa, ob) = (opts(pa, pb, "Brick"), opts(pb, pa, "RG35XX SP"));
    let (ra, rb) = (a.to_path_buf(), b.to_path_buf());
    let one = std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        run(&ra, &oa, &tx, &AtomicBool::new(false));
        rx.try_iter().collect::<Vec<_>>()
    });
    let two = std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        run(&rb, &ob, &tx, &AtomicBool::new(false));
        rx.try_iter().collect::<Vec<_>>()
    });
    (one.join().unwrap(), two.join().unwrap())
}

#[test]
fn two_devices_swap_saves_and_the_newer_copy_wins() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "Saves/GBA/Emerald.srm", b"emerald from A", 100);
    put(
        b.path(),
        "Saves/GBA/Emerald.srm",
        b"newer emerald from B",
        10,
    );
    put(a.path(), "Saves/GB/Tetris.srm", b"tetris only on A", 50);
    put(
        b.path(),
        "States/GBA/mgba/Fusion/resume.state",
        b"fusion state",
        20,
    );
    put(a.path(), "Config/slot.state", b"A's own settings", 5);
    put(a.path(), "Games/GBA/Fusion.gba", b"rom", 5);

    let (na, nb) = sync_pair(a.path(), b.path(), false);
    assert!(matches!(na.last(), Some(SyncNews::Done { .. })), "{na:?}");
    assert!(matches!(nb.last(), Some(SyncNews::Done { .. })), "{nb:?}");

    for root in [a.path(), b.path()] {
        assert_eq!(
            std::fs::read(root.join("Saves/GBA/Emerald.srm")).unwrap(),
            b"newer emerald from B"
        );
        assert_eq!(
            std::fs::read(root.join("Saves/GB/Tetris.srm")).unwrap(),
            b"tetris only on A"
        );
        assert_eq!(
            std::fs::read(root.join("States/GBA/mgba/Fusion/resume.state")).unwrap(),
            b"fusion state"
        );
    }
    assert!(
        !b.path().join("Config/slot.state").exists(),
        "device settings travelled"
    );
    assert!(
        !b.path().join("Games/GBA/Fusion.gba").exists(),
        "ROMs travelled when off"
    );
}

#[test]
fn roms_travel_when_asked() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "Games/GBA/Fusion.gba", b"rom bytes", 5);
    let (na, _) = sync_pair(a.path(), b.path(), true);
    assert_eq!(na.last(), Some(&SyncNews::Done { sent: 1, got: 0 }));
    assert_eq!(
        std::fs::read(b.path().join("Games/GBA/Fusion.gba")).unwrap(),
        b"rom bytes"
    );
}

#[test]
fn the_device_sync_screen_lives_under_network_and_toggles_roms() {
    use slot::app::{App, Phase};
    use slot_input::{Action, Btn};
    fn press(a: &mut App, btn: Btn) {
        a.apply(Action::GbaDown(btn));
        a.apply(Action::GbaUp(btn));
    }
    let d = common::tmp_root_with_carts(&["Fusion", "Zero Mission"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let (mut a, _) = common::app_booting_at(d.path(), common::CLOCK_IS_SET);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(slot_ui::QuickRow::Tools) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    while a.quick_menu() != Some(slot_ui::QuickRow::DeviceSync) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Sync { roms: false, .. }));
    let lines = a.settings_lines().expect("sync text");
    assert_eq!(lines[slot_ui::POPUP_TITLE_SLOT].0, "Device Sync");
    assert_eq!(lines[slot_ui::option_slot(2)].0, "ROMs   Off");
    press(&mut a, Btn::X);
    let lines = a.settings_lines().unwrap();
    assert_eq!(lines[slot_ui::option_slot(2)].0, "ROMs   On");
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(slot_ui::QuickRow::DeviceSync));
    assert!(!a.sync_running());
}
