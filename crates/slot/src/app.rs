use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use slot_gfx::{OUT_H, OUT_W};
use slot_input::{Action, Btn, MUTE_CHORD_MS};
use slot_power::{Battery, Charge, LedState, LidPolicy, Lights, LightsEffect, LightsMode, Power};
use slot_retro::LinkChannel;
use slot_store::{
    format_stamp, read_slot_state, scan, write_slot_state, Cart, Core, CpuMode, GbPalette,
    LightColour, LightEffect, LightLevel, LightMode, Look, Platform, SaveFormat, Shader, SlotState,
    StateEntry, StateRing, Theme, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, FF_SPEEDS, RING_MAX, VOLUME_MAX,
};
use slot_ui::{
    board_from, board_zoom, draw_backdrop, draw_empty_slot, draw_footer, draw_slot_name,
    draw_sticker, ease, grown, lid_at, lid_from, lift_of, on_board, shelf_cart_at, ClockPicker,
    Draw, FfState, GbShell, Hud, HudKind, Icon, LinkBadge, Millis, Placed, Polaroids, PowerChoice,
    QuickMenu, QuickMenuFaces, QuickRow, QuickValue, Refusal, Shelf, SlotChrome, TexId, Toast,
    BOARD_W, BOARD_X, CART_W, CHIP_H, CHIP_U, CHIP_V, CHIP_W, HINT_EDGE, HINT_H, HOP_LIFT,
    SHADOW_H, SHADOW_W, SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};

use crate::audio::Sfx;
use crate::core_picker::{Chip, CorePicker, Outcome, Press};
use crate::link_kind::{link_carried, link_kind, serial_option, LinkKind};
use crate::link_radio::{radio_jobs, LinkRole, RadioJob, RadioJobs};
use crate::link_screen::LinkSprites;
use crate::link_start::{link_port, LinkFail, LinkProgress, LinkStarter, LinkStep};
use crate::net::NetJob;
use crate::persist::{self, Snapshot};
use crate::video_mode::{self, VideoMode};

pub const INSERT_S: f32 = 0.73;
const INSERT_HOLD_S: f32 = 0.28;

pub const SEATED_AT: f32 = INSERT_S - INSERT_HOLD_S;

pub const EJECT_S: f32 = SEATED_AT;

const EJECT_HOLD_S: f32 = 0.35;

const POWER_ON_S: f32 = 0.22;

const POWER_OFF_S: f32 = 0.16;

const VOLUME_STEP: u8 = 5;
/// How often the Home panel re-reads the temperature, clock and memory.
const HOME_STATS_MS: Millis = 1000;
/// How long Restart waits for its second press.
const RESTART_CONFIRM_MS: Millis = 3000;

const AUTOSAVE_MS: Millis = 60_000;
const CARD_CHECK_MS: Millis = 2_000;

const BATTERY_CRITICAL: u8 = 5;

const BATTERY_POLL_MS: Millis = 10_000;

const CHARGE_POLL_MS: Millis = 1_000;
const HEADPHONES_POLL_MS: Millis = 500;

const BATTERY_LOW: u8 = 20;

pub const UNDO_GRACE_MS: Millis = 30_000;

pub const LINK_LOST_MS: Millis = 2000;

const PLAY_HOLD_MS: Millis = 500;

const SHUTDOWN_SHOW_MS: Millis = 250;

const POWER_MENU_PITCH: f32 = 44.0;
const POWER_MENU_BAR_INSET: f32 = 4.0;
const CORE_PICKER_RECEDE: f32 = 0.26;

const SLOT_NAME_IN_MS: Millis = 200;
const SLOT_NAME_HOLD_MS: Millis = 1200;
const SLOT_NAME_OUT_MS: Millis = 800;
const SLOT_NAME_ALPHA: f32 = 0.4;
const CORE_PICKER_DIM: f32 = 0.614;
const CORE_LEGEND_Y: f32 = 386.0;
const LINK_TEXT_Y: f32 = 44.0;
const LINK_LEGEND_Y: f32 = 422.0;
const LINK_LEGEND_GAP: f32 = 40.0;
const LID_SHADOW_W: f32 = 168.0;
const LID_SHADOW_H: f32 = 18.0;
const LID_SHADOW_DROP: f32 = 29.0;
const LID_SHADOW_ALPHA: f32 = 0.8;
const FACES_WAIT_MS: Millis = 1500;

const ALERT_HOLD: f32 = 0.45;
const ALERT_GONE: f32 = 0.9;

const CLOCK_FLOOR: i64 = 1_577_836_800;

pub enum PendingUndo {
    Save {
        stamp: String,
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    },
    Load {
        prior: Vec<u8>,
    },
}

struct LinkSession {
    client_id: u16,
    lost_at: Option<Millis>,
}

struct LinkStarting {
    starter: LinkStarter,
    client_id: u16,
}

