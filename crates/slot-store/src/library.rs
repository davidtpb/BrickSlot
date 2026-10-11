//! What the player keeps about their games beyond the games themselves:
//! which are favourites, and which were played last.

use std::collections::HashSet;
use std::path::Path;

use crate::atomic::atomic_write;
use crate::platform::Platform;

pub const FAVORITES_FILE: &str = "Config/favorites.txt";
pub const RECENT_FILE: &str = "Config/recent.txt";
/// How many games the recent list keeps.
pub const RECENT_MAX: usize = 12;

/// A game named the same way wherever it is kept: its platform's folder and
/// its file's stem.
pub fn game_key(platform: Platform, stem: &str) -> String {
    format!("{}/{}", platform.dir_name(), stem)
}

pub fn read_favorites(root: &Path) -> HashSet<String> {
    std::fs::read_to_string(root.join(FAVORITES_FILE))
        .map(|t| {
            t.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn write_lines(root: &Path, file: &str, lines: &[String]) -> std::io::Result<()> {
    let path = root.join(file);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut text = lines.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    atomic_write(&path, text.as_bytes())
}

/// Stars or unstars a game, saying whether it is now a favourite.
pub fn toggle_favorite(root: &Path, platform: Platform, stem: &str) -> std::io::Result<bool> {
    let mut all = read_favorites(root);
    let key = game_key(platform, stem);
    let now = if all.remove(&key) {
        false
    } else {
        all.insert(key);
        true
    };
    let mut lines: Vec<String> = all.into_iter().collect();
    lines.sort();
    write_lines(root, FAVORITES_FILE, &lines)?;
    Ok(now)
}

/// A game played, and when, in seconds since 1970.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Played {
    pub platform: Platform,
    pub stem: String,
    pub at: i64,
}

/// The games played last, the latest first.
pub fn read_recent(root: &Path) -> Vec<Played> {
    let Ok(text) = std::fs::read_to_string(root.join(RECENT_FILE)) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|l| {
            let (at, key) = l.split_once('\t')?;
            let (dir, stem) = key.split_once('/')?;
            let platform = Platform::ALL.into_iter().find(|p| p.dir_name() == dir)?;
            Some(Played {
                platform,
                stem: stem.to_string(),
                at: at.trim().parse().ok()?,
            })
        })
        .take(RECENT_MAX)
        .collect()
}

/// Puts a game at the top of the recent list.
pub fn note_played(root: &Path, platform: Platform, stem: &str, at: i64) -> std::io::Result<()> {
    let mut list = read_recent(root);
    list.retain(|p| !(p.platform == platform && p.stem == stem));
    list.insert(
        0,
        Played {
            platform,
            stem: stem.to_string(),
            at,
        },
    );
    list.truncate(RECENT_MAX);
    let lines: Vec<String> = list
        .iter()
        .map(|p| format!("{}\t{}", p.at, game_key(p.platform, &p.stem)))
        .collect();
    write_lines(root, RECENT_FILE, &lines)
}

/// When a game was last played, if it is on the recent list.
pub fn last_played(root: &Path, platform: Platform, stem: &str) -> Option<i64> {
    read_recent(root)
        .into_iter()
        .find(|p| p.platform == platform && p.stem == stem)
        .map(|p| p.at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_favourite_toggles_and_survives_a_reread() {
        let d = tempfile::tempdir().unwrap();
        assert!(toggle_favorite(d.path(), Platform::Gba, "Emerald").unwrap());
        assert!(read_favorites(d.path()).contains("GBA/Emerald"));
        assert!(!toggle_favorite(d.path(), Platform::Gba, "Emerald").unwrap());
        assert!(read_favorites(d.path()).is_empty());
    }

    #[test]
    fn the_recent_list_keeps_the_latest_first_once_each() {
        let d = tempfile::tempdir().unwrap();
        note_played(d.path(), Platform::Gba, "Emerald", 10).unwrap();
        note_played(d.path(), Platform::Gb, "Tetris", 20).unwrap();
        note_played(d.path(), Platform::Gba, "Emerald", 30).unwrap();
        let r = read_recent(d.path());
        assert_eq!(
            r.iter().map(|p| p.stem.as_str()).collect::<Vec<_>>(),
            ["Emerald", "Tetris"]
        );
        assert_eq!(last_played(d.path(), Platform::Gba, "Emerald"), Some(30));
        for i in 0..20 {
            note_played(d.path(), Platform::Gba, &format!("G{i}"), 100 + i).unwrap();
        }
        assert_eq!(read_recent(d.path()).len(), RECENT_MAX);
    }

    #[test]
    fn a_name_with_a_slash_or_tab_free_text_reads_back() {
        let d = tempfile::tempdir().unwrap();
        note_played(d.path(), Platform::Gba, "Mario & Luigi - Superstar Saga", 5).unwrap();
        assert_eq!(
            read_recent(d.path())[0].stem,
            "Mario & Luigi - Superstar Saga"
        );
    }
}
