//! Tools > Files: the file explorer's buttons, its menus and the jobs it
//! runs. The explorer itself is in `crate::files`.

use slot_input::{Action, Btn};

use super::{popup_window, App, Phase, QuickRow};
use crate::files::{Explorer, FileJob, FileNews, FileWorker};
use crate::keyboard::Keyboard;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    Rename,
    NewFolder,
}

/// What is over the list, if anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesPopup {
    None,
    Actions { at: usize },
    Confirm { at: usize },
    Typing { purpose: Typing, keys: Keyboard },
    Info,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileAction {
    Copy,
    Cut,
    Paste,
    Rename,
    Delete,
    NewFolder,
    Hidden,
    Info,
}

/// A copy, move or delete under way: what it says it is doing, and how
/// far it has got.
pub(super) struct FilesBusy {
    pub worker: FileWorker,
    pub label: String,
    pub progress: f32,
    pub what: &'static str,
}

fn size_text(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{} KB", n >> 10),
        n => format!("{n} B"),
    }
}

impl App {
    pub(super) fn open_files(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.phase = Phase::Files {
            ex: Explorer::new(&root),
            popup: FilesPopup::None,
        };
    }

    pub fn files_open(&self) -> bool {
        matches!(self.phase, Phase::Files { .. })
    }

    pub fn files_explorer(&self) -> Option<&Explorer> {
        match &self.phase {
            Phase::Files { ex, .. } => Some(ex),
            _ => None,
        }
    }

    pub fn files_busy(&self) -> bool {
        self.files_job.is_some()
    }

    fn file_actions(ex: &Explorer) -> Vec<FileAction> {
        let chosen = ex.chosen();
        let mut out = Vec::new();
        if !chosen.is_empty() {
            out.extend([FileAction::Copy]);
            if chosen.iter().all(|p| crate::files::removable(p)) {
                out.push(FileAction::Cut);
            }
        }
        if ex.clip.is_some() && ex.here_changeable() {
            out.push(FileAction::Paste);
        }
        let one = ex.ticked.is_empty() && ex.current().is_some();
        if one && crate::files::removable(&chosen[0]) {
            out.push(FileAction::Rename);
        }
        if !chosen.is_empty() && chosen.iter().all(|p| crate::files::removable(p)) {
            out.push(FileAction::Delete);
        }
        if ex.here_changeable() {
            out.push(FileAction::NewFolder);
        }
        out.push(FileAction::Hidden);
        if one {
            out.push(FileAction::Info);
        }
        out
    }

    fn action_name(ex: &Explorer, a: FileAction) -> String {
        let n = ex.chosen().len();
        let many = |verb: &str| match n {
            1 => verb.to_string(),
            n => format!("{verb} {n} Items"),
        };
        match a {
            FileAction::Copy => many("Copy"),
            FileAction::Cut => many("Cut"),
            FileAction::Paste => match &ex.clip {
                Some(c) if c.items.len() > 1 => format!("Paste {} Items Here", c.items.len()),
                _ => "Paste Here".into(),
            },
            FileAction::Rename => "Rename".into(),
            FileAction::Delete => many("Delete"),
            FileAction::NewFolder => "New Folder".into(),
            FileAction::Hidden => match ex.hidden {
                true => "Hide Hidden Files".into(),
                false => "Show Hidden Files".into(),
            },
            FileAction::Info => "Info".into(),
        }
    }