struct Reload {
    stem: String,
    role: LinkRow,
    cancelled: bool,
    from: LinkKind,
    from_serial: &'static str,
    fallback: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameMenu {
    Pick(LinkRow),
    Working {
        role: LinkRow,
        step: LinkStep,
        since: Millis,
    },
    Linked {
        role: LinkRow,
        worked: Millis,
        since: Millis,
        opened: bool,
    },
    Failed {
        role: LinkRow,
        fail: LinkFail,
        worked: Millis,
        since: Millis,
    },
    Unplug {
        role: LinkRow,
        since: Millis,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkRow {
    Host,
    Join,
}

impl LinkRow {
    pub const ALL: [LinkRow; 2] = [LinkRow::Host, LinkRow::Join];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            LinkRow::Host => "Host",
            LinkRow::Join => "Join",
        }
    }

    pub fn role(self) -> LinkRole {
        match self {
            LinkRow::Host => LinkRole::Host,
            LinkRow::Join => LinkRole::Join,
        }
    }

    pub fn client_id(self) -> u16 {
        match self {
            LinkRow::Host => 0,
            LinkRow::Join => 1,
        }
    }

    pub fn other(self) -> LinkRow {
        match self {
            LinkRow::Host => LinkRow::Join,
            LinkRow::Join => LinkRow::Host,
        }
    }

    pub fn from_client_id(id: u16) -> LinkRow {
        if id == 0 {
            LinkRow::Host
        } else {
            LinkRow::Join
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkLegend {
    Cancel,
    Mode,
    Swap,
    Link,
    Ok,
    Back,
    EndLink,
}

impl LinkLegend {
    pub const ALL: [LinkLegend; 7] = [
        LinkLegend::Cancel,
        LinkLegend::Mode,
        LinkLegend::Swap,
        LinkLegend::Link,
        LinkLegend::Ok,
        LinkLegend::Back,
        LinkLegend::EndLink,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

pub const LINKED_HOLD_MS: Millis = 1000;

pub const UNPLUG_HOLD_MS: Millis = 420;

pub const PLAY_GAP_MS: Millis = 1000;

fn cpu_profile(mode: CpuMode) -> slot_power::CpuProfile {
    match mode {
        CpuMode::Stock => slot_power::CpuProfile::Stock,
        CpuMode::Saver => slot_power::CpuProfile::Saver,
        CpuMode::Performance => slot_power::CpuProfile::Performance,
    }
}

pub const SCREENSHOTS: &str = "Screenshots";

/// The power press that woke the handheld from deep sleep arrives too, and
/// its release can come well after the resume; up to its release (or this
/// long, if the release never shows) power presses are ignored, so it does
/// not put the handheld straight back to sleep.
pub const WAKE_GRACE_MS: Millis = 5000;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum CheevosLogin {
    Waiting,
    Sent,
    NoAccount,
}

pub const PLAY_FLUSH_MS: Millis = 60_000;

/// The Customize panel while it is up: where its cursor is, and the cart's
/// shell as it was, so an unchanged one is not written back.
#[derive(Copy, Clone, Debug)]
struct Customize {
    panel: slot_ui::CustomizePanel,
    before: Option<slot_store::ShellChoice>,
}

#[derive(Debug)]
struct SettingsModel {
    side: Option<usize>,
    on_side: bool,
    cards: Vec<(String, String, bool)>,
    focus: Option<usize>,
    opens: bool,
}

#[derive(Debug)]
pub enum Phase {
    SetClock {
        picker: ClockPicker,
        seed: i64,
        from_menu: bool,
    },
    Shelf,
    QuickMenu {
        row: QuickRow,
    },
    Inserting {
        cart: String,
        t: f32,
        core_ready: bool,
        resumed: bool,
        clean: bool,
    },
    Playing {
        cart: String,
    },
    Ejecting {
        cart: String,
        t: f32,
    },
    Polaroids {
        cart: String,
    },
    About,
    Doze {
        cart: Option<String>,
    },
    Search,
    /// The networks in range, to pick one to join.
    Networks {
        list: Vec<crate::net::Network>,
        at: usize,
        scanning: bool,
    },
    /// Typing the password for a network.
    Password {
        ssid: String,
        keys: crate::keyboard::Keyboard,
        list: Vec<crate::net::Network>,
    },
}

pub struct App {
    phase: Phase,
    shelves: Vec<(Platform, Shelf)>,
    shelf_at: usize,
    play_held: Option<Millis>,
    refusal: Option<Refusal>,
    refused_from: Option<f32>,
    alert_face: Option<TexId>,
    shutdown_faces: Vec<(TexId, u32, u32)>,
    power_menu: Option<usize>,
    power_menu_faces: Vec<(TexId, u32, u32)>,
    core_picker: Option<CorePicker>,
    core_board_face: Option<TexId>,
    core_lid_face: Option<TexId>,
    core_faces_stem: Option<String>,
    core_socket_faces: Vec<TexId>,
    core_chip_faces: Vec<TexId>,
    core_blank_chip_face: Option<TexId>,
    core_chip_shadow_face: Option<TexId>,
    core_legend_faces: Vec<(TexId, u32)>,
    game_menu: Option<GameMenu>,
    link_sprites: Option<LinkSprites>,
    link_hardware: LinkKind,
    last_role: LinkRow,
    link_choices: HashMap<String, LinkKind>,
    link_loaded: Option<&'static str>,
    link_reload: Option<(String, &'static str)>,
    reload: Option<Reload>,
    link_menu_faces: Vec<(TexId, u32, u32)>,
    link_linked_face: Option<(TexId, u32, u32)>,
    link_step_faces: Vec<(TexId, u32, u32)>,
    link_fail_faces: Vec<(TexId, u32, u32)>,
    link_legend_faces: Vec<(TexId, u32)>,
    starting: Option<LinkStarting>,
    link_transport: Option<(u16, Box<dyn LinkChannel>)>,
    restarting: bool,
    messages: std::collections::VecDeque<(String, String)>,
    search: crate::search::Search,
    search_faces: Vec<Option<(TexId, u32, u32)>>,
    search_picked_at: Millis,
    home: Option<slot_ui::HomePanel>,
    socket_backdrop: Option<TexId>,
    socket_chrome: Vec<(TexId, u32, u32)>,
    customize: Option<Customize>,
    core_popup: Option<usize>,
    resume_popup: Option<usize>,
    restart_armed_at: Option<Millis>,
    restart_after_eject: bool,
    dropped_faces: Vec<TexId>,
    shell_generation: u32,
    settings_faces: Vec<Option<(TexId, u32, u32)>>,
    socket_faces: Vec<Option<(TexId, u32, u32)>>,
    home_faces: Vec<Option<(TexId, u32, u32)>>,
    home_stats: crate::sysinfo::Stats,
    home_stats_at: Millis,
    net: crate::net::Net,
    cheevos_login: CheevosLogin,
    clock_asked: bool,
    file_server: Option<crate::fileserver::FileServer>,
    power_quiet_until: Millis,
    play_key: Option<String>,
    play_ms: Millis,
    play_tick: Option<Millis>,
    act_at: Millis,
    root: Option<PathBuf>,
    state: SlotState,
    vol_before: Vec<(u8, bool, Millis)>,
    snapshot: Option<Box<dyn Snapshot>>,
    core: Core,
    colour_pending: Option<bool>,
    palette_live: bool,
    palette_pending: Option<GbPalette>,
    link_player: Option<u8>,
    platform: Platform,
    named_core: bool,
    video_mode: VideoMode,
    link: Option<LinkSession>,
    sfx: Option<Sfx>,
    polaroids: Option<Polaroids>,
    pending: Option<(PendingUndo, Millis)>,
    legend_faces: Vec<TexId>,
    undo_face: Option<TexId>,
    clock_faces: Option<(TexId, TexId)>,
    quick_menu_faces: Option<QuickMenuFaces>,
    quick_clock_faces: Option<[(TexId, u32, u32); 2]>,
    sticker_face: Option<TexId>,
    wallpaper: Option<TexId>,
    battery_percent: slot_ui::Printed,
    bolt: Option<TexId>,
    shelf_platform: slot_ui::Printed,
    name_pending: bool,
    slot_letter: Option<char>,
    shelf_named: Option<Millis>,
    shelf_clock: slot_ui::Printed,
    hud: Hud,
    screen: f32,
    lift: f32,
    game_ready: bool,
    clock: f64,
    power: Option<Power>,
    dozed_at: Millis,
    woke_on_press: bool,
    autosave_at: Millis,
    pending_save: Option<std::thread::JoinHandle<()>>,
    card_failing: Arc<AtomicBool>,
    card_read_only: bool,
    card_check_at: Millis,
    battery_at: Millis,
    charge_at: Millis,
    headphones: bool,
    headphones_at: Millis,
    battery: Option<Battery>,
    last_led: Option<LedState>,
    powering_off: bool,
    radio: Box<dyn RadioJobs>,
    state_writer: crate::state_writer::StateWriter,
}

const FACE_AHEAD: i32 = 8;
const FACE_KEEP: i32 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FaceWant {
    pub shelf: usize,
    pub index: usize,
    pub on_screen: bool,
}

fn shelves_of(carts: Vec<Cart>) -> Vec<(Platform, Shelf)> {
    let mut rows: Vec<(Platform, Vec<Cart>)> =
        Platform::ALL.iter().map(|p| (*p, Vec::new())).collect();
    for cart in carts {
        if let Some((_, row)) = rows.iter_mut().find(|(p, _)| *p == cart.platform) {
            row.push(cart);
        }
    }
    rows.into_iter()
        .map(|(platform, carts)| (platform, Shelf::new(carts)))
        .collect()
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        let shelves = shelves_of(carts);
        let shelf_at = shelves
            .iter()
            .position(|(_, s)| !s.carts.is_empty())
            .unwrap_or(0);
        let state_writer = crate::state_writer::StateWriter::spawn();
        let card_failing = state_writer.card_failing();
        App {
            radio: radio_jobs(),
            state_writer,
            phase: Phase::Shelf,
            shelves,
            shelf_at,
            play_held: None,
            refusal: None,
            refused_from: None,
            alert_face: None,
            shutdown_faces: Vec::new(),
            power_menu: None,
            power_menu_faces: Vec::new(),
            core_picker: None,
            core_board_face: None,
            core_lid_face: None,
            core_faces_stem: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_chip_shadow_face: None,
            core_legend_faces: Vec::new(),
            game_menu: None,
            link_sprites: None,
            link_hardware: LinkKind::Cable,
            last_role: LinkRow::Host,
            link_choices: HashMap::new(),
            link_loaded: None,
            link_reload: None,
            reload: None,
            link_menu_faces: Vec::new(),
            link_linked_face: None,
            link_step_faces: Vec::new(),
            link_fail_faces: Vec::new(),
            link_legend_faces: Vec::new(),
            starting: None,
            link_transport: None,
            restarting: false,
            messages: std::collections::VecDeque::new(),
            search: crate::search::Search::default(),
            search_faces: Vec::new(),
            search_picked_at: 0,
            home: None,
            socket_backdrop: None,
            socket_chrome: Vec::new(),
            customize: None,
            core_popup: None,
            resume_popup: None,
            restart_armed_at: None,
            restart_after_eject: false,
            dropped_faces: Vec::new(),
            shell_generation: 0,
            settings_faces: Vec::new(),
            socket_faces: Vec::new(),
            home_faces: Vec::new(),
            home_stats: crate::sysinfo::Stats::default(),
            home_stats_at: 0,
            net: crate::net::Net::idle(),
            cheevos_login: CheevosLogin::Waiting,
            clock_asked: false,
            file_server: None,
            power_quiet_until: 0,
            play_key: None,
            play_ms: 0,
            play_tick: None,
            act_at: 0,
            root: None,
            state: SlotState::default(),
            vol_before: Vec::new(),
            snapshot: None,
            core: Core::default(),
            colour_pending: None,
            palette_live: false,
            palette_pending: None,
            link_player: None,
            platform: Platform::default(),
            named_core: false,
            video_mode: VideoMode::default(),
            link: None,
            sfx: None,
            polaroids: None,
            pending: None,
            legend_faces: Vec::new(),
            undo_face: None,
            clock_faces: None,
            quick_menu_faces: None,
            quick_clock_faces: None,
            sticker_face: None,
            wallpaper: None,
            battery_percent: slot_ui::Printed::default(),
            bolt: None,
            shelf_platform: slot_ui::Printed::default(),
            name_pending: false,
            slot_letter: None,
            shelf_named: None,
            shelf_clock: slot_ui::Printed::default(),
            hud: Hud::new(),
            screen: 0.0,
            lift: 1.0,
            game_ready: false,
            clock: 0.0,
            power: None,
            dozed_at: 0,
            woke_on_press: false,
            autosave_at: AUTOSAVE_MS,
            pending_save: None,
            card_failing,
            card_read_only: false,
            card_check_at: 0,
            battery_at: BATTERY_POLL_MS,
            charge_at: CHARGE_POLL_MS,
            headphones: false,
            headphones_at: 0,
            battery: None,
            last_led: None,
            powering_off: false,
        }
    }

    pub fn boot(root: &Path) -> Self {
        crate::root::ensure(root);
        slot_ui::set_theme(Theme::read(root));
        crate::boot_time::mark("root and theme");
        let carts = scan(root).unwrap_or_default();
        crate::boot_time::mark("scan");
        let mut app = App::new(carts);
        app.root = Some(root.to_path_buf());
        app.state = read_slot_state(root);
        app.net = crate::net::Net::spawn(root);
        if app.state.syncthing {
            app.net.send(NetJob::SyncthingStart);
        } else if app.state.wifi {
            app.net.send(NetJob::WifiUp);
        }
        crate::cheevos::enable(app.state.achievements);
        if app.state.clock_set {
            app.start();
        } else {
            app.phase = clock_screen(system_secs(), 0, false);
        }
        app
    }

    fn start(&mut self) {
        let seated = if self.single_cart() {
            Some((self.shelf_at, 0))
        } else {
            let stem = self.state.cart.clone();
            let platform = self.state.cart_platform;
            stem.and_then(|stem| self.seat_of(&stem, platform))
        };
        self.phase = Phase::Shelf;
        match seated {
            Some((at, i)) => {
                self.shelf_at = at;
                self.shelf_mut().select(i);
                self.insert(false);
                if let Phase::Inserting { resumed, t, .. } = &mut self.phase {
                    *resumed = true;
                    *t = INSERT_S;
                }
            }
            None => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.name_pending = true;
            }
        }
    }

    fn shelf(&self) -> &Shelf {
        &self.shelves[self.shelf_at].1
    }

    fn shelf_mut(&mut self) -> &mut Shelf {
        &mut self.shelves[self.shelf_at].1
    }

    fn switch_shelf(&mut self, by: i32) {
        let Some(to) = self.next_shelf(by) else {
            return;
        };
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.shelf_at = to;
        self.shelf_platform = slot_ui::Printed::default();
        self.slot_letter = None;
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn jump_letter(&mut self, dir: i32) {
        let before = self.shelf().index;
        match dir > 0 {
            true => self.shelf_mut().jump_next_letter(),
            false => self.shelf_mut().jump_prev_letter(),
        }
        if self.shelf().index == before {
            return;
        }
        let Some(letter) = self.selected_stem().map(slot_store::initial) else {
            return;
        };
        if self.slot_letter != Some(letter) {
            self.shelf_platform = slot_ui::Printed::default();
        }
        self.slot_letter = Some(letter);
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn slot_name_alpha(&self) -> f32 {
        let Some(at) = self.shelf_named else {
            return 0.0;
        };
        let t = self.now().saturating_sub(at);
        let level = if t < SLOT_NAME_IN_MS {
            t as f32 / SLOT_NAME_IN_MS as f32
        } else if t < SLOT_NAME_IN_MS + SLOT_NAME_HOLD_MS {
            1.0
        } else {
            let out = t - SLOT_NAME_IN_MS - SLOT_NAME_HOLD_MS;
            1.0 - (out as f32 / SLOT_NAME_OUT_MS as f32).min(1.0)
        };
        SLOT_NAME_ALPHA * ease(level)
    }

    fn next_shelf(&self, by: i32) -> Option<usize> {
        let n = self.shelves.len() as i32;
        (1..n)
            .map(|step| (self.shelf_at as i32 + by * step).rem_euclid(n) as usize)
            .find(|at| !self.shelves[*at].1.carts.is_empty())
    }

    fn seat_of(&self, stem: &str, platform: Option<Platform>) -> Option<(usize, usize)> {
        self.shelves
            .iter()
            .enumerate()
            .filter(|(_, (p, _))| platform.is_none_or(|want| *p == want))
            .find_map(|(at, (_, shelf))| {
                shelf
                    .carts
                    .iter()
                    .position(|c| c.stem == stem)
                    .map(|i| (at, i))
            })
    }

    pub fn confirm_clock(&mut self) {
        let Phase::SetClock {
            picker,
            seed,
            from_menu,
        } = &self.phase
        else {
            return;
        };
        let (moved, offset, from_menu) = (picker.secs() - *seed, picker.offset_min(), *from_menu);
        let utc = self.utc_secs() + moved;
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
        }
        self.state.utc_offset_min = offset as i16;
        self.state.clock_set = true;
        self.persist();
        if from_menu {
            self.phase = Phase::QuickMenu {
                row: QuickRow::DateTime,
            };
        } else {
            self.start();
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn wall_secs(&self) -> i64 {
        self.utc_secs() + i64::from(self.state.utc_offset_min) * 60
    }

    fn utc_secs(&self) -> i64 {
        self.power.as_ref().map_or_else(system_secs, |p| p.now())
    }

    pub fn picker(&self) -> Option<&ClockPicker> {
        match &self.phase {
            Phase::SetClock { picker, .. } => Some(picker),
            _ => None,
        }
    }

    pub fn quick_menu(&self) -> Option<QuickRow> {
        match self.phase {
            Phase::QuickMenu { row } => Some(row),
            _ => None,
        }
    }

    pub fn quick_value(&self, row: QuickRow) -> Option<QuickValue> {
        match row {
            QuickRow::FastForward => QuickValue::speed(self.state.ff_speed),
            QuickRow::FastForwardSound => Some(QuickValue::flag(self.state.ff_sound)),
            QuickRow::ColourCorrection => Some(QuickValue::flag(self.state.colour_correction)),
            QuickRow::GbPalettes => Some(QuickValue::flag(self.state.gb_palettes)),
            QuickRow::GbaShader => Some(shader_value(self.state.shader_gba)),
            QuickRow::GbShader => Some(shader_value(self.state.shader_gb)),
            QuickRow::Rumble => Some(QuickValue::flag(self.state.rumble)),
            QuickRow::EjectSave => Some(QuickValue::flag(self.state.eject_save)),
            QuickRow::AskResume => Some(QuickValue::flag(self.state.ask_resume)),
            QuickRow::Turbo => Some(QuickValue::flag(self.state.turbo)),
            QuickRow::Rewind => Some(QuickValue::flag(self.state.rewind)),
            QuickRow::Brightness => Some(QuickValue::level(self.state.brightness)),
            QuickRow::Warmth => Some(QuickValue::level(self.state.blue_light)),
            QuickRow::Wifi => Some(QuickValue::flag(self.state.wifi)),
            QuickRow::Syncthing => Some(QuickValue::flag(self.state.syncthing)),
            QuickRow::Achievements => Some(QuickValue::flag(self.state.achievements)),
            QuickRow::AutoTime => Some(QuickValue::flag(self.state.auto_time)),
            QuickRow::FileTransfer => Some(QuickValue::flag(self.file_server.is_some())),
            QuickRow::Performance => Some(match self.state.cpu {
                CpuMode::Stock => QuickValue::CpuStock,
                CpuMode::Saver => QuickValue::CpuSaver,
                CpuMode::Performance => QuickValue::CpuPerformance,
            }),
            QuickRow::ClockFormat => Some(match self.state.clock_12h {
                true => QuickValue::Hour12,
                false => QuickValue::Hour24,
            }),
            QuickRow::Look => Some(match self.state.look {
                Look::Classic => QuickValue::LookClassic,
                Look::Socket => QuickValue::LookSocket,
            }),
            QuickRow::SaveFormat => Some(match self.state.save_format {
                SaveFormat::RetroArch => QuickValue::RetroArch,
                SaveFormat::Slot => QuickValue::Slot,
            }),
            QuickRow::LightMode => Some(match self.state.light_mode {
                LightMode::Battery => QuickValue::Battery,
                LightMode::Colour => QuickValue::Colour,
                LightMode::Off => QuickValue::Off,
            }),
            QuickRow::LightColour => Some(match self.state.light_colour {
                LightColour::White => QuickValue::White,
                LightColour::Red => QuickValue::Red,
                LightColour::Orange => QuickValue::Orange,
                LightColour::Yellow => QuickValue::Yellow,
                LightColour::Green => QuickValue::Green,
                LightColour::Cyan => QuickValue::Cyan,
                LightColour::Blue => QuickValue::Blue,
                LightColour::Purple => QuickValue::Purple,
                LightColour::Pink => QuickValue::Pink,
            }),
            QuickRow::LightEffect => Some(match self.state.light_effect {
                LightEffect::Static => QuickValue::Static,
                LightEffect::Breathe => QuickValue::Breathe,
                LightEffect::Blink => QuickValue::Blink,
                LightEffect::Rainbow => QuickValue::Rainbow,
                LightEffect::RainbowWave => QuickValue::RainbowWave,
                LightEffect::Fire => QuickValue::Fire,
                LightEffect::Ocean => QuickValue::Ocean,
                LightEffect::Aurora => QuickValue::Aurora,
                LightEffect::Sunset => QuickValue::Sunset,
                LightEffect::Candy => QuickValue::Candy,
                LightEffect::Police => QuickValue::Police,
                LightEffect::Heartbeat => QuickValue::Heartbeat,
            }),
            QuickRow::LightLevel => Some(match self.state.light_level {
                LightLevel::Low => QuickValue::Low,
                LightLevel::Medium => QuickValue::Medium,
                LightLevel::High => QuickValue::High,
            }),
            QuickRow::DateTime
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::Lights
            | QuickRow::Network
            | QuickRow::Search
            | QuickRow::Labels
            | QuickRow::JoinNetwork => None,
        }
    }

    pub fn set_quick_menu_faces(&mut self, faces: QuickMenuFaces) {
        self.quick_menu_faces = Some(faces);
    }

    pub fn set_quick_clock_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_clock_faces = Some([dim, lit]);
    }

    pub fn set_sticker_face(&mut self, face: TexId) {
        self.sticker_face = Some(face);
    }

    pub fn set_clock_faces(&mut self, line: TexId, hint: TexId) {
        self.clock_faces = Some((line, hint));
    }

    pub fn set_cart_shadow(&mut self, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_shadow(face);
        }
    }

    pub fn set_gb_cart_shadows(&mut self, notched: TexId, rounded: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_gb_shadow(GbShell::Notched, notched);
            shelf.set_gb_shadow(GbShell::Rounded, rounded);
        }
    }

    pub fn set_wallpaper(&mut self, face: TexId) {
        self.wallpaper = Some(face);
    }

    pub fn set_bolt_face(&mut self, bolt: TexId) {
        self.bolt = Some(bolt);
    }

    pub fn set_shelf_platform_face(&mut self, face: TexId, w: u32) {
        self.shelf_platform = slot_ui::Printed::new(face, w);
    }

    pub fn clear_shelf_platform(&mut self) {
        self.shelf_platform = slot_ui::Printed::default();
    }

    pub fn slot_text(&self) -> Option<String> {
        match self.slot_letter {
            Some(letter) => Some(letter.to_string()),
            None => self.shelf_platform_name().map(str::to_string),
        }
    }

    pub fn shelf_platform_name(&self) -> Option<&'static str> {
        self.next_shelf(1)?;
        Some(self.shelves[self.shelf_at].0.name())
    }

    pub fn set_battery_percent_face(&mut self, face: TexId, w: u32) {
        self.battery_percent = slot_ui::Printed::new(face, w);
    }

    pub fn set_shelf_clock_face(&mut self, face: TexId, w: u32) {
        self.shelf_clock = slot_ui::Printed::new(face, w);
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn carts(&self) -> impl Iterator<Item = &Cart> {
        self.shelves
            .iter()
            .flat_map(|(_, shelf)| shelf.carts.iter())
    }

    pub fn seated_cart(&self) -> Option<&Cart> {
        let stem = match &self.phase {
            Phase::Inserting { cart, .. }
            | Phase::Playing { cart }
            | Phase::Ejecting { cart, .. }
            | Phase::Polaroids { cart } => cart,
            Phase::Doze { cart: Some(cart) } => cart,
            _ => return None,
        };
        self.shelf().carts.iter().find(|c| c.stem == *stem)
    }

    pub fn single_cart(&self) -> bool {
        self.carts().count() == 1
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        let mut faces = faces.into_iter();
        for (_, shelf) in &mut self.shelves {
            let n = shelf.carts.len();
            shelf.set_faces(faces.by_ref().take(n).collect());
        }
    }

    pub fn face_wants(&self) -> Vec<FaceWant> {
        let here = self.shelf();
        let screen = here.on_screen();
        let mut wants: Vec<(usize, usize)> = screen.iter().map(|i| (self.shelf_at, *i)).collect();
        for at in self.neighbour_shelves() {
            wants.extend(self.shelves[at].1.on_screen().into_iter().map(|i| (at, i)));
        }
        wants.extend(
            here.around(FACE_AHEAD)
                .into_iter()
                .filter(|i| !screen.contains(i))
                .map(|i| (self.shelf_at, i)),
        );
        wants
            .into_iter()
            .filter(|(at, i)| self.shelves[*at].1.face(*i).is_none())
            .map(|(shelf, index)| FaceWant {
                shelf,
                index,
                on_screen: shelf == self.shelf_at && screen.contains(&index),
            })
            .collect()
    }

    pub fn face_cart(&self, shelf: usize, index: usize) -> Option<&Cart> {
        self.shelves.get(shelf)?.1.carts.get(index)
    }

    pub fn set_cart_face(&mut self, shelf: usize, index: usize, tex: TexId) -> Option<TexId> {
        self.shelves.get_mut(shelf)?.1.set_face(index, tex)
    }

    pub fn shed_faces(&mut self) -> Vec<TexId> {
        let neighbours = self.neighbour_shelves();
        let mut shed = std::mem::take(&mut self.dropped_faces);
        for (at, (_, shelf)) in self.shelves.iter_mut().enumerate() {
            let keep = if at == self.shelf_at {
                shelf.around(FACE_KEEP)
            } else if neighbours.contains(&at) {
                shelf.on_screen()
            } else {
                Vec::new()
            };
            for i in 0..shelf.carts.len() {
                if !keep.contains(&i) {
                    shed.extend(shelf.take_face(i));
                }
            }
        }
        shed
    }

    fn neighbour_shelves(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for by in [1, -1] {
            if let Some(at) = self.next_shelf(by) {
                if !out.contains(&at) {
                    out.push(at);
                }
            }
        }
        out
    }

    pub fn set_snapshot(&mut self, snapshot: Box<dyn Snapshot>) {
        self.snapshot = Some(snapshot);
    }

    pub fn set_core(&mut self, core: Core) {
        self.core = core;
    }

    pub fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    pub fn set_named_core(&mut self, named: bool) {
        self.named_core = named;
    }

    pub fn set_video_mode(&mut self, mode: VideoMode) {
        self.video_mode = mode;
    }

    pub fn source_rect(&self) -> [f32; 4] {
        video_mode::source_rect(self.platform, self.video_mode)
    }

    fn slot_owns_the_shoulders(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. }) && self.platform != Platform::Gba
    }

    pub fn taken_buttons(&self) -> &'static [Btn] {
        if self.slot_owns_the_shoulders() {
            &[Btn::L1, Btn::R1]
        } else {
            &[]
        }
    }

    pub fn takes_from_the_game(&self, action: Action) -> bool {
        match action {
            Action::GbaDown(btn) | Action::GbaUp(btn) => self.taken_buttons().contains(&btn),
            _ => false,
        }
    }

    fn set_picture(&mut self, mode: VideoMode) {
        if self.video_mode == mode {
            return;
        }
        self.video_mode = mode;
        let (Some(root), Phase::Playing { cart }) = (self.root.clone(), &self.phase) else {
            return;
        };
        let written = video_mode::write_video_mode(&root, cart, mode);
        if let Err(e) = &written {
            eprintln!("slot: video: could not write video_mode.txt: {e}");
        }
        self.card_failing.store(written.is_err(), Ordering::Relaxed);
    }

    pub fn set_link_loaded(&mut self, serial: &'static str) {
        self.link_loaded = Some(serial);
    }

    pub fn link_mode(&self, stem: &str) -> (LinkKind, &'static str) {
        let Some((cart, auto)) = self.auto_link(stem) else {
            return (LinkKind::Cable, "auto");
        };
        let chosen = self.link_choices.get(stem).copied().unwrap_or(auto);
        (chosen, serial_option(chosen, auto, &cart.code, &cart.title))
    }

    pub fn link_active(&self) -> bool {
        self.link.is_some()
    }

    pub fn begin_link(&mut self, client_id: u16) {
        self.link = Some(LinkSession {
            client_id,
            lost_at: None,
        });
        self.sync_link_badge();
    }

    pub fn link_client_id(&self) -> Option<u16> {
        self.link.as_ref().map(|s| s.client_id)
    }

    pub fn may_rewind(&self) -> bool {
        self.state.rewind && !self.link_active()
    }

    pub fn turbo(&self) -> bool {
        self.state.turbo
    }

    pub fn records_rewind(&self) -> bool {
        self.state.rewind
    }

    pub fn picture_rect(&self) -> [f32; 4] {
        let (w, h) = self.platform.picture();
        let (sw, sh) = (slot_gfx::SRC_W as f32, slot_gfx::SRC_H as f32);
        let (x, y) = ((sw - w as f32) / 2.0, (sh - h as f32) / 2.0);
        [x / sw, y / sh, (x + w as f32) / sw, (y + h as f32) / sh]
    }

    pub fn may_load_state(&self) -> bool {
        !self.link_active()
    }

    pub fn may_fast_forward(&self) -> bool {
        !self.link_active()
    }

    pub fn end_link(&mut self) {
        self.link = None;
        self.link_player = None;
        self.sync_link_badge();
        self.radio.ask(RadioJob::Down);
        self.radio.ask(RadioJob::Cool);
    }

    pub fn peer_lost(&mut self) {
        if matches!(self.game_menu, Some(GameMenu::Linked { .. })) {
            self.game_menu = None;
        }
        let now = self.now();
        if let Some(session) = &mut self.link {
            session.lost_at.get_or_insert(now);
        }
        self.sync_link_badge();
    }

    pub fn bios_mismatch(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::BiosMismatch, self.now());
        self.unplug(role);
    }

    pub fn peer_ended(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::PeerEnded, self.now());
        self.unplug(role);
    }

    pub fn link_badge(&self) -> LinkBadge {
        self.hud.link()
    }

