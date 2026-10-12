//! The in-game menu and the emulator settings, after NX Redux: MENU in a
//! game pauses it on a list (resume, save and load a state, options, quit),
//! and Options holds the emulator's own options, the shader, cheats and the
//! controls, kept for the game or its whole system by Save Changes.
//! Settings > Gameplay > Emulator Settings edits a system's defaults.

use slot_input::{Action, Btn};
use slot_store::emulator::{self, CoreOption, Scope};
use slot_store::{cheats::Cheat, Platform, Shader};

use super::{popup_window, App, Phase};

/// Keys of the frontend's own settings kept beside the emulator's options.
pub(super) const SHADER_KEY: &str = "slot_shader";
pub(super) const SWAP_AB_KEY: &str = "slot_swap_ab";
pub(super) const TURBO_KEY: &str = "slot_turbo";

/// One list of the menu; the menu is a stack of them, B going back down.
#[derive(Clone, Debug)]
pub(super) enum Menu {
    Pause {
        at: usize,
    },
    Options {
        at: usize,
    },
    /// The emulator's options: of the game running, or a system's defaults
    /// from Settings.
    Core {
        scope: Scope,
        list: Vec<CoreOption>,
        values: Vec<String>,
        at: usize,
    },
    Shader {
        at: usize,
    },
    Cheats {
        at: usize,
    },
    Controls {
        at: usize,
    },
    Save {
        at: usize,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum PauseRow {
    Resume,
    LeftOff,
    SaveState,
    LoadState,
    Options,
    Quit,
}

const OPTIONS_ROWS: [&str; 5] = [
    "Emulator Options",
    "Shader",
    "Cheats",
    "Controls",
    "Save Changes",
];

/// What the running game was given, and what has been changed since.
#[derive(Default, Debug)]
pub(super) struct Running {
    pub options: Vec<CoreOption>,
    pub values: Vec<String>,
    /// Keys changed from what the game started with, or loaded from its
    /// saved settings: the ones Save Changes keeps.
    pub touched: Vec<String>,
    pub shader: Option<Shader>,
    pub swap_ab: bool,
    pub turbo: Option<bool>,
    pub cheats: Vec<Cheat>,
    /// Option changes and cheats for the emulator, not yet sent.
    pub outbox: Vec<(String, String)>,
    pub cheats_dirty: bool,
}

fn on_off(on: bool) -> &'static str {
    if on {
        "On"
    } else {
        "Off"
    }
}

/// An option's name cut to fit a row.
fn short(text: &str, max: usize) -> String {
    match text.chars().count() > max {
        true => format!("{}\u{2026}", text.chars().take(max - 1).collect::<String>()),
        false => text.to_string(),
    }
}

impl App {
    pub fn menu_open(&self) -> bool {
        !self.menu.is_empty()
    }

    /// The menu's lists, deepest last, as names: for tests.
    pub fn menu_depth(&self) -> usize {
        self.menu.len()
    }

    fn game_stem(&self) -> Option<String> {
        match &self.phase {
            Phase::Playing { cart } => Some(cart.clone()),
            _ => None,
        }
    }

    fn pause_rows(&self) -> Vec<PauseRow> {
        let mut rows = vec![PauseRow::Resume];
        if self.left_off.is_some() {
            rows.push(PauseRow::LeftOff);
        }
        rows.extend([
            PauseRow::SaveState,
            PauseRow::LoadState,
            PauseRow::Options,
            PauseRow::Quit,
        ]);
        rows
    }

    pub(super) fn open_pause(&mut self) {
        let at = usize::from(self.left_off.is_some());
        self.menu = vec![Menu::Pause { at }];
    }

    /// Settings > Gameplay > Emulator Settings: a system's defaults.
    pub(super) fn open_emulator_settings(&mut self, platform: Platform) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let core = slot_store::Core::default_for(platform);
        let list = emulator::known_options(&root, core);
        let saved = emulator::read(&root, &Scope::System(platform));
        let values = list
            .iter()
            .map(|o| {
                saved
                    .iter()
                    .find(|(k, _)| *k == o.key)
                    .map_or_else(|| o.default_value().to_string(), |(_, v)| v.clone())
            })
            .collect();
        self.menu = vec![Menu::Core {
            scope: Scope::System(platform),
            list,
            values,
            at: 0,
        }];
    }

