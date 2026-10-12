//! Emulator settings: the core options a system's games start with, and the
//! ones a single game overrides them with, after NX Redux's Emulator
//! Settings and Emulator Options.
//!
//! `Config/emulator/<SYSTEM>.ini` holds a system's defaults and
//! `Config/emulator/<SYSTEM>/<game>.ini` a game's own, each `key=value` a
//! line. The options each emulator offers are remembered in
//! `Config/emulator/<core>.options` when it loads, so they can be listed
//! without a game running.

use std::path::{Path, PathBuf};

use crate::{atomic_write, Core, Platform};

const DIR: &str = "Config/emulator";

/// One option a core offers: its key, what it is called and the values it
/// takes, the first being its default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreOption {
    pub key: String,
    pub desc: String,
    pub values: Vec<String>,
}

impl CoreOption {
    pub fn default_value(&self) -> &str {
        self.values.first().map_or("", String::as_str)
    }

    /// The value after (or before) `now`, wrapping round.
    pub fn step(&self, now: &str, right: bool) -> String {
        let n = self.values.len();
        if n == 0 {
            return now.to_string();
        }
        let at = self.values.iter().position(|v| v == now).unwrap_or(0);
        self.values[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
        .clone()
    }
}

/// Which settings a change is kept in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    System(Platform),
    Game(Platform, String),
}

impl Scope {
    pub fn platform(&self) -> Platform {
        match self {
            Scope::System(p) | Scope::Game(p, _) => *p,
        }
    }
}

fn path(root: &Path, scope: &Scope) -> PathBuf {
    let dir = root.join(DIR);
    match scope {
        Scope::System(p) => dir.join(format!("{}.ini", p.dir_name())),
        Scope::Game(p, stem) => dir.join(p.dir_name()).join(format!("{stem}.ini")),
    }
}

fn options_path(root: &Path, core: Core) -> PathBuf {
    root.join(DIR).join(format!("{}.options", core.as_str()))
}

fn parse(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() || k.starts_with('#') {
            continue;
        }
        out.retain(|(old, _)| old != k);
        out.push((k.to_string(), v.to_string()));
    }
    out
}

/// The options kept in one scope, in the order they were written.
pub fn read(root: &Path, scope: &Scope) -> Vec<(String, String)> {
    std::fs::read_to_string(path(root, scope))
        .map(|t| parse(&t))
        .unwrap_or_default()
}

/// Keeps `options` as the whole of a scope's settings; none at all removes
/// its file.
pub fn write(root: &Path, scope: &Scope, options: &[(String, String)]) -> std::io::Result<()> {
    let file = path(root, scope);
    if options.is_empty() {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text: String = options.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
    atomic_write(&file, text.as_bytes())
}

/// Sets one option in a scope, or forgets it with `None`.
pub fn set(root: &Path, scope: &Scope, key: &str, value: Option<&str>) -> std::io::Result<()> {
    let mut all = read(root, scope);
    all.retain(|(k, _)| k != key);
    if let Some(v) = value {
        all.push((key.to_string(), v.to_string()));
    }
    write(root, scope, &all)
}

/// What a game starts with: its system's settings, then its own on top.
pub fn for_game(root: &Path, platform: Platform, stem: &str) -> Vec<(String, String)> {
    let mut out = read(root, &Scope::System(platform));
    for (k, v) in read(root, &Scope::Game(platform, stem.to_string())) {
        out.retain(|(old, _)| *old != k);
        out.push((k, v));
    }
    out
}

/// Remembers the options a core offers.
pub fn remember_options(root: &Path, core: Core, options: &[CoreOption]) -> std::io::Result<()> {
    if options.is_empty() {
        return Ok(());
    }
    let file = options_path(root, core);
    let text: String = options
        .iter()
        .map(|o| {
            format!(
                "{}\t{}\t{}\n",
                o.key,
                o.desc.replace(['\t', '\n'], " "),
                o.values.join("|")
            )
        })
        .collect();
    if std::fs::read_to_string(&file).is_ok_and(|old| old == text) {
        return Ok(());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&file, text.as_bytes())
}

/// The options a core offered when it last loaded.
pub fn known_options(root: &Path, core: Core) -> Vec<CoreOption> {
    let Ok(text) = std::fs::read_to_string(options_path(root, core)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let key = parts.next()?.trim();
            let desc = parts.next()?.trim();
            let values = parts.next()?;
            (!key.is_empty()).then(|| CoreOption {
                key: key.to_string(),
                desc: if desc.is_empty() { key } else { desc }.to_string(),
                values: values.split('|').map(str::to_string).collect(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt(key: &str, values: &[&str]) -> CoreOption {
        CoreOption {
            key: key.into(),
            desc: format!("The {key}"),
            values: values.iter().map(|v| v.to_string()).collect(),
        }
    }

    #[test]
    fn a_game_s_own_settings_win_over_its_system_s() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        set(root, &Scope::System(Platform::Gba), "mgba_skip", Some("2")).unwrap();
        set(root, &Scope::System(Platform::Gba), "mgba_idle", Some("on")).unwrap();
        let game = Scope::Game(Platform::Gba, "Emerald".into());
        set(root, &game, "mgba_skip", Some("0")).unwrap();
        let got = for_game(root, Platform::Gba, "Emerald");
        assert_eq!(
            got,
            [
                ("mgba_idle".to_string(), "on".to_string()),
                ("mgba_skip".to_string(), "0".to_string())
            ]
        );
        assert_eq!(for_game(root, Platform::Gb, "Emerald"), []);
        set(root, &game, "mgba_skip", None).unwrap();
        assert!(!path(root, &game).exists(), "an empty game file is kept");
    }

    #[test]
    fn known_options_come_back_as_they_were() {
        let d = tempfile::tempdir().unwrap();
        let list = vec![opt("a", &["x", "y"]), opt("b", &["1", "2", "3"])];
        remember_options(d.path(), Core::Mgba, &list).unwrap();
        assert_eq!(known_options(d.path(), Core::Mgba), list);
        assert!(known_options(d.path(), Core::Gpsp).is_empty());
    }

    #[test]
    fn a_value_steps_round() {
        let o = opt("a", &["x", "y", "z"]);
        assert_eq!(o.step("z", true), "x");
        assert_eq!(o.step("x", false), "z");
        assert_eq!(o.step("nope", true), "y");
        assert_eq!(o.default_value(), "x");
    }
}