    pub(super) fn files_input(&mut self, action: Action) {
        if let Some(job) = &self.files_job {
            if matches!(action, Action::GbaDown(Btn::B)) {
                job.worker.stop();
            }
            return;
        }
        let Phase::Files { ex, popup } = &mut self.phase else {
            return;
        };
        match popup.clone() {
            FilesPopup::None => match action {
                Action::GbaDown(Btn::Up) => ex.wrap(-1),
                Action::GbaDown(Btn::Down) => ex.wrap(1),
                Action::GbaDown(Btn::L1) | Action::GbaDown(Btn::Left) => {
                    ex.step(-(slot_ui::FILES_ROWS as i32))
                }
                Action::GbaDown(Btn::R1) | Action::GbaDown(Btn::Right) => {
                    ex.step(slot_ui::FILES_ROWS as i32)
                }
                Action::GbaDown(Btn::A) => {
                    if !ex.enter() && ex.current().is_some() {
                        *popup = FilesPopup::Actions { at: 0 };
                    }
                }
                Action::GbaDown(Btn::B) => {
                    if !ex.ticked.is_empty() {
                        ex.ticked.clear();
                    } else if !ex.up() {
                        self.phase = Phase::QuickMenu {
                            row: QuickRow::Files,
                        };
                    }
                }
                Action::GbaDown(Btn::Y) => {
                    ex.tick();
                    ex.wrap(1);
                }
                Action::GbaDown(Btn::X) => *popup = FilesPopup::Actions { at: 0 },
                Action::GbaDown(Btn::Start) => self.files_do(FileAction::Paste),
                Action::GbaDown(Btn::Select) => ex.toggle_hidden(),
                Action::QuickMenu => {
                    self.phase = Phase::QuickMenu {
                        row: QuickRow::Files,
                    }
                }
                _ => {}
            },
            FilesPopup::Actions { at } => {
                let list = Self::file_actions(ex);
                let n = list.len().max(1);
                match action {
                    Action::GbaDown(Btn::Up) => {
                        *popup = FilesPopup::Actions {
                            at: (at + n - 1) % n,
                        }
                    }
                    Action::GbaDown(Btn::Down) => *popup = FilesPopup::Actions { at: (at + 1) % n },
                    Action::GbaDown(Btn::A) => {
                        *popup = FilesPopup::None;
                        if let Some(a) = list.get(at).copied() {
                            self.files_do(a);
                        }
                    }
                    Action::GbaDown(Btn::B) | Action::GbaDown(Btn::X) | Action::QuickMenu => {
                        *popup = FilesPopup::None
                    }
                    _ => {}
                }
            }
            FilesPopup::Confirm { at } => match action {
                Action::GbaDown(Btn::Up) | Action::GbaDown(Btn::Down) => {
                    *popup = FilesPopup::Confirm { at: 1 - at }
                }
                Action::GbaDown(Btn::A) if at == 0 => {
                    *popup = FilesPopup::None;
                    let items = ex.chosen();
                    ex.ticked.clear();
                    self.files_start(FileJob::Delete { items }, "Deleting");
                }
                Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::QuickMenu => {
                    *popup = FilesPopup::None
                }
                _ => {}
            },
            FilesPopup::Info => {
                if matches!(
                    action,
                    Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::QuickMenu
                ) {
                    *popup = FilesPopup::None;
                }
            }
            FilesPopup::Typing { purpose, mut keys } => {
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
                            *popup = FilesPopup::None;
                            return;
                        }
                    }
                    Action::QuickMenu => {
                        *popup = FilesPopup::None;
                        return;
                    }
                    Action::GbaDown(Btn::Start) => {
                        let name = keys.text.clone();
                        let done = match purpose {
                            Typing::Rename => ex.rename(&name),
                            Typing::NewFolder => ex.new_folder(&name),
                        };
                        match done {
                            Ok(()) => *popup = FilesPopup::None,
                            Err(e) => {
                                let e = e.to_string();
                                *popup = FilesPopup::Typing { purpose, keys };
                                return self.say(e, "");
                            }
                        }
                        return;
                    }
                    _ => {}
                }
                *popup = FilesPopup::Typing { purpose, keys };
            }
        }
    }

    fn files_do(&mut self, a: FileAction) {
        let Phase::Files { ex, popup } = &mut self.phase else {
            return;
        };
        match a {
            FileAction::Copy | FileAction::Cut => {
                let n = ex.copy(a == FileAction::Cut);
                let verb = if a == FileAction::Cut {
                    "Cut"
                } else {
                    "Copied"
                };
                let what = if n == 1 { "1 item" } else { "items" };
                self.say(
                    format!(
                        "{verb} {}",
                        if n == 1 {
                            what.to_string()
                        } else {
                            format!("{n} {what}")
                        }
                    ),
                    "Open a folder and press START to paste",
                );
            }
            FileAction::Paste => {
                let Some(clip) = ex.clip.clone() else {
                    return;
                };
                if !ex.here_changeable() {
                    return self.say("Nothing can be pasted here", "");
                }
                let into = ex.cwd.clone();
                let label = if clip.cut { "Moving" } else { "Copying" };
                if clip.cut {
                    ex.clip = None;
                }
                self.files_start(
                    FileJob::Paste {
                        items: clip.items,
                        cut: clip.cut,
                        into,
                    },
                    label,
                );
            }
            FileAction::Rename => {
                let name = ex.current().map(|e| e.name.clone()).unwrap_or_default();
                *popup = FilesPopup::Typing {
                    purpose: Typing::Rename,
                    keys: Keyboard {
                        text: name,
                        ..Keyboard::default()
                    },
                };
            }
            FileAction::Delete => *popup = FilesPopup::Confirm { at: 1 },
            FileAction::NewFolder => {
                *popup = FilesPopup::Typing {
                    purpose: Typing::NewFolder,
                    keys: Keyboard::default(),
                }
            }
            FileAction::Hidden => ex.toggle_hidden(),
            FileAction::Info => *popup = FilesPopup::Info,
        }
    }

    fn files_start(&mut self, job: FileJob, what: &'static str) {
        let Some(root) = self.root.clone() else {
            return;
        };
        self.files_job = Some(FilesBusy {
            worker: FileWorker::start(&root, job),
            label: what.to_string(),
            progress: 0.0,
            what,
        });
    }

    /// Takes in how a running job is going, and tidies up when it ends.
    pub(super) fn files_tick(&mut self) {
        let Some(job) = &mut self.files_job else {
            return;
        };
        let mut ended = None;
        for news in job.worker.news.try_iter() {
            match news {
                FileNews::Progress { done, total, name } => {
                    job.progress = if total == 0 {
                        0.0
                    } else {
                        done as f32 / total as f32
                    };
                    job.label = format!("{} {name}", job.what);
                }
                FileNews::Done { items, errors } => ended = Some((items, errors)),
            }
        }
        let Some((items, errors)) = ended else {
            return;
        };
        let what = job.what;
        self.files_job = None;
        if let Phase::Files { ex, .. } = &mut self.phase {
            ex.refresh();
        }
        let done = match what {
            "Deleting" => "Deleted",
            "Moving" => "Moved",
            _ => "Copied",
        };
        let count = match items {
            1 => "1 item".to_string(),
            n => format!("{n} items"),
        };
        match errors.first() {
            None => self.say(format!("{done} {count}"), ""),
            Some(first) => self.say(
                format!("{done} {count}, {} failed", errors.len()),
                first.clone(),
            ),
        }
    }

    /// The text of the explorer's list, while it shows.
    pub fn files_lines(&self) -> Option<Vec<(String, bool)>> {
        let Phase::Files { ex, popup } = &self.phase else {
            return None;
        };
        if matches!(popup, FilesPopup::Typing { .. }) {
            return None;
        }
        let mut lines = vec![(String::new(), false); slot_ui::FILES_SLOTS];
        let path = match ex.cwd.as_os_str().is_empty() {
            true => "SD card".to_string(),
            false => format!("SD card/{}", ex.cwd.to_string_lossy()),
        };
        lines[slot_ui::FILES_TITLE] = (path, true);
        let status = match (&ex.clip, ex.ticked.len()) {
            (_, n) if n > 0 => format!("{n} ticked"),
            (Some(c), _) => format!(
                "{} to {}",
                c.items.len(),
                if c.cut { "move" } else { "paste" }
            ),
            _ => match ex.entries.len() {
                1 => "1 item".to_string(),
                n => format!("{n} items"),
            },
        };
        lines[slot_ui::FILES_STATUS] = (status, false);
        let (first, shown, _) = self.files_window();
        if ex.entries.is_empty() {
            lines[slot_ui::files_name_slot(0)] = ("This folder is empty".into(), false);
        }
        for (i, e) in ex.entries.iter().skip(first).take(shown).enumerate() {
            let tail = match (e.dir, e.link) {
                (_, true) => " \u{2192}",
                (true, false) => "/",
                _ => "",
            };
            let lit = first + i == ex.at;
            lines[slot_ui::files_name_slot(i)] = (format!("{}{tail}", e.name), lit);
            let size = match e.dir {
                true => String::new(),
                false => size_text(e.size),
            };
            lines[slot_ui::files_size_slot(i)] = (size, false);
        }
        let legend = match &self.files_job {
            Some(job) => format!("{}  {:.0}%      B  stop", job.label, job.progress * 100.0),
            None => {
                let paste = if ex.clip.is_some() {
                    "    START  paste"
                } else {
                    ""
                };
                format!("A  open    B  back    X  menu    Y  tick{paste}")
            }
        };
        lines[slot_ui::FILES_LEGEND] = (legend, false);
        Some(lines)
    }

    /// The first row of the folder shown, how many show, and the cursor's
    /// row among them.
    pub(super) fn files_window(&self) -> (usize, usize, Option<usize>) {
        let Phase::Files { ex, .. } = &self.phase else {
            return (0, 0, None);
        };
        let len = ex.entries.len();
        if len == 0 {
            return (0, 0, None);
        }
        let shown = len.min(slot_ui::FILES_ROWS);
        let first = ex.at.saturating_sub(shown - 1).min(len - shown);
        (first, shown, Some(ex.at - first))
    }

    /// Which of the rows shown are ticked.
    pub(super) fn files_ticked_rows(&self) -> [bool; slot_ui::FILES_ROWS] {
        let mut out = [false; slot_ui::FILES_ROWS];
        let Phase::Files { ex, .. } = &self.phase else {
            return out;
        };
        let (first, shown, _) = self.files_window();
        for (i, e) in ex.entries.iter().skip(first).take(shown).enumerate() {
            out[i] = ex.ticked.contains(&e.name);
        }
        out
    }

    pub(super) fn files_progress(&self) -> Option<f32> {
        self.files_job.as_ref().map(|j| j.progress)
    }

    /// The popup's text over the list: the menu, the delete check or an
    /// item's details.
    pub(super) fn files_popup_lines(&self, lines: &mut [(String, bool)]) {
        let Phase::Files { ex, popup } = &self.phase else {
            return;
        };
        let key = |k: &str, label: &str| format!("{k}{}{label}", slot_ui::KEY_SEP);
        let (title, rows, at): (String, Vec<String>, usize) = match popup {
            FilesPopup::Actions { at } => (
                ex.current()
                    .map(|e| e.name.clone())
                    .filter(|_| ex.ticked.is_empty())
                    .unwrap_or_else(|| format!("{} Items", ex.chosen().len())),
                Self::file_actions(ex)
                    .into_iter()
                    .map(|a| Self::action_name(ex, a))
                    .collect(),
                *at,
            ),
            FilesPopup::Confirm { at } => {
                let n = ex.chosen().len();
                let what = match n {
                    1 => ex.current().map(|e| e.name.clone()).unwrap_or_default(),
                    n => format!("{n} items"),
                };
                (
                    format!("Delete {what}?"),
                    vec!["Delete".into(), "Cancel".into()],
                    *at,
                )
            }
            FilesPopup::Info => {
                let Some(e) = ex.current() else {
                    return;
                };
                let full = ex.root().join(&ex.cwd).join(&e.name);
                let meta = std::fs::symlink_metadata(&full).ok();
                let kind = match (e.dir, e.link) {
                    (_, true) => "Link".to_string(),
                    (true, _) => format!(
                        "Folder, {} items",
                        std::fs::read_dir(&full).map(|d| d.count()).unwrap_or(0)
                    ),
                    _ => format!("File, {}", size_text(e.size)),
                };
                let when = meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| slot_ui::date_time_text(d.as_secs() as i64))
                    .unwrap_or_default();
                (
                    e.name.clone(),
                    vec![kind, format!("Changed {when}")],
                    usize::MAX,
                )
            }
            _ => return,
        };
        let (first, shown, _) = popup_window(rows.len(), at.min(rows.len().saturating_sub(1)));
        lines[slot_ui::POPUP_TITLE_SLOT] = (title, true);
        for (i, r) in rows.iter().skip(first).take(shown).enumerate() {
            lines[slot_ui::option_slot(i)] = (r.clone(), true);
        }
        let close = match popup {
            FilesPopup::Confirm { .. } => key("B", "Cancel"),
            _ => key("B", "Close"),
        };
        lines[slot_ui::POPUP_CLOSE_SLOT] = (close, true);
    }

    /// How many options the popup over the list shows, and which the
    /// cursor is on.
    pub(super) fn files_popup_window(&self) -> Option<(usize, Option<usize>)> {
        let Phase::Files { ex, popup } = &self.phase else {
            return None;
        };
        match popup {
            FilesPopup::Actions { at } => {
                let (_, shown, focus) = popup_window(Self::file_actions(ex).len(), *at);
                Some((shown, focus))
            }
            FilesPopup::Confirm { at } => Some((2, Some(*at))),
            FilesPopup::Info => Some((2, None)),
            _ => None,
        }
    }

    /// The keyboard's text while a name is typed.
    pub(super) fn files_typing_lines(&self) -> Option<Vec<(String, bool)>> {
        let Phase::Files {
            popup: FilesPopup::Typing { purpose, keys },
            ..
        } = &self.phase
        else {
            return None;
        };
        let title = match purpose {
            Typing::Rename => "Rename",
            Typing::NewFolder => "New Folder",
        };
        Some(super::keyboard_lines(title, "Name", "save", keys))
    }

    pub(super) fn files_typing(&self) -> Option<&Keyboard> {
        match &self.phase {
            Phase::Files {
                popup: FilesPopup::Typing { keys, .. },
                ..
            } => Some(keys),
            _ => None,
        }
    }

    pub(super) fn files_popup_up(&self) -> bool {
        matches!(
            &self.phase,
            Phase::Files {
                popup: FilesPopup::Actions { .. } | FilesPopup::Confirm { .. } | FilesPopup::Info,
                ..
            }
        )
    }
}