    pub fn set_link_badge_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_link_faces(faces);
    }

    fn sync_link_badge(&mut self) {
        let badge = match &self.link {
            None => LinkBadge::Off,
            Some(s) => match (s.client_id == 0, s.lost_at.is_some()) {
                (true, false) => LinkBadge::Hosting,
                (false, false) => LinkBadge::Joined,
                (true, true) => LinkBadge::HostingLost,
                (false, true) => LinkBadge::JoinedLost,
            },
        };
        self.hud.set_link(badge);
    }

    pub fn take_sfx(&mut self) -> Option<Sfx> {
        self.sfx.take()
    }

    pub fn set_power(&mut self, mut power: Power) {
        power.set_backlight(self.state.brightness);
        self.headphones = power.headphones();
        self.hud.set_headphones(self.headphones);
        let secs = power.now();
        if matches!(self.phase, Phase::SetClock { .. }) || secs < CLOCK_FLOOR {
            self.phase = clock_screen(secs, 0, false);
        }
        power.set_lights(self.lights());
        if self.state.cpu != CpuMode::Stock {
            power.set_cpu(cpu_profile(self.state.cpu));
        }
        self.power = Some(power);
        self.battery_at = self.now();
        self.charge_at = self.now();
        self.last_led = None;
    }

    pub fn lights(&self) -> Lights {
        let s = &self.state;
        Lights {
            mode: match s.light_mode {
                LightMode::Battery => LightsMode::Battery,
                LightMode::Colour => LightsMode::Colour,
                LightMode::Off => LightsMode::Off,
            },
            rgb: s.light_colour.rgb(),
            effect: match s.light_effect {
                LightEffect::Static => LightsEffect::Static,
                LightEffect::Breathe => LightsEffect::Breathe,
                LightEffect::Blink => LightsEffect::Blink,
                LightEffect::Rainbow => LightsEffect::Rainbow,
                LightEffect::RainbowWave => LightsEffect::RainbowWave,
                LightEffect::Fire => LightsEffect::Fire,
                LightEffect::Ocean => LightsEffect::Ocean,
                LightEffect::Aurora => LightsEffect::Aurora,
                LightEffect::Sunset => LightsEffect::Sunset,
                LightEffect::Candy => LightsEffect::Candy,
                LightEffect::Police => LightsEffect::Police,
                LightEffect::Heartbeat => LightsEffect::Heartbeat,
            },
            percent: s.light_level.percent(),
        }
    }

    pub fn set_rumble(&mut self, strength: u16) {
        if let Some(power) = &mut self.power {
            power.set_rumble(strength);
        }
    }

    pub fn rumble_enabled(&self) -> bool {
        self.state.rumble
    }

    pub fn ff_speed(&self) -> u8 {
        self.state.ff_speed
    }

    pub fn ff_sound(&self) -> bool {
        self.state.ff_sound
    }

    pub fn screen_shader(&self) -> Shader {
        match self.platform {
            Platform::Gba => self.state.shader_gba,
            Platform::Gb | Platform::Gbc => self.state.shader_gb,
        }
    }

    pub fn colour_correction(&self) -> bool {
        self.state.colour_correction
    }

    pub fn powering_off(&self) -> bool {
        self.powering_off
    }

    pub fn ready_to_power_off(&self) -> bool {
        self.powering_off && self.now() >= self.act_at
    }

    pub fn ready_to_restart(&self) -> bool {
        self.restarting && self.now() >= self.act_at
    }

    pub fn shutting_down(&self) -> bool {
        self.powering_off || self.restarting
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn restart(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.restart();
        }
    }

    pub fn power_menu(&self) -> Option<usize> {
        self.power_menu
    }

    pub fn core_picker(&self) -> Option<Core> {
        self.core_picker.map(|p| p.seat())
    }

    pub fn core_picker_chip(&self) -> Option<Chip> {
        self.core_picker.map(|p| p.chip(self.now()))
    }

    pub fn game_menu_open(&self) -> bool {
        self.game_menu.is_some()
    }

    pub fn game_menu(&self) -> Option<GameMenu> {
        self.game_menu
    }

    pub fn take_link_transport(&mut self) -> Option<(u16, Box<dyn LinkChannel>)> {
        self.link_transport.take()
    }

    pub fn take_link_reload(&mut self) -> Option<(String, &'static str)> {
        self.link_reload.take()
    }

    pub fn link_player(&self) -> Option<u8> {
        self.link_player
    }

    pub fn take_colour_correction(&mut self) -> Option<bool> {
        self.colour_pending.take()
    }

    pub fn gb_palette(&self) -> Option<GbPalette> {
        self.state.gb_palettes.then_some(self.state.gb_palette)
    }

    pub fn set_palette_live(&mut self, live: bool) {
        self.palette_live = live;
    }

    pub fn palette_live(&self) -> bool {
        self.palette_live
    }

    pub fn take_gb_palette(&mut self) -> Option<GbPalette> {
        self.palette_pending.take()
    }

    pub fn core(&self) -> Core {
        self.core
    }

    pub fn link_reload_done(&mut self) {
        let Some(reload) = self.reload.take() else {
            return;
        };
        if reload.fallback {
            self.link_choices.insert(reload.stem, reload.from);
            self.close_game_menu();
            return self.refuse();
        }
        if reload.cancelled {
            return self.close_game_menu();
        }
        let since = match self.game_menu {
            Some(GameMenu::Working { since, .. }) => since,
            _ => self.now(),
        };
        self.start_link_from(
            LinkStarter::spawn(reload.role.role(), link_port()),
            reload.role.client_id(),
            since,
        );
    }

    pub fn link_reload_failed(&mut self) {
        let Some(mut reload) = self.reload.take() else {
            return;
        };
        if !reload.fallback {
            self.link_reload = Some((reload.stem.clone(), reload.from_serial));
            reload.fallback = true;
            self.reload = Some(reload);
            return;
        }
        self.link_choices.insert(reload.stem, reload.from);
        self.refuse_seated();
    }

    pub fn selected_stem(&self) -> Option<&str> {
        self.shelf()
            .carts
            .get(self.shelf().index)
            .map(|c| c.stem.as_str())
    }

    pub fn battery(&self) -> Option<Battery> {
        self.battery
    }

    pub fn poweroff(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.poweroff();
        }
    }

    pub fn apply(&mut self, action: Action) {
        let power_key = matches!(
            action,
            Action::PowerPress | Action::PowerTap | Action::PowerHold | Action::PowerOff
        );
        if power_key && self.now() < self.power_quiet_until {
            if matches!(action, Action::PowerTap | Action::PowerOff) {
                self.power_quiet_until = 0;
            }
            return;
        }
        if self.power_menu.is_some() {
            match action {
                Action::LidClose => {
                    eprintln!("slot: lid closed");
                    return self.doze();
                }
                Action::LidOpen => {
                    eprintln!("slot: lid opened");
                    return self.wake();
                }
                _ => return self.power_menu_input(action),
            }
        }
        match action {
            Action::LidClose => {
                eprintln!("slot: lid closed");
                return self.doze();
            }
            Action::LidOpen => {
                eprintln!("slot: lid opened");
                return self.wake();
            }
            Action::PowerPress => {
                if self.link_active() {
                    self.end_link();
                    return;
                }
                self.woke_on_press = matches!(self.phase, Phase::Doze { .. });
                if self.woke_on_press {
                    self.wake();
                }
                return self.flush_resume();
            }
            Action::PowerTap => return self.power_press(),
            Action::PowerHold => return self.open_power_menu(),
            Action::PowerOff => return,
            _ => {}
        }
        if self.home.is_some() {
            if self.adjust(action) {
                return;
            }
            return self.home_input(action);
        }
        if self.customize.is_some() {
            if self.adjust(action) {
                return;
            }
            return self.customize_input(action);
        }
        if self.core_popup.is_some() {
            if self.adjust(action) {
                return;
            }
            return self.core_popup_input(action);
        }
        if self.resume_popup.is_some() {
            if self.adjust(action) {
                return;
            }
            return self.resume_popup_input(action);
        }
        if let Phase::SetClock {
            picker, from_menu, ..
        } = &mut self.phase
        {
            match action {
                Action::GbaDown(Btn::Left) | Action::ShelfLeft => picker.left(),
                Action::GbaDown(Btn::Right) | Action::ShelfRight => picker.right(),
                Action::GbaDown(Btn::Up) => picker.up(),
                Action::GbaDown(Btn::Down) => picker.down(),
                Action::GbaDown(Btn::A) | Action::Insert => self.confirm_clock(),
                Action::GbaDown(Btn::B) if *from_menu => {
                    self.phase = Phase::QuickMenu {
                        row: QuickRow::DateTime,
                    }
                }
                _ => {}
            }
            return;
        }
        if self.adjust(action) {
            return;
        }
        match action {
            Action::GbaUp(Btn::Left) => self.shelf_mut().release_left(),
            Action::GbaUp(Btn::Right) => self.shelf_mut().release_right(),
            _ => {}
        }
        if self.game_menu.is_some() {
            return self.game_menu_input(action);
        }
        let now = self.now();
        match self.phase {
            Phase::Shelf => match action {
                Action::GbaDown(Btn::Start) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::GbaDown(Btn::Up) => self.jump_letter(-1),
                Action::GbaDown(Btn::Down) => self.jump_letter(1),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf_mut().hold_left(now),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => {
                    self.shelf_mut().hold_right(now)
                }
                Action::QuickMenu => self.open_quick_menu(),
                Action::GbaDown(Btn::A) => self.play_held = Some(now),
                Action::GbaUp(Btn::A) => {
                    if self.play_held.take().is_some() {
                        self.insert_or_ask();
                    }
                }
                Action::Insert => self.insert_or_ask(),
                Action::GbaDown(Btn::L1) => self.switch_shelf(-1),
                Action::GbaDown(Btn::R1) => self.switch_shelf(1),
                Action::GbaDown(Btn::Y) => self.open_search(),
                Action::GbaDown(Btn::X) => self.open_customize(),
                Action::GbaDown(Btn::Home) | Action::HomePanel => self.open_home(),
                _ => {}
            },
            Phase::Search => self.search_input(action),
            Phase::Networks { .. } => self.networks_input(action),
            Phase::Password { .. } => self.password_input(action),
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::GbaDown(Btn::L1) if self.slot_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Stretch)
                }
                Action::GbaDown(Btn::R1) if self.slot_owns_the_shoulders() => {
                    self.set_picture(VideoMode::Actual)
                }
                Action::Eject => self.eject(),
                Action::GameMenu => self.game_menu_shortcut(),
                Action::Polaroids => self.open_polaroids(),
                Action::SaveState => self.save_state(),
                Action::LoadState => self.load_newest(),
                Action::Screenshot => self.screenshot(),
                Action::GbaDown(Btn::Home) | Action::HomePanel => self.open_home(),
                Action::RewindStart if !self.may_rewind() => self.refuse(),
                Action::FfStart if !self.may_fast_forward() => self.refuse(),
                _ => {}
            },
            Phase::Polaroids { .. } => match action {
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.flick(Polaroids::left),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.flick(Polaroids::right),
                Action::GbaDown(Btn::A) => self.load_selected(),
                Action::GbaDown(Btn::B) | Action::Polaroids => self.close_polaroids(),
                Action::GbaDown(Btn::X) => self.undo(self.now()),
                Action::GbaDown(Btn::Y) => self.delete_selected(),
                _ => {}
            },
            Phase::About if action == Action::GbaDown(Btn::B) || action == Action::QuickMenu => {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::About,
                }
            }
            Phase::QuickMenu { row } => self.quick_menu_input(row, action),
            _ => {}
        }
    }

    fn toggle_file_transfer(&mut self) {
        if self.file_server.take().is_some() {
            return self.say("File transfer off", "");
        }
        if !self.net.online() {
            return self.say("Turn on Wi-Fi first", "Network > Wi-Fi");
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        match crate::fileserver::FileServer::start(&root, crate::fileserver::PORT) {
            Ok(server) => {
                self.net.send(NetJob::Announce {
                    title: "File transfer on".into(),
                    port: server.port,
                });
                self.file_server = Some(server);
            }
            Err(e) => self.say("File transfer would not start", e.to_string()),
        }
    }

    fn download_labels(&mut self) {
        if !self.net.online() {
            return self.say("Turn on Wi-Fi first", "Network > Wi-Fi");
        }
        let Some(root) = self.root.clone() else {
            return;
        };
        let wanted: Vec<crate::labels::Wanted> = self
            .shelves
            .iter()
            .flat_map(|(_, s)| s.carts.iter())
            .filter(|c| c.label.is_none())
            .map(|c| crate::labels::Wanted {
                platform: c.platform,
                stem: c.stem.clone(),
                dest: root
                    .join("Labels")
                    .join(c.platform.dir_name())
                    .join(format!("{}.png", c.stem)),
            })
            .collect();
        if wanted.is_empty() {
            return self.say("Every game has a label", "");
        }
        self.say(
            "Downloading labels",
            format!("{} games without one", wanted.len()),
        );
        self.net.send(NetJob::Labels(wanted));
    }

    pub fn socket_look(&self) -> bool {
        self.state.look == Look::Socket
    }

    fn socket_screen(&self, fade: f32) -> slot_ui::SocketScreen<'_> {
        slot_ui::SocketScreen {
            backdrop: self.socket_backdrop,
            faces: &self.socket_faces,
            fade,
        }
    }

    pub fn wants_socket_chrome(&self) -> bool {
        (self.socket_look()
            || self.customize.is_some()
            || self.core_popup.is_some()
            || self.resume_popup.is_some()
            || matches!(self.phase, Phase::Networks { .. }))
            && self.socket_chrome.is_empty()
    }

    pub fn set_socket_chrome(&mut self, chrome: Vec<(TexId, u32, u32)>) {
        self.socket_chrome = chrome;
    }

    pub fn set_settings_faces(&mut self, faces: Vec<Option<(TexId, u32, u32)>>) {
        self.settings_faces = faces;
    }

    /// The settings as Socket lays them out for `row`: which category is lit
    /// and whether the cursor is on it, and the cards on the right with the
    /// first one showing and the one under the cursor.
    fn settings_model(&self, row: QuickRow) -> SettingsModel {
        let main = row.parent().unwrap_or(row);
        let on_side = row.parent().is_none();
        let (page, first, focus): (&[QuickRow], usize, Option<usize>) = match on_side {
            true => match main.child() {
                Some(child) => (child.page(), 0, None),
                None => (&[], 0, None),
            },
            false => {
                let (first, _) = slot_ui::quick_window(row);
                (row.page(), first, Some(row.position() - first))
            }
        };
        let mut cards: Vec<(String, String, bool)> = page
            .iter()
            .skip(first)
            .take(slot_ui::SETTINGS_ITEMS)
            .map(|r| {
                let value = match r {
                    QuickRow::DateTime => Some(slot_ui::date_time_text(self.wall_secs())),
                    _ => self.quick_value(*r).map(|v| v.text().to_string()),
                };
                match value {
                    Some(v) => (r.label().to_string(), v, r.opens()),
                    None => (String::new(), r.label().to_string(), true),
                }
            })
            .collect();
        if page.is_empty() {
            let about = match main {
                QuickRow::DateTime => slot_ui::date_time_text(self.wall_secs()),
                QuickRow::Search => "Find a game by name".to_string(),
                _ => "brickslot".to_string(),
            };
            cards.push((main.label().to_string(), about, true));
        }
        SettingsModel {
            side: QuickRow::MAIN.iter().position(|r| *r == main),
            on_side,
            cards,
            focus,
            opens: row.opens() || on_side,
        }
    }

    fn draw_settings(&self, row: QuickRow, out: &mut Vec<Draw>) {
        let model = self.settings_model(row);
        let cards: Vec<slot_ui::Card> = model
            .cards
            .iter()
            .map(|(_, _, opens)| slot_ui::Card { opens: *opens })
            .collect();
        slot_ui::SocketSettings {
            chrome: &self.socket_chrome,
            faces: &self.settings_faces,
            sides: QuickRow::MAIN.len(),
            side_lit: model.side,
            side_focus: model.on_side,
            cards: &cards,
            card_focus: model.focus,
        }
        .draw(out);
    }

    /// The text for every slot of the Socket settings and power popup, while
    /// either shows.
    pub fn settings_lines(&self) -> Option<Vec<(String, bool)>> {
        let row = match self.phase {
            Phase::QuickMenu { row } => Some(row),
            _ => None,
        };
        let customizing = self
            .customize
            .filter(|_| matches!(self.phase, Phase::Shelf));
        let core_popup = self
            .core_popup
            .filter(|_| matches!(self.phase, Phase::Shelf));
        let networks = matches!(self.phase, Phase::Networks { .. });
        let resume_popup = self
            .resume_popup
            .filter(|_| matches!(self.phase, Phase::Shelf));
        let shown = (self.socket_look() && (row.is_some() || self.power_menu.is_some()))
            || customizing.is_some()
            || core_popup.is_some()
            || resume_popup.is_some()
            || networks;
        if !shown {
            return None;
        }
        let key = |k: &str, label: &str| format!("{k}{}{label}", slot_ui::KEY_SEP);
        let mut lines = vec![(String::new(), false); slot_ui::SETTINGS_SLOTS];
        if let Some(row) = row {
            let model = self.settings_model(row);
            for (i, r) in QuickRow::MAIN.iter().enumerate() {
                let label = match r {
                    QuickRow::Game => "Gameplay",
                    QuickRow::Search => "Search",
                    _ => r.label(),
                };
                lines[slot_ui::side_slot(i)] = (label.to_string(), model.side == Some(i));
            }
            for (i, (caption, value, _)) in model.cards.iter().enumerate() {
                lines[slot_ui::caption_slot(i)] = (caption.clone(), false);
                lines[slot_ui::value_slot(i)] = (value.clone(), model.focus == Some(i));
            }
            let close = if model.on_side { "Close" } else { "Back" };
            lines[slot_ui::SETTINGS_CLOSE] = (key("B", close), true);
            let hint = match model.opens {
                true => key("A", "Open"),
                false => key("\u{2190}\u{2192}", "Change"),
            };
            lines[slot_ui::SETTINGS_HINT] = (hint, true);
        }
        if let Some(c) = customizing {
            let material = slot_ui::Material::of(self.selected_shell());
            lines[slot_ui::CUSTOM_TITLE] = ("Customize".into(), true);
            lines[slot_ui::CUSTOM_MATERIAL] = ("Material".into(), true);
            lines[slot_ui::CUSTOM_COLOUR] = ("Color".into(), true);
            lines[slot_ui::CUSTOM_TYPE] = ("Type".into(), true);
            lines[slot_ui::CUSTOM_TYPE_VALUE] = (material.name().into(), true);
            lines[slot_ui::CUSTOM_CLOSE] = (key("B", "Close"), true);
            if c.panel.popup.is_some() && self.power_menu.is_none() {
                lines[slot_ui::POPUP_TITLE_SLOT] = ("Type".into(), true);
                for (i, m) in slot_ui::Material::ALL.iter().enumerate() {
                    lines[slot_ui::option_slot(i)] = (m.name().to_string(), true);
                }
                lines[slot_ui::POPUP_CLOSE_SLOT] = (key("B", "Close"), true);
            }
        }
        if core_popup.is_some() && self.power_menu.is_none() {
            let now = self
                .root
                .as_deref()
                .zip(self.selected_stem())
                .map(|(root, stem)| slot_store::core_for(root, stem));
            lines[slot_ui::POPUP_TITLE_SLOT] = ("Core".into(), true);
            for (i, c) in Core::ALL.iter().enumerate() {
                let text = match now == Some(*c) {
                    true => format!("{}  \u{2713}", c.text()),
                    false => c.text().to_string(),
                };
                lines[slot_ui::option_slot(i)] = (text, true);
            }
            lines[slot_ui::POPUP_CLOSE_SLOT] = (key("B", "Close"), true);
        }
        if resume_popup.is_some() && self.power_menu.is_none() {
            let name = self
                .shelf()
                .carts
                .get(self.shelf().index)
                .map(slot_ui::label_text)
                .unwrap_or_default();
            lines[slot_ui::POPUP_TITLE_SLOT] = (name, true);
            lines[slot_ui::option_slot(0)] = ("Continue where you left off".into(), true);
            lines[slot_ui::option_slot(1)] = ("Start fresh from the save".into(), true);
            lines[slot_ui::POPUP_CLOSE_SLOT] = (key("B", "Cancel"), true);
        }
        if let Phase::Networks { list, scanning, .. } = &self.phase {
            let (first, shown, _) = self.networks_window();
            lines[slot_ui::POPUP_TITLE_SLOT] = ("Wi-Fi Networks".into(), true);
            if *scanning {
                lines[slot_ui::option_slot(0)] = ("Looking for networks\u{2026}".into(), true);
            } else if list.is_empty() {
                lines[slot_ui::option_slot(0)] = ("No networks found".into(), true);
            }
            for (i, n) in list.iter().skip(first).take(shown).enumerate() {
                let bars: String = (1..=4)
                    .map(|b| {
                        if b <= n.bars() {
                            '\u{25cf}'
                        } else {
                            '\u{25cb}'
                        }
                    })
                    .collect();
                let open = if n.secured { "" } else { "   Open" };
                lines[slot_ui::option_slot(i)] = (format!("{}   {bars}{open}", n.ssid), true);
            }
            lines[slot_ui::POPUP_CLOSE_SLOT] = (key("B", "Close    Y  Scan again"), true);
        }
        if self.power_menu.is_some() {
            lines[slot_ui::POPUP_TITLE_SLOT] = ("Power".into(), true);
            for (i, c) in PowerChoice::ALL.iter().enumerate() {
                lines[slot_ui::option_slot(i)] = (c.text().to_string(), true);
            }
            lines[slot_ui::POPUP_CLOSE_SLOT] = (key("B", "Close"), true);
        }
        Some(lines)
    }

    pub fn wants_socket_backdrop(&self) -> bool {
        (self.socket_look() || self.customize.is_some()) && self.socket_backdrop.is_none()
    }

    pub fn set_socket_backdrop(&mut self, tex: TexId) {
        self.socket_backdrop = Some(tex);
    }

    pub fn set_socket_faces(&mut self, faces: Vec<Option<(TexId, u32, u32)>>) {
        self.socket_faces = faces;
    }

    /// The text for every pill of the Socket shelf, while it shows.
    pub fn socket_lines(&self) -> Option<Vec<(String, bool)>> {
        let shelf_up = matches!(
            self.phase,
            Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. }
        );
        if !shelf_up || !(self.socket_look() || self.customize.is_some()) {
            return None;
        }
        let key = |k: &str, label: &str| format!("{k}{}{label}", slot_ui::KEY_SEP);
        let mut lines = vec![(String::new(), false); slot_ui::SOCKET_SLOTS];
        lines[slot_ui::SOCKET_CLOCK] = (slot_ui::hhmm(self.wall_secs()), true);
        if self.shelves.len() > 1 {
            lines[slot_ui::SOCKET_L] = ("L".into(), true);
            lines[slot_ui::SOCKET_R] = ("R".into(), true);
        }
        for (i, (platform, _)) in self.shelves.iter().take(slot_ui::SOCKET_TABS).enumerate() {
            lines[slot_ui::socket_tab_slot(i)] =
                (platform.dir_name().to_string(), i == self.shelf_at);
        }
        let status = slot_ui::SocketStatus {
            percent: self.battery.map(|b| b.percent),
            charging: self
                .battery
                .is_some_and(|b| matches!(b.charge, Charge::Charging)),
            wifi: self.state.wifi,
        };
        lines[slot_ui::SOCKET_STATUS] = (status.text(), true);
        if let Some(cart) = self.shelf().carts.get(self.shelf().index) {
            lines[slot_ui::SOCKET_NAME] = (key("A", &slot_ui::label_text(cart)), true);
        }
        lines[slot_ui::socket_bottom_slot(0)] = (key("MENU", "Settings"), true);
        lines[slot_ui::socket_bottom_slot(1)] = (key("Y", "Search"), true);
        lines[slot_ui::socket_bottom_slot(2)] = (key("X", "Customize"), true);
        Some(lines)
    }

    pub fn customize_open(&self) -> bool {
        self.customize.is_some()
    }

    /// Bumped whenever a cart's shell changes, so the faces built from the
    /// old one are built again.
    pub fn shell_generation(&self) -> u32 {
        self.shell_generation
    }

    pub fn selected_shell(&self) -> Option<slot_store::ShellChoice> {
        self.shelf()
            .carts
            .get(self.shelf().index)
            .and_then(|c| c.shell)
    }

    fn open_customize(&mut self) {
        if self.core_picker.is_some() || self.shelf().carts.is_empty() {
            return;
        }
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.customize = Some(Customize {
            panel: slot_ui::CustomizePanel::default(),
            before: self.selected_shell(),
        });
    }

    fn close_customize(&mut self) {
        let Some(c) = self.customize.take() else {
            return;
        };
        let now = self.selected_shell();
        if now == c.before {
            return;
        }
        let (Some(root), Some(stem)) = (self.root.clone(), self.selected_stem().map(str::to_owned))
        else {
            return;
        };
        if let Err(e) = slot_store::cart_shell::save(&root, &stem, now) {
            eprintln!("slot: customize: {e}");
            self.say("Could not save the cart's colour", e.to_string());
        }
    }

    /// Puts `shell` on the selected cart and has its face built again.
    fn set_shell(&mut self, shell: Option<slot_store::ShellChoice>) {
        let shelf = self.shelf_mut();
        let index = shelf.index;
        let Some(cart) = shelf.carts.get_mut(index) else {
            return;
        };
        if cart.shell == shell {
            return;
        }
        cart.shell = shell;
        let old = shelf.take_face(index);
        self.dropped_faces.extend(old);
        self.shell_generation = self.shell_generation.wrapping_add(1);
    }

    fn customize_input(&mut self, action: Action) {
        let Some(mut c) = self.customize else {
            return;
        };
        let now = self.selected_shell();
        match action {
            Action::GbaDown(Btn::Up) => c.panel.up(),
            Action::GbaDown(Btn::Down) => c.panel.down(),
            Action::GbaDown(Btn::Left)
            | Action::ShelfLeft
            | Action::GbaDown(Btn::Right)
            | Action::ShelfRight => {
                let right = matches!(action, Action::GbaDown(Btn::Right) | Action::ShelfRight);
                if c.panel.popup.is_none() && c.panel.focus == slot_ui::CustomFocus::Material {
                    let all = slot_ui::Material::ALL;
                    let at = all
                        .iter()
                        .position(|m| *m == slot_ui::Material::of(now))
                        .unwrap_or(0);
                    let to = match right {
                        true => (at + 1) % all.len(),
                        false => (at + all.len() - 1) % all.len(),
                    };
                    self.set_shell(slot_ui::with_material(now, all[to]));
                } else {
                    c.panel.step(right);
                }
            }
            Action::GbaDown(Btn::A) => match (c.panel.popup, c.panel.focus) {
                (Some(at), _) => {
                    self.set_shell(slot_ui::with_material(now, slot_ui::Material::ALL[at]));
                    c.panel.popup = None;
                }
                (None, slot_ui::CustomFocus::Material) => {
                    c.panel.popup = slot_ui::Material::ALL
                        .iter()
                        .position(|m| *m == slot_ui::Material::of(now));
                }
                (None, slot_ui::CustomFocus::Swatch(i)) => {
                    self.set_shell(Some(slot_ui::with_colour(now, slot_ui::SWATCHES[i])));
                }
            },
            Action::GbaDown(Btn::B) | Action::GbaDown(Btn::X) if c.panel.popup.is_some() => {
                c.panel.popup = None;
            }
            Action::GbaDown(Btn::B) | Action::GbaDown(Btn::X) => return self.close_customize(),
            _ => {}
        }
        if self.customize.is_some() {
            self.customize = Some(c);
        }
    }

    fn draw_customize(&self, out: &mut Vec<Draw>) {
        let Some(c) = self.customize else {
            return;
        };
        let screen = self.socket_screen(0.0);
        screen.draw_backdrop(out);
        self.shelf()
            .draw_socket_solo(self.now(), slot_ui::CUSTOM_CART_X, out);
        screen.draw_clock(out);
        screen.draw_name_at(slot_ui::CUSTOM_CART_X, out);
        let chosen = self
            .selected_shell()
            .and_then(|s| slot_ui::SWATCHES.iter().position(|w| *w == s.colour));
        slot_ui::CustomizeScreen {
            chrome: &self.socket_chrome,
            faces: &self.settings_faces,
            panel: c.panel,
            chosen,
        }
        .draw(out);
        if let Some(Some((tex, w, h))) = self.settings_faces.get(slot_ui::CUSTOM_CLOSE).copied() {
            out.push(Draw::Tex {
                x: 12.0,
                y: (OUT_H - 12) as f32 - h as f32,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
    }

    /// Whether Restart has been pressed once and waits for a second press.
    fn restart_armed(&self) -> bool {
        self.restart_armed_at
            .is_some_and(|at| self.now().saturating_sub(at) <= RESTART_CONFIRM_MS)
    }

    /// Takes the game out without keeping where it got to and puts it back
    /// fresh, so it starts from the save on the card.
    fn restart_game(&mut self) {
        let cart = match &mut self.phase {
            Phase::Playing { cart } => std::mem::take(cart),
            _ => return,
        };
        eprintln!("slot: restart: {cart} from the save on the card");
        self.end_link();
        self.close_game_menu();
        self.settle_saves();
        self.pending = None;
        self.refusal = None;
        self.refused_from = None;
        self.restart_after_eject = true;
        self.phase = Phase::Ejecting {
            cart,
            t: -EJECT_HOLD_S,
        };
    }

    pub fn home_open(&self) -> bool {
        self.home.is_some()
    }

    pub fn home_focus(&self) -> Option<slot_ui::HomeFocus> {
        self.home.map(|p| p.focus())
    }

    fn open_home(&mut self) {
        if self.core_picker.is_some() || self.link_active() {
            return;
        }
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.home = Some(slot_ui::HomePanel::new());
        self.home_stats = crate::sysinfo::Stats::read();
        self.home_stats_at = self.now();
    }

    fn close_home(&mut self) {
        self.home = None;
        self.restart_armed_at = None;
    }

    fn home_tick(&mut self) {
        let now = self.now();
        if self.home.is_some() && now.saturating_sub(self.home_stats_at) >= HOME_STATS_MS {
            self.home_stats = crate::sysinfo::Stats::read();
            self.home_stats_at = now;
        }
    }

    fn home_input(&mut self, action: Action) {
        let Some(mut panel) = self.home else {
            return;
        };
        let focus = panel.focus();
        match action {
            Action::GbaDown(Btn::Up) => panel.up(),
            Action::GbaDown(Btn::Down) => panel.down(),
            Action::GbaDown(Btn::Left)
            | Action::ShelfLeft
            | Action::GbaDown(Btn::Right)
            | Action::ShelfRight => {
                let right = matches!(action, Action::GbaDown(Btn::Right) | Action::ShelfRight);
                if !panel.step(right) {
                    let step = match (focus, right) {
                        (slot_ui::HomeFocus::Brightness, true) => Action::BrightnessUp,
                        (slot_ui::HomeFocus::Brightness, false) => Action::BrightnessDown,
                        (_, true) => Action::VolumeUp,
                        (_, false) => Action::VolumeDown,
                    };
                    self.adjust(step);
                }
            }
            Action::GbaDown(Btn::A) => match focus {
                slot_ui::HomeFocus::Tile(tile) => return self.home_tile(tile),
                slot_ui::HomeFocus::Volume => self.mute_toggle(),
                slot_ui::HomeFocus::Brightness => {}
            },
            Action::GbaDown(Btn::B) | Action::GbaDown(Btn::Home) | Action::HomePanel => {
                return self.close_home()
            }
            Action::QuickMenu if self.on_shelf() => {
                self.close_home();
                return self.open_quick_menu();
            }
            _ => {}
        }
        if self.home.is_some() {
            self.home = Some(panel);
        }
    }

    fn home_tile(&mut self, tile: slot_ui::HomeTile) {
        match tile {
            slot_ui::HomeTile::Screenshot => {
                if !matches!(self.phase, Phase::Playing { .. }) {
                    return self.say("Screenshots are taken in a game", "");
                }
                self.close_home();
                self.screenshot();
            }
            slot_ui::HomeTile::Sound => self.mute_toggle(),
            slot_ui::HomeTile::Rumble => self.change_setting(QuickRow::Rumble, true),
            slot_ui::HomeTile::Wifi => self.change_setting(QuickRow::Wifi, true),
            slot_ui::HomeTile::Lights => self.change_setting(QuickRow::LightMode, true),
            slot_ui::HomeTile::Restart => {
                if !matches!(self.phase, Phase::Playing { .. }) {
                    return self.say("Restart works in a game", "");
                }
                if !self.restart_armed() {
                    self.restart_armed_at = Some(self.now());
                    return;
                }
                self.close_home();
                self.restart_game();
            }
            slot_ui::HomeTile::Power => {
                self.close_home();
                self.open_power_menu();
            }
        }
    }

    fn home_tiles_on(&self) -> [bool; slot_ui::HomeTile::ALL.len()] {
        slot_ui::HomeTile::ALL.map(|tile| match tile {
            slot_ui::HomeTile::Screenshot => matches!(self.phase, Phase::Playing { .. }),
            slot_ui::HomeTile::Sound => !self.muted(),
            slot_ui::HomeTile::Rumble => self.state.rumble,
            slot_ui::HomeTile::Wifi => self.state.wifi,
            slot_ui::HomeTile::Lights => self.state.light_mode != LightMode::Off,
            slot_ui::HomeTile::Restart => self.restart_armed(),
            slot_ui::HomeTile::Power => false,
        })
    }

    /// The text for every slot of the Home panel and whether it is lit,
    /// while the panel is up.
    pub fn home_lines(&self) -> Option<Vec<(String, bool)>> {
        self.home?;
        let on = self.home_tiles_on();
        let mut lines = vec![(String::new(), false); slot_ui::HOME_SLOTS];
        let title = match self.phase {
            Phase::Playing { .. } => self.seated().unwrap_or("brickslot").to_string(),
            _ => "brickslot".to_string(),
        };
        lines[slot_ui::HOME_TITLE] = (title, true);
        lines[slot_ui::HOME_CLOCK] = (slot_ui::hhmm(self.wall_secs()), true);
        for (i, tile) in slot_ui::HomeTile::ALL.iter().enumerate() {
            let state = match tile {
                slot_ui::HomeTile::Screenshot if on[i] => "Take",
                slot_ui::HomeTile::Screenshot => "In game",
                slot_ui::HomeTile::Wifi if on[i] && !self.net.online() => "Joining",
                slot_ui::HomeTile::Lights => match self.state.light_mode {
                    LightMode::Battery => "Battery",
                    LightMode::Colour => "Colour",
                    LightMode::Off => "Off",
                },
                slot_ui::HomeTile::Restart if on[i] => "Press A again",
                slot_ui::HomeTile::Restart => "From save",
                slot_ui::HomeTile::Power => "Menu",
                _ if on[i] => "On",
                _ => "Off",
            };
            lines[slot_ui::home_tile_name_slot(i)] = (tile.name().to_string(), true);
            lines[slot_ui::home_tile_state_slot(i)] = (state.to_string(), on[i]);
        }
        let focus = self.home.map(|p| p.focus());
        lines[slot_ui::home_slider_name_slot(0)] = (
            "Brightness".into(),
            focus == Some(slot_ui::HomeFocus::Brightness),
        );
        lines[slot_ui::home_slider_name_slot(1)] =
            ("Volume".into(), focus == Some(slot_ui::HomeFocus::Volume));
        lines[slot_ui::home_slider_value_slot(0)] = (
            format!("{}/{}", self.state.brightness, BRIGHTNESS_MAX),
            true,
        );
        let volume = match self.muted() {
            true => "Muted".to_string(),
            false => format!("{}%", self.level()),
        };
        lines[slot_ui::home_slider_value_slot(1)] = (volume, true);
        let stats = &self.home_stats;
        let battery = self
            .battery
            .map_or("--".to_string(), |b| format!("{}%", b.percent));
        let named = [
            ("Temp", crate::sysinfo::temp_text(stats)),
            ("CPU", crate::sysinfo::cpu_text(stats)),
            ("RAM", crate::sysinfo::ram_text(stats)),
            ("Battery", battery),
        ];
        for (i, (name, value)) in named.into_iter().enumerate() {
            lines[slot_ui::home_stat_name_slot(i)] = (name.to_string(), false);
            lines[slot_ui::home_stat_value_slot(i)] = (value, true);
        }
        let legend = match self.on_shelf() {
            true => "A select   D-PAD adjust   B close   MENU settings",
            false => "A select   D-PAD adjust   B resume",
        };
        lines[slot_ui::HOME_LEGEND] = (legend.to_string(), false);
        Some(lines)
    }

    pub fn set_home_faces(&mut self, faces: Vec<Option<(TexId, u32, u32)>>) {
        self.home_faces = faces;
    }

    pub fn search_light(&self) -> bool {
        self.socket_look() || matches!(self.phase, Phase::Password { .. })
    }

    fn open_networks(&mut self) {
        if !self.state.wifi {
            self.state.wifi = true;
            self.persist();
        }
        self.net.send(NetJob::Scan);
        self.phase = Phase::Networks {
            list: Vec::new(),
            at: 0,
            scanning: true,
        };
    }

    /// The first network the list shows, how many it shows, and which of
    /// them the cursor is on.
    fn networks_window(&self) -> (usize, usize, Option<usize>) {
        let Phase::Networks { list, at, scanning } = &self.phase else {
            return (0, 0, None);
        };
        if *scanning || list.is_empty() {
            return (0, 0, None);
        }
        let shown = list.len().min(slot_ui::POPUP_OPTIONS);
        let first = at.saturating_sub(shown - 1).min(list.len() - shown);
        (first, shown, Some(at - first))
    }

    /// Puts a finished scan's networks on the list, while it is up.
    pub fn networks_found(&mut self, found: Vec<crate::net::Network>) {
        if let Phase::Networks { list, at, scanning } = &mut self.phase {
            *list = found;
            *at = 0;
            *scanning = false;
        }
    }

    fn back_to_join(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::JoinNetwork,
        };
    }

    fn join(&mut self, ssid: String, psk: String) {
        self.say(format!("Joining {ssid}"), "");
        self.net.send(NetJob::Join { ssid, psk });
        self.back_to_join();
    }

    fn networks_input(&mut self, action: Action) {
        let Phase::Networks { list, at, scanning } = &mut self.phase else {
            return;
        };
        let n = list.len();
        match action {
            Action::GbaDown(Btn::Up) if n > 0 && !*scanning => *at = (*at + n - 1) % n,
            Action::GbaDown(Btn::Down) if n > 0 && !*scanning => *at = (*at + 1) % n,
            Action::GbaDown(Btn::Y) if !*scanning => {
                *scanning = true;
                self.net.send(NetJob::Scan);
            }
            Action::GbaDown(Btn::A) if n > 0 && !*scanning => {
                let picked = list[*at].clone();
                let list = std::mem::take(list);
                if picked.secured {
                    self.phase = Phase::Password {
                        ssid: picked.ssid,
                        keys: crate::keyboard::Keyboard::default(),
                        list,
                    };
                } else {
                    self.join(picked.ssid, String::new());
                }
            }
            Action::GbaDown(Btn::B) | Action::QuickMenu => self.back_to_join(),
            _ => {}
        }
    }

    fn password_input(&mut self, action: Action) {
        let Phase::Password { ssid, keys, list } = &mut self.phase else {
            return;
        };
        match action {
            Action::GbaDown(Btn::Up) => keys.step(0, -1),
            Action::GbaDown(Btn::Down) => keys.step(0, 1),
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => keys.step(-1, 0),
            Action::GbaDown(Btn::Right) | Action::ShelfRight => keys.step(1, 0),
            Action::GbaDown(Btn::A) => keys.type_key(),
            Action::GbaDown(Btn::X) => keys.space(),
            Action::GbaDown(Btn::Y) => keys.next_layer(),
            Action::GbaDown(Btn::B) => {
                if !keys.delete() {
                    let at = list.iter().position(|n| n.ssid == *ssid).unwrap_or(0);
                    self.phase = Phase::Networks {
                        list: std::mem::take(list),
                        at,
                        scanning: false,
                    };
                }
            }
            Action::GbaDown(Btn::Start) if !keys.text.is_empty() => {
                let (ssid, psk) = (ssid.clone(), keys.text.clone());
                self.join(ssid, psk);
            }
            Action::QuickMenu => self.back_to_join(),
            _ => {}
        }
    }

    fn open_search(&mut self) {
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.refresh_search();
        self.phase = Phase::Search;
    }

    fn refresh_search(&mut self) {
        self.search_picked_at = self.now();
        let stems: Vec<(Platform, Vec<String>)> = self
            .shelves
            .iter()
            .map(|(p, s)| (*p, s.carts.iter().map(|c| c.stem.clone()).collect()))
            .collect();
        self.search
            .refresh(stems.iter().map(|(p, s)| (*p, s.as_slice())));
    }

    fn search_input(&mut self, action: Action) {
        let changed = match action {
            Action::GbaDown(Btn::Up) => {
                self.search.step(0, -1);
                false
            }
            Action::GbaDown(Btn::Down) => {
                self.search.step(0, 1);
                false
            }
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => {
                self.search.step(-1, 0);
                false
            }
            Action::GbaDown(Btn::Right) | Action::ShelfRight => {
                self.search.step(1, 0);
                false
            }
            Action::GbaDown(Btn::A) => self.search.type_key(),
            Action::GbaDown(Btn::B) => {
                if !self.search.delete() {
                    self.phase = Phase::Shelf;
                    return;
                }
                true
            }
            Action::GbaDown(Btn::L1) => {
                self.search.pick_by(-1);
                self.search_picked_at = self.now();
                false
            }
            Action::GbaDown(Btn::R1) => {
                self.search.pick_by(1);
                self.search_picked_at = self.now();
                false
            }
            Action::GbaDown(Btn::Start) => {
                self.go_to_hit();
                return;
            }
            Action::QuickMenu | Action::GbaDown(Btn::Home) => {
                self.phase = Phase::Shelf;
                return;
            }
            _ => false,
        };
        if changed {
            self.refresh_search();
        }
    }

    fn go_to_hit(&mut self) {
        let Some(hit) = self.search.picked().cloned() else {
            return;
        };
        self.phase = Phase::Shelf;
        if hit.shelf != self.shelf_at {
            self.shelf_at = hit.shelf;
            self.shelf_platform = slot_ui::Printed::default();
            self.slot_letter = None;
        }
        self.shelf_mut().select(hit.index);
        self.shelf_named = None;
        self.name_pending = true;
    }

    /// The text for every slot of the search screen and whether it is lit,
    /// while the screen is up.
    pub fn search_lines(&self) -> Option<Vec<(String, bool)>> {
        if let Phase::Password { ssid, keys, .. } = &self.phase {
            return Some(password_lines(ssid, keys));
        }
        if !matches!(self.phase, Phase::Search) {
            return None;
        }
        let s = &self.search;
        let mut lines = vec![(String::new(), false); slot_ui::SEARCH_SLOTS];
        lines[slot_ui::SEARCH_QUERY] = (format!("Search: {}_", s.query), true);
        for i in 0..slot_ui::SEARCH_HITS {
            let text = match s.hits.get(i) {
                Some(hit) => format!("{}  ({})", hit.name, hit.platform.dir_name()),
                None if i == 0 && !s.query.trim().is_empty() => "No games found".to_string(),
                None => String::new(),
            };
            lines[slot_ui::search_hit_slot(i)] = (text, s.pick == i);
        }
        for (row, keys) in crate::search::KEYS.iter().enumerate() {
            for (col, key) in keys.iter().enumerate() {
                let lit = (row, col) == (s.row, s.col);
                lines[slot_ui::search_key_slot(row, col)] = (key.to_string(), lit);
            }
        }
        let more = match s.hits.len() {
            n if n > slot_ui::SEARCH_HITS => format!("   {n} found"),
            _ => String::new(),
        };
        lines[slot_ui::SEARCH_LEGEND] = (
            format!("A type   B delete   L/R pick   START go{more}"),
            false,
        );
        Some(lines)
    }

    pub fn set_search_faces(&mut self, faces: Vec<Option<(TexId, u32, u32)>>) {
        self.search_faces = faces;
    }

    pub fn search(&self) -> &crate::search::Search {
        &self.search
    }

    fn open_quick_menu(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::MAIN[0],
        };
    }

    fn quick_menu_input(&mut self, row: QuickRow, action: Action) {
        let row = match action {
            Action::GbaDown(Btn::Up) => row.up(),
            Action::GbaDown(Btn::Down) => row.down(),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) => return self.open_quick_row(row),
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = match row.parent() {
                    Some(row) => Phase::QuickMenu { row },
                    None => Phase::Shelf,
                };
                return;
            }
            _ => return,
        };
        self.phase = Phase::QuickMenu { row };
    }

    fn open_quick_row(&mut self, row: QuickRow) {
        match row {
            QuickRow::DateTime => {
                self.phase = clock_screen(self.utc_secs(), self.state.utc_offset_min, true);
            }
            QuickRow::About => self.phase = Phase::About,
            QuickRow::Search => self.open_search(),
            QuickRow::Labels => self.download_labels(),
            QuickRow::JoinNetwork => self.open_networks(),
            QuickRow::Screen | QuickRow::Game | QuickRow::Lights | QuickRow::Network => {
                if let Some(row) = row.child() {
                    self.phase = Phase::QuickMenu { row };
                }
            }
            QuickRow::EjectSave
            | QuickRow::Look
            | QuickRow::ClockFormat
            | QuickRow::AskResume
            | QuickRow::Turbo
            | QuickRow::Rewind
            | QuickRow::FastForward
            | QuickRow::FastForwardSound
            | QuickRow::ColourCorrection
            | QuickRow::GbaShader
            | QuickRow::GbShader
            | QuickRow::GbPalettes
            | QuickRow::Brightness
            | QuickRow::Warmth
            | QuickRow::LightMode
            | QuickRow::LightColour
            | QuickRow::LightEffect
            | QuickRow::LightLevel
            | QuickRow::Wifi
            | QuickRow::Syncthing
            | QuickRow::Achievements
            | QuickRow::SaveFormat
            | QuickRow::AutoTime
            | QuickRow::Performance
            | QuickRow::FileTransfer
            | QuickRow::Rumble => {}
        }
    }

    fn change_setting(&mut self, row: QuickRow, right: bool) {
        let s = &mut self.state;
        match row {
            QuickRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            QuickRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            QuickRow::ColourCorrection => {
                s.colour_correction = !s.colour_correction;
                self.colour_pending = Some(s.colour_correction);
            }
            QuickRow::GbaShader => {
                let to = s.shader_gba.step(&Shader::GBA, right);
                if to == s.shader_gba {
                    return;
                }
                s.shader_gba = to;
            }
            QuickRow::GbShader => {
                let to = s.shader_gb.step(&Shader::GB, right);
                if to == s.shader_gb {
                    return;
                }
                s.shader_gb = to;
            }
            QuickRow::Rumble => s.rumble = !s.rumble,
            QuickRow::EjectSave => s.eject_save = !s.eject_save,
            QuickRow::AskResume => s.ask_resume = !s.ask_resume,
            QuickRow::Turbo => s.turbo = !s.turbo,
            QuickRow::Rewind => s.rewind = !s.rewind,
            QuickRow::GbPalettes => s.gb_palettes = !s.gb_palettes,
            QuickRow::Brightness => {
                let to = match right {
                    true => up(s.brightness, 1, BRIGHTNESS_MAX),
                    false => s.brightness.saturating_sub(1),
                };
                if to == s.brightness {
                    return;
                }
                s.brightness = to;
                if let Some(power) = &mut self.power {
                    power.set_backlight(to);
                }
            }
            QuickRow::Warmth => {
                let to = match right {
                    true => up(s.blue_light, 1, BLUE_LIGHT_MAX),
                    false => s.blue_light.saturating_sub(1),
                };
                if to == s.blue_light {
                    return;
                }
                s.blue_light = to;
            }
            QuickRow::LightMode => s.light_mode = s.light_mode.step(right),
            QuickRow::LightColour => s.light_colour = s.light_colour.step(right),
            QuickRow::LightEffect => s.light_effect = s.light_effect.step(right),
            QuickRow::LightLevel => s.light_level = s.light_level.step(right),
            QuickRow::Wifi => {
                s.wifi = !s.wifi;
                if s.wifi {
                    self.net.send(NetJob::WifiUp);
                } else {
                    s.syncthing = false;
                    s.achievements = false;
                    self.file_server = None;
                    crate::cheevos::enable(false);
                    self.cheevos_login = CheevosLogin::Waiting;
                    self.net.send(NetJob::WifiDown);
                }
            }
            QuickRow::SaveFormat => s.save_format = s.save_format.other(),
            QuickRow::Look => s.look = s.look.other(),
            QuickRow::ClockFormat => {
                s.clock_12h = !s.clock_12h;
                slot_ui::set_twelve_hour(s.clock_12h);
            }
            QuickRow::FileTransfer => return self.toggle_file_transfer(),
            QuickRow::AutoTime => {
                s.auto_time = !s.auto_time;
                self.clock_asked = false;
            }
            QuickRow::Performance => {
                s.cpu = s.cpu.step(right);
                let profile = cpu_profile(s.cpu);
                if let Some(power) = &mut self.power {
                    power.set_cpu(profile);
                }
            }
            QuickRow::Achievements => {
                s.achievements = !s.achievements;
                crate::cheevos::enable(s.achievements);
                self.cheevos_login = CheevosLogin::Waiting;
                if s.achievements && !s.wifi {
                    s.wifi = true;
                    self.net.send(NetJob::WifiUp);
                }
            }
            QuickRow::Syncthing => {
                s.syncthing = !s.syncthing;
                if s.syncthing {
                    s.wifi = true;
                    self.net.send(NetJob::SyncthingStart);
                } else {
                    self.net.send(NetJob::SyncthingStop);
                }
            }
            QuickRow::DateTime
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::Lights
            | QuickRow::Network
            | QuickRow::Search
            | QuickRow::Labels
            | QuickRow::JoinNetwork => return,
        }
        if QuickRow::LIGHTS.contains(&row) {
            let lights = self.lights();
            if let Some(power) = &mut self.power {
                power.set_lights(lights);
            }
        }
        self.persist();
    }

    pub fn apply_at(&mut self, action: Action, now: Millis) {
        self.clock = now as f64;
        self.apply(action);
    }

    fn step_shader(&mut self, right: bool) {
        if !matches!(self.phase, Phase::Inserting { .. } | Phase::Playing { .. }) {
            return;
        }
        let (shader, choices) = match self.platform {
            Platform::Gba => (&mut self.state.shader_gba, &Shader::GBA[..]),
            Platform::Gb | Platform::Gbc => (&mut self.state.shader_gb, &Shader::GB[..]),
        };
        *shader = shader.step(choices, right);
        let said = match *shader {
            Shader::Off => Toast::ShaderOff,
            Shader::Lcd3x => Toast::ShaderLcd3x,
            Shader::Grid => Toast::ShaderGrid,
            Shader::Dot => Toast::ShaderDot,
            Shader::Simpletex => Toast::ShaderSimpletex,
        };
        self.persist();
        self.hud.toast(said, self.now());
    }

    fn adjust(&mut self, action: Action) -> bool {
        if action == Action::MuteToggle {
            self.mute_toggle();
            return true;
        }
        if let Action::ShaderNext | Action::ShaderPrev = action {
            self.step_shader(action == Action::ShaderNext);
            return true;
        }
        if let Action::PaletteNext | Action::PalettePrev = action {
            if self.palette_live && matches!(self.phase, Phase::Playing { .. }) {
                let to = match action {
                    Action::PalettePrev => self.state.gb_palette.prev(),
                    _ => self.state.gb_palette.next(),
                };
                self.state.gb_palette = to;
                self.palette_pending = Some(to);
                self.persist();
                self.hud.toast(Toast::Palette(to), self.now());
            }
            return true;
        }
        if action == Action::ColourCorrectionToggle {
            self.change_setting(QuickRow::ColourCorrection, true);
            let said = match self.state.colour_correction {
                true => Toast::ColourOn,
                false => Toast::ColourOff,
            };
            self.hud.toast(said, self.now());
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
            Action::BrightnessUp => (HudKind::Brightness, up(s.brightness, 1, BRIGHTNESS_MAX)),
            Action::BrightnessDown => (HudKind::Brightness, s.brightness.saturating_sub(1)),
            Action::BlueLightUp => (HudKind::BlueLight, up(s.blue_light, 1, BLUE_LIGHT_MAX)),
            Action::BlueLightDown => (HudKind::BlueLight, s.blue_light.saturating_sub(1)),
            Action::VolumeUp => (HudKind::Volume, up(self.level(), VOLUME_STEP, VOLUME_MAX)),
            Action::VolumeDown => (HudKind::Volume, self.level().saturating_sub(VOLUME_STEP)),
            _ => return false,
        };
        if kind == HudKind::Volume {
            self.remember_volume();
        }
        let level = match kind {
            HudKind::Brightness => &mut self.state.brightness,
            HudKind::BlueLight => &mut self.state.blue_light,
            HudKind::Volume if self.headphones => &mut self.state.volume_hp,
            HudKind::Volume => &mut self.state.volume,
            HudKind::Rewind => return false,
        };
        let moved = *level != value;
        *level = value;
        let unmuted = kind == HudKind::Volume && std::mem::take(self.muted_mut());
        let (shown, now) = (self.hud_value(kind, value), self.now());
        self.hud.show(kind, shown, self.muted(), now);
        if let (HudKind::Brightness, Some(power)) = (kind, &mut self.power) {
            power.set_backlight(value);
        }
        if moved || unmuted {
            self.persist_soon();
        }
        true
    }

    fn remember_volume(&mut self) {
        if self.vol_before.len() == 2 {
            self.vol_before.remove(0);
        }
        self.vol_before
            .push((self.level(), self.muted(), self.now()));
    }

    fn mute_toggle(&mut self) {
        let now = self.now();
        if let Some((volume, muted, _)) = self
            .vol_before
            .iter()
            .find(|(_, _, at)| now.saturating_sub(*at) <= MUTE_CHORD_MS)
            .copied()
        {
            *self.level_mut() = volume;
            *self.muted_mut() = muted;
        }
        self.vol_before.clear();
        let muted = !self.muted();
        *self.muted_mut() = muted;
        self.hud
            .show(HudKind::Volume, self.output_volume(), muted, now);
        self.persist_soon();
    }

    fn hud_value(&self, kind: HudKind, value: u8) -> u8 {
        match kind {
            HudKind::Volume => self.output_volume(),
            _ => value,
        }
    }

    pub fn show_rewind(&mut self, fill: u8) {
        let now = self.now();
        self.hud.show(HudKind::Rewind, fill, false, now);
    }

    pub fn hide_rewind(&mut self) {
        self.hud.release_rewind();
    }

    pub fn set_ff(&mut self, ff: FfState) {
        self.hud.set_ff(ff);
    }

    pub fn ff_badge(&self) -> Option<Icon> {
        self.hud.badge()
    }

    pub fn blue_light(&self) -> u8 {
        self.state.blue_light
    }

    pub fn brightness(&self) -> u8 {
        self.state.brightness
    }

    pub fn volume(&self) -> u8 {
        self.level()
    }

    fn level(&self) -> u8 {
        if self.headphones {
            self.state.volume_hp
        } else {
            self.state.volume
        }
    }

    fn level_mut(&mut self) -> &mut u8 {
        if self.headphones {
            &mut self.state.volume_hp
        } else {
            &mut self.state.volume
        }
    }

    pub fn muted(&self) -> bool {
        if self.headphones {
            self.state.muted_hp
        } else {
            self.state.muted
        }
    }

    fn muted_mut(&mut self) -> &mut bool {
        if self.headphones {
            &mut self.state.muted_hp
        } else {
            &mut self.state.muted
        }
    }

    pub fn output_volume(&self) -> u8 {
        if self.muted() {
            0
        } else {
            self.level()
        }
    }

    pub fn hud_icon(&self) -> Icon {
        self.hud.glyph()
    }

    pub fn now(&self) -> Millis {
        self.clock as Millis
    }

    fn flick(&mut self, step: fn(&mut Polaroids)) {
        if let Some(p) = &mut self.polaroids {
            step(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.clock += dt as f64 * 1000.0;
        if self.name_pending && self.shelf_platform.face.is_some() {
            self.name_pending = false;
            self.shelf_named = Some(self.now());
        }
        self.timers();
        self.poll_link();
        self.home_tick();
        if matches!(self.phase, Phase::Networks { scanning: true, .. }) {
            if let Some(found) = self.net.networks() {
                self.networks_found(found);
            }
        }
        if slot_ui::twelve_hour() != self.state.clock_12h {
            slot_ui::set_twelve_hour(self.state.clock_12h);
        }
        let now = self.now();
        let ready = self.core_faces_ready();
        if let Some(picker) = &mut self.core_picker {
            if picker.waiting() && (ready || picker.waited(now) >= FACES_WAIT_MS) {
                picker.start(now);
            }
        }
        if self.core_picker.is_some_and(|p| p.finished(now)) {
            self.core_picker = None;
        }
        if !self.on_shelf() {
            self.shelf_mut().release_hold();
        }
        let mut touched = false;
        let at = self.shelf_at;
        let next = match &mut self.phase {
            Phase::Shelf => {
                let shelf = &mut self.shelves[at].1;
                shelf.tick(now);
                shelf.update(dt);
                None
            }
            Phase::Inserting {
                cart,
                t,
                core_ready,
                resumed,
                ..
            } => {
                let was = *t;
                *t += dt;
                let at = SEATED_AT - Sfx::Insert.lead();
                touched = !*resumed && was < at && *t >= at;
                (*t >= INSERT_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += dt;
                    touched = was < 0.0 && *t >= 0.0;
                }
                (*t >= EJECT_S).then_some(Phase::Shelf)
            }
            _ => None,
        };
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => Sfx::Eject,
                _ => Sfx::Insert,
            });
        }
        if let Some(phase) = next {
            self.phase = phase;
            self.refused_from = None;
            if matches!(self.phase, Phase::Shelf) && std::mem::take(&mut self.restart_after_eject) {
                self.insert(true);
            }
            let seated = match &self.phase {
                Phase::Playing { cart } => Some(cart.clone()),
                _ => None,
            };
            if seated.is_none() {
                self.link_loaded = None;
            }
            self.record_cart(seated);
        }
        self.step_screen(dt);
    }

    fn step_screen(&mut self, dt: f32) {
        let lit = matches!(self.phase, Phase::Playing { .. } | Phase::Polaroids { .. });
        let step = if lit {
            dt / POWER_ON_S
        } else {
            -dt / POWER_OFF_S
        };
        self.screen = (self.screen + step).clamp(0.0, 1.0);
        let docked = self.power_menu.is_none()
            && !self.socket_look()
            && self.customize.is_none()
            && matches!(
                self.phase,
                Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. }
            );
        let sliding = self.power_menu.is_none()
            && matches!(
                self.phase,
                Phase::Inserting { .. }
                    | Phase::Playing { .. }
                    | Phase::Polaroids { .. }
                    | Phase::Ejecting { .. }
            );
        let reach = if sliding { dt / POWER_ON_S } else { 1.0 };
        self.lift = match docked {
            true => (self.lift - reach).max(0.0),
            false => (self.lift + reach).min(1.0),
        };
    }

    pub fn frame_split(&self) -> Option<f32> {
        match (&self.phase, self.power_menu) {
            (Phase::Shelf, None) if self.customize.is_some() => Some(slot_ui::SOCKET_SPLIT),
            (Phase::Shelf, None) if self.socket_look() && self.home.is_some() => {
                Some(slot_ui::HOME_SPLIT)
            }
            (Phase::Search, None) if self.socket_look() => Some(slot_ui::SEARCH_SPLIT),
            (Phase::Password { .. }, None) => Some(slot_ui::SEARCH_SPLIT),
            (Phase::QuickMenu { .. }, None) if self.socket_look() => Some(slot_ui::SETTINGS_SPLIT),
            (Phase::QuickMenu { .. }, None) => Some(slot_ui::QUICK_SPLIT),
            (Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. }, None)
                if self.socket_look() =>
            {
                Some(slot_ui::SOCKET_SPLIT)
            }
            _ => None,
        }
    }

    pub fn frame_lift(&self) -> f32 {
        self.lift
    }

    pub fn screen_power(&self) -> f32 {
        self.screen
    }

    pub fn set_game_ready(&mut self, ready: bool) {
        self.game_ready = ready;
    }

    pub fn game_ready(&self) -> bool {
        self.game_ready
    }

    pub fn game_visible(&self) -> bool {
        self.game_ready && self.screen > 0.0
    }

    pub fn tick_ms(&mut self, now: Millis) {
        self.clock = self.clock.max(now as f64);
        self.timers();
    }

    pub fn say(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        self.messages.push_back((title.into(), detail.into()));
    }

    pub fn take_message(&mut self) -> Option<(String, String)> {
        if self.hud.said(self.now()) == Some(Toast::Message) {
            return None;
        }
        self.messages.pop_front()
    }

    pub fn show_message(&mut self) {
        let now = self.now();
        self.hud.toast(Toast::Message, now);
    }

    pub fn message_tex(&self) -> Option<TexId> {
        self.hud.toast_tex(Toast::Message)
    }

    fn track_play(&mut self) {
        let now = self.now();
        let playing = matches!(self.phase, Phase::Playing { .. })
            && self.game_menu.is_none()
            && self.home.is_none();
        if playing {
            if let Some(last) = self.play_tick {
                self.play_ms += now.saturating_sub(last).min(PLAY_GAP_MS);
            }
            self.play_tick = Some(now);
        } else {
            self.play_tick = None;
        }
        if playing && self.play_key.is_none() {
            if let Some(cart) = self.seated_cart() {
                let key = crate::playtime::key(cart.platform, &cart.stem);
                let title = cart.stem.clone();
                if let Some(root) = &self.root {
                    let secs = crate::playtime::read(root, &key);
                    if secs >= 60 {
                        self.say(format!("Played {}", crate::playtime::format(secs)), title);
                    }
                }
                self.play_key = Some(key);
            }
        }
        if self.play_ms >= PLAY_FLUSH_MS || (!playing && self.play_ms >= 1000) {
            self.flush_play();
        }
        if matches!(self.phase, Phase::Shelf | Phase::Ejecting { .. }) {
            self.flush_play();
            self.play_key = None;
        }
    }

    fn flush_play(&mut self) {
        let secs = self.play_ms / 1000;
        if secs == 0 {
            return;
        }
        let (Some(root), Some(key)) = (self.root.clone(), self.play_key.clone()) else {
            self.play_ms = 0;
            return;
        };
        self.play_ms -= secs * 1000;
        let _ = std::thread::Builder::new()
            .name("slot-playtime".into())
            .spawn(move || {
                if let Err(e) = crate::playtime::add(&root, &key, secs) {
                    eprintln!("slot: playtime {key}: {e}");
                }
            });
    }

    fn clock_tick(&mut self) {
        if !self.state.auto_time {
            return;
        }
        if !self.clock_asked && self.net.online() {
            self.clock_asked = true;
            self.net.send(NetJob::SyncClock);
        }
        if let Some(utc) = self.net.clock() {
            self.set_network_time(utc);
        }
    }

    /// Takes the time a time server gave. The time zone stays the one picked
    /// under Date & Time, so only the UTC clock moves.
    pub fn set_network_time(&mut self, utc: i64) {
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
        }
        let was_set = self.state.clock_set;
        self.state.clock_set = true;
        self.persist();
        if matches!(
            self.phase,
            Phase::SetClock {
                from_menu: false,
                ..
            }
        ) {
            self.start();
        }
        let local = utc + i64::from(self.state.utc_offset_min) * 60;
        let (h, m) = (local.rem_euclid(86_400) / 3600, local.rem_euclid(3600) / 60);
        let detail = match was_set {
            true => format!("{h:02}:{m:02}"),
            false => format!("{h:02}:{m:02} UTC, set your time zone in Date & Time"),
        };
        self.say("Clock set from the internet", detail);
    }

    fn cheevos_tick(&mut self) {
        if !self.state.achievements {
            return;
        }
        if self.cheevos_login == CheevosLogin::Waiting && self.net.online() {
            if let Some(root) = self.root.clone() {
                self.cheevos_login = match crate::cheevos::login(&root) {
                    crate::cheevos::Login::Started => CheevosLogin::Sent,
                    crate::cheevos::Login::NoAccount => {
                        self.say(
                            "RetroAchievements needs an account",
                            "Put username= and password= in Config/retroachievements.txt",
                        );
                        CheevosLogin::NoAccount
                    }
                };
            }
        }
        for event in crate::cheevos::take() {
            if let slot_cheevos::Event::LoggedIn { user, token } = &event {
                if let Some(root) = &self.root {
                    if let Err(e) = crate::cheevos::remember(root, user, token) {
                        eprintln!("slot: cheevos: {e}");
                    }
                }
            }
            if let Some((title, detail)) = crate::cheevos::words(&event) {
                self.say(title, detail);
            }
        }
    }

    pub fn net_jobs(&self) -> &[NetJob] {
        self.net.sent()
    }

    fn timers(&mut self) {
        self.play_hold();
        self.track_play();
        while let Some((title, detail)) = self.net.heard() {
            self.say(title, detail);
        }
        self.cheevos_tick();
        self.clock_tick();
        let offer = self.undo_label();
        if let Some(p) = &mut self.polaroids {
            p.set_undo(offer);
        }
        if self.doze_expired() {
            self.on_doze_timeout();
        }
        if self.now() >= self.autosave_at {
            self.autosave();
        }
        if self.now() >= self.card_check_at {
            self.card_check_at = self.now() + CARD_CHECK_MS;
            self.card_read_only = self.power.as_ref().is_some_and(|p| p.card_read_only());
        }
        let failing = self.card_failing.load(Ordering::Relaxed) || self.card_read_only;
        if failing != self.hud.alarmed() {
            eprintln!(
                "slot: card: alarm {} (read-only {})",
                if failing { "on" } else { "off" },
                self.card_read_only
            );
        }
        self.hud.set_alarm(failing, self.now());
        if self.now() >= self.battery_at {
            self.battery_at = self.now() + BATTERY_POLL_MS;
            self.battery = self.power.as_ref().and_then(|p| p.battery());
            if let Some(b) = self.battery {
                self.on_battery(b);
            }
        }
        if self.now() >= self.headphones_at {
            self.headphones_at = self.now() + HEADPHONES_POLL_MS;
            let on = self.power.as_ref().is_some_and(|p| p.headphones());
            if on != self.headphones {
                self.headphones = on;
                self.hud.set_headphones(on);
                let now = self.now();
                self.hud
                    .show(HudKind::Volume, self.output_volume(), self.muted(), now);
            }
        }
        if self.now() >= self.charge_at {
            self.charge_at = self.now() + CHARGE_POLL_MS;
            if let (Some(power), Some(b)) = (self.power.as_ref(), self.battery.as_mut()) {
                b.charge = power.charge();
            }
            let state = self.led_state();
            self.set_led(state);
        }
        if let Some(GameMenu::Linked {
            since,
            opened: false,
            ..
        }) = self.game_menu
        {
            if self.now().saturating_sub(since) >= LINKED_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(GameMenu::Unplug { since, .. }) = self.game_menu {
            if self.now().saturating_sub(since) >= UNPLUG_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(at) = self.link.as_ref().and_then(|s| s.lost_at) {
            if self.now().saturating_sub(at) >= LINK_LOST_MS {
                self.end_link();
            }
        }
    }

    pub fn led_state(&self) -> LedState {
        if matches!(self.phase, Phase::Doze { .. }) {
            return LedState::Off;
        }
        let Some(b) = self.battery else {
            return LedState::Running;
        };
        match b.charge {
            Charge::Charging => LedState::Charging,
            Charge::Full => LedState::Charged,
            _ if b.percent <= BATTERY_LOW => LedState::Low,
            _ => LedState::Running,
        }
    }

    fn set_led(&mut self, state: LedState) {
        if self.shutting_down() && state != LedState::Off {
            return;
        }
        if self.last_led == Some(state) {
            return;
        }
        self.last_led = Some(state);
        if let Some(power) = self.power.as_mut() {
            power.set_led(state);
        }
    }

    fn record_cart(&mut self, cart: Option<String>) {
        let platform = self.seated_cart().map(|c| c.platform);
        if self.state.cart == cart && self.state.cart_platform == platform {
            return;
        }
        self.state.cart = cart;
        self.state.cart_platform = platform;
        self.persist();
    }

    fn persist(&self) {
        let Some(root) = &self.root else {
            return;
        };
        self.state_writer.flush();
        let written = write_slot_state(root, &self.state);
        if let Err(e) = &written {
            eprintln!("slot: slot.state: {e}");
        }
        self.card_failing.store(written.is_err(), Ordering::Relaxed);
    }

    fn persist_soon(&self) {
        if let Some(root) = &self.root {
            self.state_writer.save(root, &self.state);
        }
    }

    pub fn flush_state(&self) {
        self.state_writer.flush();
    }

    pub fn on_core_ready(&mut self) {
        let Phase::Inserting {
            cart, core_ready, ..
        } = &mut self.phase
        else {
            return;
        };
        *core_ready = true;
        let cart = cart.clone();
        self.retire_refused_resume(&cart);
    }

    fn retire_refused_resume(&mut self, stem: &str) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if snapshot.resume_trusted() {
            return;
        }
        if !self.named_core {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
        let ring = StateRing::new(root, self.platform, self.core, stem);
        match ring.retire_resume(&format_stamp(self.wall_secs())) {
            Ok(Some(to)) => eprintln!(
                "slot: resume: {} refused this state, moved it to {}",
                self.core.as_str(),
                to.display()
            ),
            Ok(None) => {}
            Err(e) => eprintln!("slot: resume: could not move the refused state aside: {e}"),
        }
    }

    pub fn on_core_failed(&mut self) {
        let caught = self.seat();
        let Phase::Inserting { cart, .. } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.refuse_out(cart, caught);
    }

    fn refuse_out(&mut self, cart: String, caught: f32) {
        let t = (1.0 - caught) * EJECT_S;
        self.phase = Phase::Ejecting { cart, t };
        self.refused_from = Some(t);
    }

    pub fn refuse(&mut self) {
        self.refusal = Some(Refusal::started(self.now()));
    }

    pub fn refusal_active(&self, now: Millis) -> bool {
        self.refusal.is_some_and(|r| r.active(now))
    }

    pub fn seat(&self) -> f32 {
        match &self.phase {
            Phase::Shelf => 0.0,
            Phase::Inserting { t, resumed, .. } => {
                if *resumed {
                    1.0
                } else {
                    (t / SEATED_AT).clamp(0.0, 1.0)
                }
            }
            Phase::Ejecting { t, .. } => 1.0 - (t / EJECT_S).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        if let Some(index) = self.power_menu.filter(|_| !self.socket_look()) {
            self.draw_power_menu(index, out);
            return;
        }
        if self.shutting_down() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            });
            let which = if self.restarting {
                PowerChoice::Restart
            } else {
                PowerChoice::PowerOff
            };
            if let Some((tex, w, h)) = self.shutdown_faces.get(which.index()).copied() {
                out.push(Draw::Tex {
                    x: ((OUT_W - w) / 2) as f32,
                    y: ((OUT_H - h) / 2) as f32,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        match &self.phase {
            Phase::SetClock {
                picker, from_menu, ..
            } => {
                let (line, hint) = match self.clock_faces {
                    Some((line, hint)) => (Some(line), Some(hint)),
                    None => (None, None),
                };
                let back = self
                    .quick_menu_faces
                    .as_ref()
                    .filter(|_| *from_menu)
                    .map(|f| f.legend[0]);
                picker.draw(line, hint, back, out);
                return;
            }
            Phase::QuickMenu { row } if self.socket_look() => self.draw_settings(*row, out),
            Phase::QuickMenu { row } => QuickMenu {
                row: *row,
                values: QuickRow::ALL.map(|r| self.quick_value(r)),
                clock: self.quick_clock_faces,
                faces: self.quick_menu_faces.as_ref(),
            }
            .draw(out),
            Phase::Shelf if self.customize.is_some() => self.draw_customize(out),
            Phase::Shelf if self.socket_look() => {
                let screen = self.socket_screen(0.0);
                screen.draw_backdrop(out);
                self.shelf().draw_socket(self.now(), 0.0, out);
                screen.draw_pills(out);
            }
            Phase::Inserting { .. } | Phase::Ejecting { .. } if self.socket_look() => {
                let seat = self.seat();
                let screen = self.socket_screen(seat);
                screen.draw_backdrop(out);
                self.shelf().draw_socket(self.now(), seat, out);
                screen.draw_pills(out);
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, ease(seat)],
                });
            }
            Phase::Playing { .. } if self.screen < 1.0 && self.socket_look() => {
                self.push_game(out);
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, 1.0 - self.screen],
                });
            }
            Phase::Shelf => {
                draw_backdrop(self.wallpaper, out);
                match (self.core_picker_shown(), self.selected_stem()) {
                    (Some(picker), Some(stem)) => {
                        let open = ease(picker.openness(self.now()));
                        let dim = 1.0 + (CORE_PICKER_DIM - 1.0) * open;
                        self.shelf()
                            .draw_row(Some(stem), 0.0, CORE_PICKER_RECEDE * open, dim, out);
                        draw_empty_slot(out);
                    }
                    _ => {
                        self.shelf().draw(self.shelf_shake(), out);
                        draw_slot_name(self.shelf_platform, self.slot_name_alpha(), out);
                    }
                }
                draw_footer(
                    self.battery,
                    self.battery_percent,
                    self.bolt,
                    self.shelf_clock,
                    out,
                );
            }
            Phase::About => {
                draw_backdrop(self.wallpaper, out);
                draw_sticker(self.sticker_face, out);
                return;
            }
            Phase::Inserting { cart, resumed, .. } => {
                if !resumed {
                    draw_backdrop(self.wallpaper, out);
                    self.shelf()
                        .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                }
                self.chrome(cart, self.seat(), out);
            }
            Phase::Ejecting { cart, .. } => {
                draw_backdrop(self.wallpaper, out);
                self.shelf()
                    .draw_row(Some(cart), 0.0, self.seat(), 1.0, out);
                self.chrome(cart, self.seat(), out);
            }
            Phase::Playing { cart } if self.screen < 1.0 => self.chrome(cart, 0.0, out),
            Phase::Playing { .. } => self.push_game(out),
            Phase::Polaroids { .. } => {
                self.push_game(out);
                if let Some(p) = &self.polaroids {
                    p.draw(
                        self.battery,
                        self.battery_percent,
                        self.bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            Phase::Doze { .. } => {
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, 1.0],
                });
                return;
            }
            Phase::Search => slot_ui::SearchScreen {
                faces: &self.search_faces,
                row: self.search.row,
                col: self.search.col,
                pick: (!self.search.hits.is_empty()).then_some(self.search.pick),
                picked_ms: self.now().saturating_sub(self.search_picked_at),
                light: self.socket_look(),
            }
            .draw(out),
            Phase::Password { keys, .. } => slot_ui::SearchScreen {
                faces: &self.search_faces,
                row: keys.row,
                col: keys.col,
                pick: Some(1),
                picked_ms: 0,
                light: true,
            }
            .draw(out),
            Phase::Networks { .. } => {
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: OUT_W as f32,
                    h: OUT_H as f32,
                    colour: [0.2, 0.2, 0.2, 1.0],
                });
                let (_, shown, focus) = self.networks_window();
                slot_ui::SocketPopup {
                    chrome: &self.socket_chrome,
                    faces: &self.settings_faces,
                    options: shown.max(1),
                    focus: focus.unwrap_or(usize::MAX),
                }
                .draw(out);
            }
        }
        if let Some(picker) = self.core_picker_shown() {
            self.draw_core_picker(&picker, out);
        }
        if let Some(menu) = self.game_menu {
            self.draw_game_menu(menu, out);
        }
        if let Some(at) = self
            .customize
            .and_then(|c| c.panel.popup)
            .filter(|_| matches!(self.phase, Phase::Shelf) && self.power_menu.is_none())
        {
            slot_ui::SocketPopup {
                chrome: &self.socket_chrome,
                faces: &self.settings_faces,
                options: slot_ui::Material::ALL.len(),
                focus: at,
            }
            .draw(out);
        }
        if let Some(focus) = self
            .resume_popup
            .filter(|_| matches!(self.phase, Phase::Shelf) && self.power_menu.is_none())
        {
            slot_ui::SocketPopup {
                chrome: &self.socket_chrome,
                faces: &self.settings_faces,
                options: 2,
                focus,
            }
            .draw(out);
        }
        if let Some(focus) = self
            .core_popup
            .filter(|_| matches!(self.phase, Phase::Shelf) && self.power_menu.is_none())
        {
            slot_ui::SocketPopup {
                chrome: &self.socket_chrome,
                faces: &self.settings_faces,
                options: Core::ALL.len(),
                focus,
            }
            .draw(out);
        }
        if let Some(focus) = self.power_menu {
            slot_ui::SocketPopup {
                chrome: &self.socket_chrome,
                faces: &self.settings_faces,
                options: PowerChoice::ALL.len(),
                focus,
            }
            .draw(out);
        }
        if let Some(panel) = self.home {
            slot_ui::HomeScreen {
                faces: &self.home_faces,
                panel,
                on: self.home_tiles_on(),
                levels: [
                    f32::from(self.state.brightness) / f32::from(BRIGHTNESS_MAX),
                    match self.muted() {
                        true => 0.0,
                        false => f32::from(self.level()) / f32::from(VOLUME_MAX),
                    },
                ],
                light: self.socket_look(),
            }
            .draw(out);
        }
        self.hud.draw(self.now(), out);
    }

    pub fn screen_shake(&self) -> f32 {
        self.shake_at(self.now())
    }

    pub fn shake_at(&self, now: Millis) -> f32 {
        self.shake_when(matches!(self.phase, Phase::Playing { .. }), now)
    }

    pub fn shelf_shake(&self) -> f32 {
        self.shake_when(self.on_shelf() && self.core_picker.is_none(), self.now())
    }

    fn shake_when(&self, mine: bool, now: Millis) -> f32 {
        if !mine {
            return 0.0;
        }
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    fn push_game(&self, out: &mut Vec<Draw>) {
        if self.game_visible() {
            out.push(Draw::Game);
        }
    }

    fn chrome(&self, stem: &str, dim: f32, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf().find(stem) else {
            return;
        };
        let alpha = self.alert_alpha();
        let (rest, scale) = self.shelf().selected_at();
        SlotChrome {
            cart,
            face,
            rest,
            scale,
            seat: self.seat(),
            alert: self.alert_face.filter(|_| alpha > 0.0).map(|t| (t, alpha)),
            dim,
            screen: self.screen,
            game: self.game_ready,
        }
        .draw(out);
    }

    pub fn alert_visible(&self) -> bool {
        self.alert_alpha() > 0.0
    }

    pub fn alert_alpha(&self) -> f32 {
        let (Some(from), Phase::Ejecting { t, .. }) = (self.refused_from, &self.phase) else {
            return 0.0;
        };
        let span = EJECT_S - from;
        if span <= 0.0 {
            return 0.0;
        }
        let u = ((t - from) / span).clamp(0.0, 1.0);
        ((ALERT_GONE - u) / (ALERT_GONE - ALERT_HOLD)).clamp(0.0, 1.0)
    }

    pub fn set_alert_face(&mut self, face: TexId) {
        self.alert_face = Some(face);
    }

    pub fn set_shutdown_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shutdown_faces = faces;
    }

    pub fn set_power_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.power_menu_faces = faces;
    }

    pub fn set_core_board_faces(&mut self, board: TexId, lid: TexId) {
        self.core_board_face = Some(board);
        self.core_lid_face = Some(lid);
        self.core_faces_stem = self.selected_stem().map(str::to_string);
    }

    pub fn set_core_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
        shadow: TexId,
    ) {
        self.core_socket_faces = sockets;
        self.core_chip_faces = chips;
        self.core_blank_chip_face = Some(blank);
        self.core_chip_shadow_face = Some(shadow);
    }

    pub fn set_core_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.core_legend_faces = faces;
    }

    pub fn set_link_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_menu_faces = faces;
    }

    pub fn set_link_linked_face(&mut self, face: (TexId, u32, u32)) {
        self.link_linked_face = Some(face);
    }

    pub fn set_link_step_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_step_faces = faces;
    }

    pub fn set_link_fail_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_fail_faces = faces;
    }

    pub fn set_link_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.link_legend_faces = faces;
    }

    pub fn set_link_sprites(&mut self, sprites: LinkSprites) {
        self.link_sprites = Some(sprites);
    }

    pub fn link_sprites_ready(&self) -> bool {
        self.link_sprites.is_some()
    }

    fn draw_power_menu(&self, index: usize, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        draw_menu_rows(
            &self.power_menu_faces,
            Some(index),
            centred_top(self.power_menu_faces.len()),
            out,
        );
    }

    fn core_picker_shown(&self) -> Option<CorePicker> {
        self.core_picker.filter(|p| !p.waiting())
    }

    fn draw_core_picker(&self, picker: &CorePicker, out: &mut Vec<Draw>) {
        let now = self.now();
        let progress = picker.openness(now);
        let lift = lift_of(progress);
        let (rest, scale) = self.shelf().selected_at();
        let shelf = shelf_cart_at(rest, scale);
        let board = board_from(shelf, progress);
        let zoom = board_zoom(board);
        let ready = self.core_faces_ready();

        if ready {
            if let Some(tex) = self.core_board_face {
                out.push(Draw::Tex {
                    x: board.x,
                    y: board.y,
                    w: board.w,
                    h: board.h,
                    tex,
                    alpha: 1.0,
                });
            }
            for (i, tex) in self.core_socket_faces.iter().copied().enumerate() {
                let (x, y) = on_board(board, SOCKET_U[i], SOCKET_V);
                out.push(Draw::Tex {
                    x: x.round(),
                    y: y.round(),
                    w: SOCKET_W as f32 * zoom,
                    h: SOCKET_H as f32 * zoom,
                    tex,
                    alpha: 1.0,
                });
            }

            let chip = picker.chip(now);
            let u = CHIP_U[0] + (CHIP_U[1] - CHIP_U[0]) * chip.across;
            if chip.lift > 0.0 {
                if let Some(tex) = self.core_chip_shadow_face {
                    let (cx, cy) = on_board(board, u + 19.0, CHIP_V + 29.4);
                    let (w, h) = (SHADOW_W as f32 * zoom, SHADOW_H as f32 * zoom);
                    out.push(Draw::Tex {
                        x: cx - w / 2.0,
                        y: cy - h / 2.0,
                        w,
                        h,
                        tex,
                        alpha: 0.6 * chip.lift * lift,
                    });
                }
            }
            let face = match chip.seated {
                Some(core) => self.core_chip_faces.get(core.index()).copied(),
                None => self.core_blank_chip_face,
            };
            if let Some(tex) = face {
                let (x, y) = on_board(board, u, CHIP_V - HOP_LIFT * chip.lift);
                let body = Placed {
                    x: x + chip.shake,
                    y,
                    w: CHIP_W as f32 * zoom,
                    h: CHIP_H as f32 * zoom,
                };
                let at = grown(body, TURN_PAD as f32 * zoom);
                out.push(Draw::Turned {
                    x: at.x.round(),
                    y: at.y.round(),
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn: chip.tip,
                });
            }
        }

        if let Some(tex) = self.core_chip_shadow_face {
            let (lid, _) = lid_from(shelf, progress);
            let k = lid.w / lid_at(1.0).0.w;
            let (w, h) = (LID_SHADOW_W * k, LID_SHADOW_H * k);
            out.push(Draw::Tex {
                x: lid.x + (lid.w - w) / 2.0,
                y: lid.y + lid.h + LID_SHADOW_DROP * k - h / 2.0,
                w,
                h,
                tex,
                alpha: LID_SHADOW_ALPHA * lift,
            });
        }

        if ready {
            if let Some(tex) = self.core_lid_face {
                let (lid, turn) = lid_from(shelf, progress);
                let at = grown(lid, TURN_PAD as f32 * lid.w / CART_W as f32);
                out.push(Draw::Turned {
                    x: at.x,
                    y: at.y,
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn,
                });
            }
        } else if let Some((_, Some(tex))) = self
            .selected_stem()
            .and_then(|stem| self.shelf().find(stem))
        {
            let (lid, turn) = lid_from(shelf, progress);
            out.push(Draw::Turned {
                x: lid.x,
                y: lid.y,
                w: lid.w,
                h: lid.h,
                tex,
                alpha: 1.0,
                turn,
            });
        }

        if let [cancel, swap, choose] = self.core_legend_faces.as_slice() {
            let right = BOARD_X + BOARD_W as f32;
            let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
            for (tex, w, x) in [
                (cancel.0, cancel.1, BOARD_X),
                (swap.0, swap.1, (OUT_W as f32 - seen(swap.1)) / 2.0),
                (choose.0, choose.1, right - seen(choose.1)),
            ] {
                out.push(Draw::Tex {
                    x: x.round(),
                    y: CORE_LEGEND_Y,
                    w: w as f32,
                    h: HINT_H as f32,
                    tex,
                    alpha: lift,
                });
            }
        }
    }

    fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            colour: slot_ui::opening(),
        });
        if let Some(sprites) = &self.link_sprites {
            crate::link_screen::draw_link_art(menu, self.link_hardware, self.now(), sprites, out);
        }
        let line = match menu {
            GameMenu::Pick(role) => self.link_menu_faces.get(role.index()).copied(),
            GameMenu::Working { step, .. } => self
                .link_step_faces
                .get(step.shown(self.radio.warmed()).index())
                .copied(),
            GameMenu::Linked { .. } => self.link_linked_face,
            GameMenu::Failed { fail, .. } => fail
                .shown()
                .and_then(|i| self.link_fail_faces.get(i).copied()),
            GameMenu::Unplug { .. } => None,
        };
        if let Some((tex, w, h)) = line {
            out.push(Draw::Tex {
                x: ((OUT_W as f32 - w as f32) / 2.0).round(),
                y: LINK_TEXT_Y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
        let switchable = self.seated().is_some_and(|stem| self.link_switchable(stem));
        let keys: &[LinkLegend] = match menu {
            GameMenu::Pick(_) if switchable => &[
                LinkLegend::Cancel,
                LinkLegend::Mode,
                LinkLegend::Swap,
                LinkLegend::Link,
            ],
            GameMenu::Pick(_) => &[LinkLegend::Cancel, LinkLegend::Swap, LinkLegend::Link],
            GameMenu::Working { .. } => &[LinkLegend::Cancel],
            GameMenu::Linked { opened: true, .. } => &[LinkLegend::Back, LinkLegend::EndLink],
            GameMenu::Linked { .. } => &[],
            GameMenu::Failed { .. } => &[LinkLegend::Ok],
            GameMenu::Unplug { .. } => &[],
        };
        let faces: Vec<(TexId, u32)> = keys
            .iter()
            .filter_map(|k| self.link_legend_faces.get(k.index()).copied())
            .collect();
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let total: f32 = faces.iter().map(|(_, w)| seen(*w)).sum::<f32>()
            + LINK_LEGEND_GAP * faces.len().saturating_sub(1) as f32;
        let mut x = ((OUT_W as f32 - total) / 2.0).round();
        for (tex, w) in faces {
            out.push(Draw::Tex {
                x,
                y: LINK_LEGEND_Y,
                w: w as f32,
                h: HINT_H as f32,
                tex,
                alpha: 1.0,
            });
            x += (seen(w) + LINK_LEGEND_GAP).round();
        }
    }

    fn auto_link(&self, stem: &str) -> Option<(&Cart, LinkKind)> {
        let cart = self.shelf().carts.iter().find(|c| c.stem == stem)?;
        let auto = link_kind(&cart.code, &cart.title, slot_store::header_clean(&cart.rom));
        Some((cart, auto))
    }

    fn on_shelf(&self) -> bool {
        matches!(self.phase, Phase::Shelf)
    }

    pub fn browsed_rom(&self) -> Option<&std::path::Path> {
        if !self.on_shelf() {
            return None;
        }
        let shelf = self.shelf();
        shelf.carts.get(shelf.index).map(|c| c.rom.as_path())
    }

    fn insert(&mut self, clean: bool) {
        if !self.on_shelf() {
            return;
        }
        let at = self.shelf().index;
        let Some(cart) = self.shelf_mut().carts.get_mut(at).map(|c| {
            c.read_header();
            c.stem.clone()
        }) else {
            return;
        };
        eprintln!(
            "slot: launch: insert {cart} at {:.1} ms",
            crate::boot_time::ms()
        );
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Inserting {
            cart,
            t: 0.0,
            core_ready: false,
            resumed: false,
            clean,
        };
    }

    pub fn starting_clean(&self) -> bool {
        matches!(self.phase, Phase::Inserting { clean: true, .. })
    }

    fn play_hold(&mut self) {
        let Some(at) = self.play_held else {
            return;
        };
        if !self.on_shelf() {
            self.play_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.insert(true);
        }
    }

    fn eject(&mut self) {
        if self.single_cart() {
            return self.refuse();
        }
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Inserting { cart, .. } => std::mem::take(cart),
            _ => return,
        };
        self.end_link();
        self.close_game_menu();
        self.flush_eject(&cart);
        self.pending = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Ejecting {
            cart,
            t: -EJECT_HOLD_S,
        };
    }

    fn flush_eject(&mut self, stem: &str) {
        self.settle_saves();
        let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: eject: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "eject");
        let state = state.filter(|_| self.state.eject_save);
        match persist::eject_as(
            root,
            self.platform,
            self.core,
            stem,
            state.as_deref(),
            sav.as_deref(),
            self.state.save_format,
        ) {
            Ok(()) => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.card_failing.store(false, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("slot: eject: {e}");
                self.card_failing.store(true, Ordering::Relaxed);
            }
        }
    }

    fn doze(&mut self) {
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.core_picker = None;
        if matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.state_writer.flush();
        self.flush_resume();
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(std::mem::take(cart)),
            _ => None,
        };
        self.polaroids = None;
        eprintln!("slot: doze at {:.1} ms", crate::boot_time::ms());
        self.phase = Phase::Doze { cart };
        self.dozed_at = self.now();
        self.set_led(LedState::Off);
        self.radio.ask(RadioJob::Cool);
        if let Some(power) = &mut self.power {
            power.on_close();
        }
    }

    fn wake(&mut self) {
        let Phase::Doze { cart } = &mut self.phase else {
            return;
        };
        eprintln!("slot: wake at {:.1} ms", crate::boot_time::ms());
        self.phase = match cart.take() {
            Some(cart) => Phase::Playing { cart },
            None => Phase::Shelf,
        };
        let lit = self.led_state();
        self.set_led(lit);
        if let Some(power) = &mut self.power {
            power.on_open();
        }
    }

    pub fn on_doze_timeout(&mut self) {
        if !matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        if self.power.as_ref().is_some_and(|p| p.can_deep_sleep()) {
            if matches!(
                self.power.as_ref().map(|p| p.charge()),
                Some(Charge::Charging)
            ) {
                self.dozed_at = self.now();
                return;
            }
            self.settle_saves();
            eprintln!("slot: deep sleep at {:.1} ms", crate::boot_time::ms());
            let slept = self.power.as_mut().is_some_and(|p| p.deep_sleep());
            if slept {
                return self.resumed();
            }
        }
        self.begin_power_off();
    }

    /// Back from deep sleep: the screen comes on, and the press that woke
    /// the handheld is not taken as a new one.
    pub fn resumed(&mut self) {
        eprintln!("slot: back from deep sleep");
        self.power_quiet_until = self.now() + WAKE_GRACE_MS;
        self.wake();
    }

    fn power_press(&mut self) {
        if std::mem::take(&mut self.woke_on_press) {
            return;
        }
        match self.phase {
            Phase::Doze { .. } => self.wake(),
            _ => self.doze(),
        }
    }

    fn open_power_menu(&mut self) {
        if self.power_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        self.flush_resume();
        self.power_menu = Some(0);
    }

    fn power_menu_input(&mut self, action: Action) {
        let Some(index) = self.power_menu else {
            return;
        };
        let last = PowerChoice::ALL.len() - 1;
        match action {
            Action::GbaDown(Btn::Up) => {
                self.power_menu = Some(if index == 0 { last } else { index - 1 })
            }
            Action::GbaDown(Btn::Down) => {
                self.power_menu = Some(if index >= last { 0 } else { index + 1 })
            }
            Action::GbaDown(Btn::B) => self.power_menu = None,
            Action::GbaDown(Btn::A) => {
                self.power_menu = None;
                self.close_game_menu();
                match PowerChoice::ALL[index] {
                    PowerChoice::Restart => {
                        self.restarting = true;
                        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
                        self.set_led(LedState::Off);
                    }
                    PowerChoice::PowerOff => self.begin_power_off(),
                }
            }
            _ => {}
        }
    }

    fn open_core_picker(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        if self.socket_look() {
            return self.open_core_popup(&root);
        }
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        if cart.platform != Platform::Gba {
            return;
        }
        let seat = slot_store::core_for(&root, &cart.stem);
        let now = self.now();
        let mut picker = CorePicker::open(seat, now);
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    /// The core choice as a Socket popup: the cores in a list, the cursor on
    /// the one the game runs on now.
    fn open_core_popup(&mut self, root: &Path) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        if cart.platform != Platform::Gba {
            return;
        }
        let now = slot_store::core_for(root, &cart.stem);
        self.core_popup = Core::ALL.iter().position(|c| *c == now);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    fn core_popup_input(&mut self, action: Action) {
        let Some(at) = self.core_popup else {
            return;
        };
        let n = Core::ALL.len();
        match action {
            Action::GbaDown(Btn::Up) => self.core_popup = Some((at + n - 1) % n),
            Action::GbaDown(Btn::Down) => self.core_popup = Some((at + 1) % n),
            Action::GbaDown(Btn::A) => {
                self.write_core(Core::ALL[at]);
                self.core_popup = None;
                self.say(format!("Plays on {}", Core::ALL[at].text()), "");
            }
            Action::GbaDown(Btn::B) | Action::GbaDown(Btn::Start) => self.core_popup = None,
            _ => {}
        }
    }

    /// Puts the selected game in, first asking whether to pick up where it
    /// was left or start from its save, when there is a choice to make.
    fn insert_or_ask(&mut self) {
        if !self.state.eject_save {
            return self.insert(true);
        }
        if self.state.ask_resume && self.selected_has_resume() {
            self.shelf_mut().release_hold();
            self.play_held = None;
            self.resume_popup = Some(0);
            return;
        }
        self.insert(false);
    }

    fn selected_has_resume(&self) -> bool {
        let (Some(root), Some(cart)) = (&self.root, self.shelf().carts.get(self.shelf().index))
        else {
            return false;
        };
        let core = slot_store::core_for_platform(root, &cart.stem, cart.platform);
        StateRing::new(root, cart.platform, core, &cart.stem).has_resume()
            && !persist::save_replaced(root, cart.platform, &cart.stem)
    }

    fn resume_popup_input(&mut self, action: Action) {
        let Some(at) = self.resume_popup else {
            return;
        };
        match action {
            Action::GbaDown(Btn::Up) | Action::GbaDown(Btn::Down) => {
                self.resume_popup = Some(1 - at)
            }
            Action::GbaDown(Btn::A) => {
                self.resume_popup = None;
                self.insert(at == 1);
            }
            Action::GbaDown(Btn::B) => self.resume_popup = None,
            _ => {}
        }
    }

    pub fn resume_popup_open(&self) -> bool {
        self.resume_popup.is_some()
    }

    pub fn core_popup_open(&self) -> bool {
        self.core_popup.is_some()
    }

    fn core_faces_ready(&self) -> bool {
        self.core_faces_stem
            .as_deref()
            .is_some_and(|stem| self.selected_stem() == Some(stem))
    }

    fn core_picker_input(&mut self, action: Action) {
        let press = match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => Press::Left,
            Action::GbaDown(Btn::Right) | Action::ShelfRight => Press::Right,
            Action::GbaDown(Btn::A) => Press::Keep,
            Action::GbaDown(Btn::B) => Press::Back,
            _ => return,
        };
        let now = self.now();
        let Some(picker) = &mut self.core_picker else {
            return;
        };
        let outcome = picker.press(press, now);
        if let Outcome::Write(core) = outcome {
            self.write_core(core);
        }
    }

    fn open_game_menu(&mut self) {
        if self.game_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        if self.core == Core::Mgba {
            let wireless = self
                .seated()
                .is_some_and(|stem| self.link_mode(stem).0 == LinkKind::Wireless);
            if wireless {
                self.hud.toast(Toast::NeedsGpsp, self.now());
                return;
            }
            self.link_hardware = LinkKind::Cable;
            self.radio.ask(RadioJob::Warm);
            self.game_menu = Some(GameMenu::Pick(self.last_role));
            return;
        }
        if self.platform != Platform::Gba {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        let carried = self
            .seated()
            .and_then(|stem| self.auto_link(stem))
            .is_some_and(|(cart, _)| link_carried(&cart.code, &cart.title));
        if !carried {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        if self.core != Core::Gpsp {
            self.hud.toast(Toast::NeedsGpsp, self.now());
            return;
        }
        let hardware = self
            .seated()
            .map_or(LinkKind::Cable, |stem| self.link_mode(stem).0);
        self.link_hardware = hardware;
        self.radio.ask(RadioJob::Warm);
        self.game_menu = Some(GameMenu::Pick(self.last_role));
    }

    fn game_menu_shortcut(&mut self) {
        let Some(client_id) = self.link_client_id() else {
            return self.open_game_menu();
        };
        let now = self.now();
        self.game_menu = Some(GameMenu::Linked {
            role: LinkRow::from_client_id(client_id),
            worked: now,
            since: now,
            opened: true,
        });
    }

    fn end_link_from_menu(&mut self) {
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::LinkEnded, self.now());
        self.unplug(role);
    }

    fn unplug(&mut self, role: LinkRow) {
        self.game_menu = Some(GameMenu::Unplug {
            role,
            since: self.now(),
        });
    }

    pub fn set_radio_jobs(&mut self, jobs: Box<dyn RadioJobs>) {
        self.radio = jobs;
    }

    fn game_menu_input(&mut self, action: Action) {
        let Some(menu) = self.game_menu else {
            return;
        };
        match menu {
            GameMenu::Pick(role) => match action {
                Action::GbaDown(Btn::Left) | Action::GbaDown(Btn::Right) => {
                    self.last_role = role.other();
                    self.game_menu = Some(GameMenu::Pick(role.other()));
                }
                Action::GbaDown(Btn::Select) => self.switch_hardware(),
                Action::GbaDown(Btn::A) => self.pick_link(role),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Working { .. } => {
                if action == Action::GbaDown(Btn::B) {
                    if let Some(starting) = &mut self.starting {
                        starting.starter.cancel();
                    }
                    if let Some(reload) = &mut self.reload {
                        reload.cancelled = true;
                    }
                }
            }
            GameMenu::Linked { opened: true, .. } => match action {
                Action::GbaDown(Btn::A) => self.end_link_from_menu(),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Linked { .. } => {}
            GameMenu::Failed { .. } => {
                if matches!(
                    action,
                    Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::GameMenu
                ) {
                    self.close_game_menu();
                }
            }
            GameMenu::Unplug { .. } => {}
        }
    }

    pub fn start_link(&mut self, starter: LinkStarter, client_id: u16) {
        self.start_link_from(starter, client_id, self.now());
    }

    fn start_link_from(&mut self, starter: LinkStarter, client_id: u16, since: Millis) {
        if let Some(mut old) = self.starting.replace(LinkStarting { starter, client_id }) {
            old.starter.cancel();
        }
        self.game_menu = Some(GameMenu::Working {
            role: LinkRow::from_client_id(client_id),
            step: LinkStep::Radio,
            since,
        });
    }

    fn switch_hardware(&mut self) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        if !self.link_switchable(&stem) {
            return self.refuse();
        }
        let other = self.link_hardware.other();
        self.link_hardware = other;
        self.link_choices.insert(stem, other);
    }

    fn link_switchable(&self, stem: &str) -> bool {
        self.auto_link(stem).is_some_and(|(cart, auto)| {
            serial_option(self.link_hardware.other(), auto, &cart.code, &cart.title)
                != serial_option(self.link_hardware, auto, &cart.code, &cart.title)
        })
    }

    fn pick_link(&mut self, role: LinkRow) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let loaded = self.link_loaded.unwrap_or("auto");
        if self.core == Core::Mgba {
            let player = role.client_id() as u8;
            if self.link_player == Some(player) {
                return self.start_link(
                    LinkStarter::spawn(role.role(), link_port()),
                    role.client_id(),
                );
            }
            if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
                return self.refuse();
            }
            self.link_player = Some(player);
            self.link_reload = Some((stem.clone(), loaded));
            self.reload = Some(Reload {
                stem,
                role,
                cancelled: false,
                from: self.link_hardware,
                from_serial: loaded,
                fallback: false,
            });
            self.game_menu = Some(GameMenu::Working {
                role,
                step: LinkStep::Radio,
                since: self.now(),
            });
            return;
        }
        let (_, serial) = self.link_mode(&stem);
        if serial == loaded {
            return self.start_link(
                LinkStarter::spawn(role.role(), link_port()),
                role.client_id(),
            );
        }
        if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
            return self.refuse();
        }
        self.link_reload = Some((stem.clone(), serial));
        self.reload = Some(Reload {
            stem,
            role,
            cancelled: false,
            from: self.link_hardware.other(),
            from_serial: loaded,
            fallback: false,
        });
        self.game_menu = Some(GameMenu::Working {
            role,
            step: LinkStep::Radio,
            since: self.now(),
        });
    }

    fn refuse_seated(&mut self) {
        self.close_game_menu();
        self.pending = None;
        let caught = self.seat();
        let cart = match &mut self.phase {
            Phase::Playing { cart } => Some(std::mem::take(cart)),
            Phase::Doze { cart } => {
                *cart = None;
                None
            }
            _ => None,
        };
        if let Some(cart) = cart {
            self.refuse_out(cart, caught);
        }
    }

    fn close_game_menu(&mut self) {
        self.game_menu = None;
        if let Some(mut starting) = self.starting.take() {
            starting.starter.cancel();
        }
        if !self.link_active() {
            self.radio.ask(RadioJob::Cool);
        }
        let uncollected =
            self.link_reload.is_some() && self.reload.as_ref().is_some_and(|r| !r.fallback);
        if uncollected {
            self.link_reload = None;
            self.reload = None;
        } else if let Some(reload) = &mut self.reload {
            reload.cancelled = true;
        }
    }

    fn working_role(&self, client_id: u16) -> (LinkRow, Millis) {
        match self.game_menu {
            Some(GameMenu::Working { role, since, .. }) => (role, since),
            _ => (LinkRow::from_client_id(client_id), self.now()),
        }
    }

    fn poll_link(&mut self) {
        if self.power_menu.is_some() {
            return;
        }
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        match starting.starter.poll() {
            None => self.starting = Some(starting),
            Some(LinkProgress::At(step)) => {
                if let Some(GameMenu::Working { role, since, .. }) = self.game_menu {
                    self.game_menu = Some(GameMenu::Working { role, step, since });
                }
                self.starting = Some(starting);
            }
            Some(LinkProgress::Ready(link)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Linked {
                    role,
                    worked,
                    since: self.now(),
                    opened: false,
                });
                self.begin_link(starting.client_id);
                self.link_transport = Some((starting.client_id, Box::new(link)));
            }
            Some(LinkProgress::Failed(LinkFail::Cancelled)) => self.game_menu = None,
            Some(LinkProgress::Failed(fail)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Failed {
                    role,
                    fail,
                    worked,
                    since: self.now(),
                });
            }
        }
    }

    fn write_core(&self, core: Core) {
        let (Some(root), Some(cart)) = (
            self.root.clone(),
            self.shelf().carts.get(self.shelf().index),
        ) else {
            return;
        };
        if let Err(e) = slot_store::write_selected_core(&root, &cart.stem, core) {
            eprintln!("slot: core: could not write selected_core.txt: {e}");
        }
    }

    fn begin_power_off(&mut self) {
        if self.powering_off {
            return;
        }
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.powering_off = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    pub fn on_battery(&mut self, b: Battery) {
        if b.percent > BATTERY_CRITICAL || self.powering_off {
            return;
        }
        if matches!(b.charge, Charge::Charging | Charge::Full) {
            return;
        }
        self.flush_resume();
        self.begin_power_off();
    }

    fn doze_expired(&self) -> bool {
        if self.link_active() {
            return false;
        }
        let (Phase::Doze { .. }, Some(power)) = (&self.phase, &self.power) else {
            return false;
        };
        self.now().saturating_sub(self.dozed_at) >= power.timeout().as_millis() as Millis
    }

    pub fn settle_saves(&mut self) {
        self.state_writer.flush();
        let secs = std::mem::take(&mut self.play_ms) / 1000;
        if let (Some(root), Some(key), true) = (&self.root, &self.play_key, secs > 0) {
            if let Err(e) = crate::playtime::add(root, key, secs) {
                eprintln!("slot: playtime {key}: {e}");
            }
        }
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
    }

    fn autosave(&mut self) {
        self.autosave_at = self.now() + AUTOSAVE_MS;
        self.settle_saves();
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: autosave: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "autosave");
        let (root, platform, core, cart) =
            (root.clone(), self.platform, self.core, cart.to_owned());
        let format = self.state.save_format;
        let failing = self.card_failing.clone();
        let write = move || {
            let written = persist::flush_as(
                &root,
                platform,
                core,
                &cart,
                state.as_deref(),
                sav.as_deref(),
                format,
            );
            if let Err(e) = &written {
                eprintln!("slot: autosave: {e}");
            }
            failing.store(written.is_err(), Ordering::Relaxed);
        };
        match std::thread::Builder::new()
            .name("slot-autosave".into())
            .spawn(write)
        {
            Ok(h) => self.pending_save = Some(h),
            Err(e) => eprintln!("slot: autosave: no writer thread: {e}"),
        }
    }

    pub fn flush_resume(&mut self) {
        self.settle_saves();
        self.autosave_at = self.now() + AUTOSAVE_MS;
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: flush: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "flush");
        let written = persist::flush_as(
            root,
            self.platform,
            self.core,
            cart,
            state.as_deref(),
            sav.as_deref(),
            self.state.save_format,
        );
        if let Err(e) = &written {
            eprintln!("slot: flush: {e}");
        }
        self.card_failing.store(written.is_err(), Ordering::Relaxed);
    }

    fn ring(&self) -> Option<StateRing> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        Some(StateRing::new(root, self.platform, self.core, cart))
    }

    fn seated(&self) -> Option<&str> {
        match &self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(cart),
            _ => None,
        }
    }

    fn entries(&self) -> Vec<StateEntry> {
        self.ring().and_then(|r| r.list().ok()).unwrap_or_default()
    }

    fn open_polaroids(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let entries = self.entries();
        if entries.is_empty() {
            return self.refuse();
        }
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        let mut p = Polaroids::new(entries);
        p.set_undo(self.undo_label());
        self.polaroids = Some(p);
        self.phase = Phase::Polaroids { cart };
        self.push_hint_faces();
    }

    fn close_polaroids(&mut self) {
        let Phase::Polaroids { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.polaroids = None;
        self.phase = Phase::Playing { cart };
    }

    fn load_selected(&mut self) {
        let state = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.state.clone());
        let refused = state.map(|state| self.load_file(&state)) == Some(false);
        if !refused {
            self.close_polaroids();
        }
    }

    fn delete_selected(&mut self) {
        let stamp = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.clone());
        let (Some(stamp), Some(ring)) = (stamp, self.ring()) else {
            return;
        };
        if let Err(e) = ring.remove(&stamp) {
            eprintln!("slot: delete: {e}");
            return;
        }
        if self.undo_targets(&stamp) {
            self.pending = None;
        }
        let Some(p) = &mut self.polaroids else {
            return;
        };
        p.remove_selected();
        if p.is_empty() {
            self.close_polaroids();
        }
    }

    fn undo_targets(&self, stamp: &str) -> bool {
        match self.pending.as_ref() {
            Some((PendingUndo::Save { stamp: pending, .. }, _)) => pending == stamp,
            _ => false,
        }
    }

    fn load_newest(&mut self) {
        let Some(newest) = self.entries().first().map(|e| e.state.clone()) else {
            return self.refuse();
        };
        self.load_file(&newest);
    }

    fn load_file(&mut self, state: &Path) -> bool {
        if !self.may_load_state() {
            self.refuse();
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        let bytes = match std::fs::read(state) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("slot: load: {e}");
                return false;
            }
        };
        let prior = snapshot.state();
        snapshot.load(bytes);
        self.hud.toast(Toast::StateLoaded, self.now());
        if let Some(prior) = prior {
            self.pending = Some((PendingUndo::Load { prior }, self.now()));
        }
        true
    }

    fn screenshot(&mut self) {
        let (Some(root), Some(snapshot), Some(stem)) = (
            self.root.clone(),
            &self.snapshot,
            self.seated().map(str::to_owned),
        ) else {
            return;
        };
        let Some(frame) = snapshot.thumb() else {
            eprintln!("slot: screenshot: the core gave up no frame");
            return;
        };
        let (w, h) = self.platform.picture();
        let png = crate::thumb::crop_png(&frame, w, h).unwrap_or(frame);
        let dir = root.join(SCREENSHOTS);
        let name = format!("{stem} {}.png", format_stamp(self.wall_secs()));
        let written = std::fs::create_dir_all(&dir)
            .and_then(|()| slot_store::atomic_write(&dir.join(&name), &png));
        match written {
            Ok(()) => self.say("Screenshot saved", format!("{SCREENSHOTS}/{name}")),
            Err(e) => eprintln!("slot: screenshot: {e}"),
        }
    }

    fn save_state(&mut self) {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return;
        };
        if !snapshot.resume_trusted() {
            eprintln!("slot: save: the core refused the resume it was given, not pushing a state");
            return self.refuse();
        }
        let Some(state) = snapshot.state() else {
            eprintln!("slot: save: the core gave up no state");
            return;
        };
        let thumb = snapshot.thumb().unwrap_or_default();
        let stamp = free_stamp(&ring, self.wall_secs());
        let evicted = doomed(&ring);
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: save: {e}");
            return;
        }
        self.hud.toast(Toast::StateSaved, self.now());
        self.pending = Some((PendingUndo::Save { stamp, evicted }, self.now()));
    }

    pub fn undo_available(&self, now: Millis) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_sub(*at) <= UNDO_GRACE_MS)
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        if !self.undo_available(self.now()) {
            return None;
        }
        match self.pending.as_ref()?.0 {
            PendingUndo::Save { .. } => Some("undo save"),
            PendingUndo::Load { .. } => Some("undo load"),
        }
    }

    pub fn set_legend_faces(&mut self, faces: Vec<TexId>) {
        self.legend_faces = faces;
        self.push_hint_faces();
    }

    pub fn set_undo_face(&mut self, face: Option<TexId>) {
        self.undo_face = face;
        self.push_hint_faces();
    }

    fn push_hint_faces(&mut self) {
        let mut faces = self.legend_faces.clone();
        faces.extend(self.undo_face);
        if let Some(p) = &mut self.polaroids {
            p.set_hint_faces(faces);
        }
    }

    pub fn set_icon_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_icons(faces);
    }

    pub fn set_toast_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_toasts(faces);
    }

    pub fn toast(&self) -> Option<Toast> {
        self.hud.said(self.now())
    }

    pub fn card_alarm(&self) -> bool {
        self.hud.alarmed()
    }

    pub fn undo(&mut self, now: Millis) {
        if !self.undo_available(now) {
            self.pending = None;
            return;
        }
        if matches!(&self.pending, Some((PendingUndo::Load { .. }, _))) && !self.may_load_state() {
            return self.refuse();
        }
        let Some((what, _)) = self.pending.take() else {
            return;
        };
        match what {
            PendingUndo::Save { stamp, evicted } => self.undo_save(&stamp, evicted),
            PendingUndo::Load { prior } => {
                if let Some(snapshot) = &self.snapshot {
                    snapshot.load(prior);
                }
            }
        }
        self.close_polaroids();
    }

    fn undo_save(&self, stamp: &str, evicted: Option<(String, Vec<u8>, Vec<u8>)>) {
        let Some(ring) = self.ring() else {
            return;
        };
        if let Err(e) = ring.remove(stamp) {
            eprintln!("slot: undo: {e}");
            return;
        }
        let Some((stamp, state, thumb)) = evicted else {
            return;
        };
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: undo: {e}");
        }
    }

    pub fn polaroid_entries(&self) -> &[StateEntry] {
        match &self.polaroids {
            Some(p) => &p.entries,
            None => &[],
        }
    }

    pub fn set_polaroid_faces(&mut self, faces: Vec<TexId>) {
        if let Some(p) = &mut self.polaroids {
            p.set_faces(faces);
        }
    }

    pub fn polaroid_stamp(&self) -> Option<&str> {
        self.polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.as_str())
    }

    pub fn polaroid_title(&self, now: &str) -> String {
        self.polaroids
            .as_ref()
            .map_or_else(String::new, |p| p.title(now))
    }

    pub fn set_polaroid_title_face(&mut self, face: TexId) {
        if let Some(p) = &mut self.polaroids {
            p.set_title_face(Some(face));
        }
    }
}