    /// Called once a game's emulator is open: what it offers, the values
    /// it has, and the settings saved for the game and its system.
    pub fn core_opened(
        &mut self,
        options: Vec<CoreOption>,
        values: Vec<String>,
        saved: Vec<(String, String)>,
        cheats: Vec<Cheat>,
    ) {
        let flag = |key: &str| {
            saved
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v == "true" || v == "on")
        };
        let shader = saved
            .iter()
            .find(|(k, _)| k == SHADER_KEY)
            .and_then(|(_, v)| Shader::parse(v));
        self.running = Running {
            touched: saved
                .iter()
                .map(|(k, _)| k.clone())
                .filter(|k| !k.starts_with("slot_"))
                .collect(),
            options,
            values,
            shader,
            swap_ab: flag(SWAP_AB_KEY).unwrap_or(false),
            turbo: flag(TURBO_KEY),
            cheats_dirty: cheats.iter().any(|c| c.enabled),
            cheats,
            outbox: Vec::new(),
        };
    }

    /// Option changes for the running emulator, to send it.
    pub fn take_option_changes(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.running.outbox)
    }

    /// The cheats to turn on, when they have changed.
    pub fn take_cheats(&mut self) -> Option<Vec<String>> {
        if !std::mem::take(&mut self.running.cheats_dirty) {
            return None;
        }
        Some(
            self.running
                .cheats
                .iter()
                .filter(|c| c.enabled)
                .map(|c| c.code.clone())
                .collect(),
        )
    }

    pub fn swap_ab(&self) -> bool {
        self.running.swap_ab
    }

    pub(super) fn game_shader(&self) -> Option<Shader> {
        self.running.shader
    }

    pub(super) fn game_turbo(&self) -> Option<bool> {
        self.running.turbo
    }

    fn shaders(&self) -> &'static [Shader] {
        match self.platform {
            Platform::Gba => &Shader::GBA,
            Platform::Gb | Platform::Gbc => &Shader::GB,
        }
    }

    /// The title and rows of the list on top.
    pub(super) fn menu_text(&self) -> Option<(String, Vec<String>, usize, String)> {
        let top = self.menu.last()?;
        let key = |k: &str, label: &str| format!("{k}{}{label}", slot_ui::KEY_SEP);
        let back = key("B", "Back");
        Some(match top {
            Menu::Pause { at } => {
                let rows = self
                    .pause_rows()
                    .iter()
                    .map(|r| {
                        match r {
                            PauseRow::Resume => "Resume",
                            PauseRow::LeftOff => "Load where you left off",
                            PauseRow::SaveState => "Save State",
                            PauseRow::LoadState => "Load State",
                            PauseRow::Options => "Options",
                            PauseRow::Quit => "Quit Game",
                        }
                        .to_string()
                    })
                    .collect();
                let name = self
                    .seated_cart()
                    .map(slot_ui::label_text)
                    .unwrap_or_else(|| "Paused".into());
                (name, rows, *at, key("B", "Resume"))
            }
            Menu::Options { at } => {
                let shader = super::shader_value(self.screen_shader()).text();
                let rows = OPTIONS_ROWS
                    .iter()
                    .map(|r| match *r {
                        "Shader" => format!("Shader   {shader}"),
                        r => r.to_string(),
                    })
                    .collect();
                ("Options".into(), rows, *at, back)
            }
            Menu::Core {
                scope,
                list,
                values,
                at,
            } => {
                let title = match scope {
                    Scope::System(p) => format!("{} Emulator", p.dir_name()),
                    Scope::Game(..) => "Emulator Options".into(),
                };
                let rows = match list.is_empty() {
                    true => vec!["Play a game once to list these".into()],
                    false => list
                        .iter()
                        .zip(values)
                        .map(|(o, v)| format!("{}   {}", short(&o.desc, 22), short(v, 14)))
                        .collect(),
                };
                let hint = match scope {
                    Scope::System(_) => key(
                        "B",
                        "Back    \u{2190}\u{2192}  Change    Y  Default    L/R  System",
                    ),
                    Scope::Game(..) => key("B", "Back    \u{2190}\u{2192}  Change    Y  Default"),
                };
                (title, rows, *at, hint)
            }
            Menu::Shader { at } => {
                let now = self.screen_shader();
                let rows = self
                    .shaders()
                    .iter()
                    .map(|s| {
                        let name = super::shader_value(*s).text();
                        match *s == now {
                            true => format!("{name}  \u{2713}"),
                            false => name.to_string(),
                        }
                    })
                    .collect();
                ("Shader".into(), rows, *at, back)
            }
            Menu::Cheats { at } => {
                let rows = match self.running.cheats.is_empty() {
                    true => vec![format!(
                        "Put {}.cht in Cheats/{}",
                        short(&self.game_stem().unwrap_or_default(), 18),
                        self.platform.dir_name()
                    )],
                    false => self
                        .running
                        .cheats
                        .iter()
                        .map(|c| {
                            let mark = if c.enabled { "\u{2713}" } else { "\u{25cb}" };
                            format!("{mark}  {}", short(&c.desc, 30))
                        })
                        .collect(),
                };
                ("Cheats".into(), rows, *at, key("B", "Back    A  On/Off"))
            }
            Menu::Controls { at } => {
                let turbo = self.game_turbo().unwrap_or(self.state.turbo);
                let rows = vec![
                    format!("Swap A and B   {}", on_off(self.running.swap_ab)),
                    format!("Turbo on X / Y   {}", on_off(turbo)),
                ];
                ("Controls".into(), rows, *at, back)
            }
            Menu::Save { at } => {
                let system = self.platform.dir_name();
                let rows = vec![
                    "Save for This Game".into(),
                    format!("Save for All {system} Games"),
                    "Reset This Game to Defaults".into(),
                ];
                ("Save Changes".into(), rows, *at, back)
            }
        })
    }

    /// The rows the popup shows and which is under the cursor.
    pub(super) fn menu_window(&self) -> (usize, usize, Option<usize>) {
        match self.menu_text() {
            Some((_, rows, at, _)) => popup_window(rows.len(), at),
            None => (0, 0, None),
        }
    }

    fn menu_len(&self) -> usize {
        self.menu_text().map_or(0, |(_, rows, _, _)| rows.len())
    }

    fn menu_at(&mut self) -> Option<&mut usize> {
        Some(match self.menu.last_mut()? {
            Menu::Pause { at }
            | Menu::Options { at }
            | Menu::Core { at, .. }
            | Menu::Shader { at }
            | Menu::Cheats { at }
            | Menu::Controls { at }
            | Menu::Save { at } => at,
        })
    }

    pub(super) fn menu_input(&mut self, action: Action) {
        let n = self.menu_len().max(1);
        match action {
            Action::GbaDown(Btn::Up) => {
                if let Some(at) = self.menu_at() {
                    *at = (*at + n - 1) % n;
                }
            }
            Action::GbaDown(Btn::Down) => {
                if let Some(at) = self.menu_at() {
                    *at = (*at + 1) % n;
                }
            }
            Action::GbaDown(Btn::B) => {
                self.menu.pop();
            }
            Action::QuickMenu | Action::GbaDown(Btn::Menu) => self.menu.clear(),
            Action::GbaDown(Btn::A) => self.menu_pick(),
            Action::GbaDown(Btn::Left) => self.menu_step(false),
            Action::GbaDown(Btn::Right) => self.menu_step(true),
            Action::GbaDown(Btn::Y) => self.menu_default(),
            Action::GbaDown(Btn::L1) => self.menu_system(false),
            Action::GbaDown(Btn::R1) => self.menu_system(true),
            Action::Eject => {
                self.menu.clear();
                self.eject();
            }
            Action::Polaroids if matches!(self.phase, Phase::Playing { .. }) => {
                self.menu.clear();
                self.open_polaroids();
            }
            _ => {}
        }
    }

    fn menu_pick(&mut self) {
        let Some(top) = self.menu.last().cloned() else {
            return;
        };
        match top {
            Menu::Pause { at } => {
                let Some(row) = self.pause_rows().get(at).copied() else {
                    return;
                };
                match row {
                    PauseRow::Resume => self.menu.clear(),
                    PauseRow::LeftOff => {
                        self.menu.clear();
                        if let Some(bytes) = self.left_off.clone() {
                            self.load_bytes(bytes);
                        }
                    }
                    PauseRow::SaveState => {
                        self.menu.clear();
                        self.save_state();
                    }
                    PauseRow::LoadState => {
                        self.menu.clear();
                        self.open_polaroids();
                    }
                    PauseRow::Options => self.menu.push(Menu::Options { at: 0 }),
                    PauseRow::Quit => {
                        self.menu.clear();
                        self.eject();
                    }
                }
            }
            Menu::Options { at } => match OPTIONS_ROWS[at.min(OPTIONS_ROWS.len() - 1)] {
                "Emulator Options" => {
                    let Some(stem) = self.game_stem() else {
                        return;
                    };
                    self.menu.push(Menu::Core {
                        scope: Scope::Game(self.platform, stem),
                        list: self.running.options.clone(),
                        values: self.running.values.clone(),
                        at: 0,
                    });
                }
                "Shader" => {
                    let now = self.screen_shader();
                    let at = self.shaders().iter().position(|s| *s == now).unwrap_or(0);
                    self.menu.push(Menu::Shader { at });
                }
                "Cheats" => self.menu.push(Menu::Cheats { at: 0 }),
                "Controls" => self.menu.push(Menu::Controls { at: 0 }),
                _ => self.menu.push(Menu::Save { at: 0 }),
            },
            Menu::Core { .. } => self.menu_step(true),
            Menu::Shader { at } => {
                if let Some(s) = self.shaders().get(at).copied() {
                    self.running.shader = Some(s);
                }
                self.menu.pop();
            }
            Menu::Cheats { at } => {
                if let Some(c) = self.running.cheats.get_mut(at) {
                    c.enabled = !c.enabled;
                    self.running.cheats_dirty = true;
                    self.keep_cheats();
                }
            }
            Menu::Controls { .. } => self.menu_step(true),
            Menu::Save { at } => {
                self.save_changes(at);
                self.menu.clear();
            }
        }
    }

    /// Which cheats are on is kept in the cheat file at once, as RetroArch
    /// does.
    fn keep_cheats(&mut self) {
        let (Some(root), Some(stem)) = (self.root.clone(), self.game_stem()) else {
            return;
        };
        if let Err(e) = slot_store::cheats::write(&root, self.platform, &stem, &self.running.cheats)
        {
            self.say("Could not keep the cheats", e.to_string());
        }
    }

    fn menu_step(&mut self, right: bool) {
        let root = self.root.clone();
        match self.menu.last_mut() {
            Some(Menu::Core {
                scope,
                list,
                values,
                at,
            }) => {
                let (Some(opt), Some(value)) = (list.get(*at), values.get_mut(*at)) else {
                    return;
                };
                *value = opt.step(value, right);
                let (key, value) = (opt.key.clone(), value.clone());
                let default = opt.default_value() == value;
                match scope.clone() {
                    Scope::System(_) => {
                        if let Some(root) = root {
                            let keep = (!default).then_some(value.as_str());
                            if let Err(e) = emulator::set(&root, scope, &key, keep) {
                                eprintln!("slot: emulator settings: {e}");
                            }
                        }
                    }
                    Scope::Game(..) => self.change_running_option(key, value),
                }
            }
            Some(Menu::Controls { at }) => match *at {
                0 => self.running.swap_ab = !self.running.swap_ab,
                _ => {
                    let now = self.game_turbo().unwrap_or(self.state.turbo);
                    self.running.turbo = Some(!now);
                }
            },
            _ => {}
        }
    }

    fn change_running_option(&mut self, key: String, value: String) {
        if let Some(i) = self.running.options.iter().position(|o| o.key == key) {
            self.running.values[i] = value.clone();
        }
        if !self.running.touched.contains(&key) {
            self.running.touched.push(key.clone());
        }
        self.running.outbox.push((key, value));
    }

    /// Y puts the option under the cursor back to the emulator's default.
    fn menu_default(&mut self) {
        let Some(Menu::Core {
            list, values, at, ..
        }) = self.menu.last()
        else {
            return;
        };
        let (Some(opt), Some(now)) = (list.get(*at), values.get(*at)) else {
            return;
        };
        let target = opt.default_value().to_string();
        let tries = opt.values.len();
        let mut now = now.clone();
        for _ in 0..tries {
            if now == target {
                break;
            }
            self.menu_step(true);
            now = match self.menu.last() {
                Some(Menu::Core { values, at, .. }) => values[*at].clone(),
                _ => return,
            };
        }
    }

    /// L and R move a system's defaults to the next system.
    fn menu_system(&mut self, right: bool) {
        let Some(Menu::Core {
            scope: Scope::System(p),
            ..
        }) = self.menu.last()
        else {
            return;
        };
        let all = Platform::ALL;
        let i = all.iter().position(|x| x == p).unwrap_or(0);
        let n = all.len();
        let next = all[if right { (i + 1) % n } else { (i + n - 1) % n }];
        self.open_emulator_settings(next);
    }

    /// Keeps what was changed in the game: for the game alone (0), for its
    /// whole system (1), or forgets the game's own settings (2).
    fn save_changes(&mut self, how: usize) {
        let (Some(root), Some(stem)) = (self.root.clone(), self.game_stem()) else {
            return;
        };
        let platform = self.platform;
        let game = Scope::Game(platform, stem);
        let mut kept: Vec<(String, String)> = self
            .running
            .options
            .iter()
            .zip(&self.running.values)
            .filter(|(o, _)| self.running.touched.contains(&o.key))
            .map(|(o, v)| (o.key.clone(), v.clone()))
            .collect();
        if let Some(s) = self.running.shader {
            kept.push((SHADER_KEY.into(), s.name().into()));
        }
        if self.running.swap_ab {
            kept.push((SWAP_AB_KEY.into(), "true".into()));
        }
        if let Some(t) = self.running.turbo {
            kept.push((TURBO_KEY.into(), t.to_string()));
        }
        let done = match how {
            0 => emulator::write(&root, &game, &kept).map(|_| "Saved for this game"),
            1 => {
                let system = Scope::System(platform);
                let mut all = emulator::read(&root, &system);
                for (k, v) in &kept {
                    all.retain(|(old, _)| old != k);
                    all.push((k.clone(), v.clone()));
                }
                emulator::write(&root, &system, &all)
                    .and_then(|_| emulator::write(&root, &game, &[]))
                    .map(|_| "Saved for every game on this system")
            }
            _ => emulator::write(&root, &game, &[])
                .map(|_| "This game is back to its system's settings"),
        };
        match done {
            Ok(said) => {
                let detail = match how {
                    2 => "From the next time it starts",
                    _ => "",
                };
                self.say(said, detail);
            }
            Err(e) => self.say("Could not save the changes", e.to_string()),
        }
    }

    pub(super) fn menu_lines(&self, lines: &mut [(String, bool)]) {
        let Some((title, rows, at, hint)) = self.menu_text() else {
            return;
        };
        let (first, shown, _) = popup_window(rows.len(), at);
        lines[slot_ui::POPUP_TITLE_SLOT] = (title, true);
        for (i, row) in rows.iter().skip(first).take(shown).enumerate() {
            lines[slot_ui::option_slot(i)] = (row.clone(), true);
        }
        lines[slot_ui::POPUP_CLOSE_SLOT] = (hint, true);
    }
}