fn trusted_write(
    snapshot: &dyn Snapshot,
    state: Vec<u8>,
    verb: &str,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let state = if snapshot.resume_trusted() {
        Some(state)
    } else {
        eprintln!(
            "slot: {verb}: the core refused the resume it was given, not overwriting the saved one"
        );
        None
    };
    let sav = snapshot.save_ram();
    let sav = if snapshot.save_ram_trusted() {
        sav
    } else {
        if sav.is_some() {
            eprintln!(
                "slot: {verb}: the core refused the save ram it was given, not overwriting the saved one"
            );
        }
        None
    };
    (state, sav)
}

fn up(level: u8, step: u8, max: u8) -> u8 {
    level.saturating_add(step).min(max)
}

fn shader_value(shader: Shader) -> QuickValue {
    match shader {
        Shader::Off => QuickValue::Off,
        Shader::Lcd3x => QuickValue::Lcd3x,
        Shader::Grid => QuickValue::Grid,
        Shader::Dot => QuickValue::Dot,
        Shader::Simpletex => QuickValue::Simpletex,
    }
}

fn ff_next(from: u8, right: bool) -> u8 {
    let n = FF_SPEEDS.len();
    let at = FF_SPEEDS.iter().position(|&v| v == from).unwrap_or(0);
    FF_SPEEDS[if right {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }]
}

fn clock_screen(utc: i64, offset_min: i16, from_menu: bool) -> Phase {
    Phase::SetClock {
        picker: ClockPicker::local(utc, offset_min),
        seed: utc - utc.rem_euclid(60),
        from_menu,
    }
}

fn system_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn doomed(ring: &StateRing) -> Option<(String, Vec<u8>, Vec<u8>)> {
    let entries = ring.list().ok()?;
    let oldest = entries.get(RING_MAX - 1)?;
    let (state, thumb) = ring.read(&oldest.stamp).ok()?;
    Some((oldest.stamp.clone(), state, thumb))
}

fn free_stamp(ring: &StateRing, now: i64) -> String {
    let taken: Vec<String> = ring
        .list()
        .map(|l| l.into_iter().map(|e| e.stamp).collect())
        .unwrap_or_default();
    let mut secs = now;
    let mut stamp = format_stamp(secs);
    while taken.contains(&stamp) {
        secs += 1;
        stamp = format_stamp(secs);
    }
    stamp
}

fn centred_top(rows: usize) -> f32 {
    (OUT_H as f32 - POWER_MENU_PITCH * rows as f32) / 2.0
}

fn draw_menu_rows(
    faces: &[(TexId, u32, u32)],
    index: Option<usize>,
    top: f32,
    out: &mut Vec<Draw>,
) {
    for (row, (tex, w, h)) in faces.iter().copied().enumerate() {
        let y = top + POWER_MENU_PITCH * row as f32;
        let x = ((OUT_W as f32 - w as f32) / 2.0).round();
        if index == Some(row) {
            out.push(Draw::Rect {
                x,
                y: y + POWER_MENU_BAR_INSET,
                w: w as f32,
                h: POWER_MENU_PITCH - 2.0 * POWER_MENU_BAR_INSET,
                colour: slot_ui::edge(),
            });
        }
        out.push(Draw::Tex {
            x,
            y: y + (POWER_MENU_PITCH - h as f32) / 2.0,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }
}

/// The search screen's slots, filled for typing a network's password.
fn password_lines(ssid: &str, keys: &crate::keyboard::Keyboard) -> Vec<(String, bool)> {
    let mut lines = vec![(String::new(), false); slot_ui::SEARCH_SLOTS];
    lines[slot_ui::SEARCH_QUERY] = (format!("Join {ssid}"), true);
    lines[slot_ui::search_hit_slot(0)] = ("Password".into(), false);
    lines[slot_ui::search_hit_slot(1)] = (format!("{}_", keys.text), true);
    let layers: Vec<String> = crate::keyboard::LAYER_NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| match i == keys.layer {
            true => format!("[{n}]"),
            false => n.to_string(),
        })
        .collect();
    lines[slot_ui::search_hit_slot(3)] = (format!("Y  {}", layers.join("  ")), false);
    for (row, ks) in crate::keyboard::LAYERS[keys.layer].iter().enumerate() {
        for (col, k) in ks.iter().enumerate() {
            let lit = (row, col) == (keys.row, keys.col);
            lines[slot_ui::search_key_slot(row, col)] = (k.to_string(), lit);
        }
    }
    lines[slot_ui::SEARCH_LEGEND] = (
        "A type   B delete   X space   Y shift   START join".into(),
        false,
    );
    lines
}
