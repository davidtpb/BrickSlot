//! Cheats in RetroArch's `.cht` format, read from `Cheats/<SYSTEM>/<game>.cht`
//! (or `Cheats/<game>.cht`), with the ones turned on written back.
//!
//! ```text
//! cheats = 2
//! cheat0_desc = "Infinite money"
//! cheat0_code = "82025BC4+FFFF"
//! cheat0_enable = false
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{atomic_write, Platform};

pub const CHEATS_DIR: &str = "Cheats";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cheat {
    pub desc: String,
    pub code: String,
    pub enabled: bool,
}

/// The file a game's cheats are in, if it has one.
pub fn file_for(root: &Path, platform: Platform, stem: &str) -> Option<PathBuf> {
    let dir = root.join(CHEATS_DIR);
    [
        dir.join(platform.dir_name()).join(format!("{stem}.cht")),
        dir.join(format!("{stem}.cht")),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Where a game's cheats would go.
pub fn home_for(root: &Path, platform: Platform, stem: &str) -> PathBuf {
    root.join(CHEATS_DIR)
        .join(platform.dir_name())
        .join(format!("{stem}.cht"))
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    v.strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(v)
        .to_string()
}

pub fn parse(text: &str) -> Vec<Cheat> {
    let mut keys: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            keys.insert(k.trim().to_ascii_lowercase(), unquote(v));
        }
    }
    let count: usize = keys.get("cheats").and_then(|n| n.parse().ok()).unwrap_or(0);
    (0..count.min(4096))
        .filter_map(|i| {
            let code = keys.get(&format!("cheat{i}_code"))?.trim().to_string();
            if code.is_empty() {
                return None;
            }
            let desc = keys
                .get(&format!("cheat{i}_desc"))
                .filter(|d| !d.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Cheat {}", i + 1));
            let enabled = keys
                .get(&format!("cheat{i}_enable"))
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"));
            Some(Cheat {
                desc,
                code,
                enabled,
            })
        })
        .collect()
}

pub fn read(root: &Path, platform: Platform, stem: &str) -> Vec<Cheat> {
    file_for(root, platform, stem)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn render(cheats: &[Cheat]) -> String {
    let mut out = format!("cheats = {}\n\n", cheats.len());
    for (i, c) in cheats.iter().enumerate() {
        out.push_str(&format!(
            "cheat{i}_desc = \"{}\"\ncheat{i}_code = \"{}\"\ncheat{i}_enable = {}\n\n",
            c.desc.replace('"', "'"),
            c.code.replace('"', ""),
            c.enabled
        ));
    }
    out
}

/// Writes the cheats back, keeping which are on, to the file they came
/// from.
pub fn write(root: &Path, platform: Platform, stem: &str, cheats: &[Cheat]) -> std::io::Result<()> {
    let path = file_for(root, platform, stem).unwrap_or_else(|| home_for(root, platform, stem));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(&path, render(cheats).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHT: &str = "cheats = 3\n\n\
        cheat0_desc = \"Infinite money\"\n\
        cheat0_code = \"82025BC4+FFFF\"\n\
        cheat0_enable = false\n\n\
        cheat1_desc = \"Walk through walls\"\n\
        cheat1_code = \"509176A0+0000\"\n\
        cheat1_enable = true\n\n\
        cheat2_desc = \"Empty\"\n\
        cheat2_code = \"\"\n";

    #[test]
    fn a_retroarch_cheat_file_reads_and_writes_back_the_same() {
        let list = parse(CHT);
        assert_eq!(list.len(), 2, "the empty cheat was kept");
        assert_eq!(list[0].desc, "Infinite money");
        assert_eq!(list[0].code, "82025BC4+FFFF");
        assert!(!list[0].enabled && list[1].enabled);
        assert_eq!(parse(&render(&list)), list);
    }

    #[test]
    fn the_system_folder_is_looked_in_first_and_changes_go_back_there() {
        let d = tempfile::tempdir().unwrap();
        let loose = d.path().join(CHEATS_DIR).join("Emerald.cht");
        std::fs::create_dir_all(loose.parent().unwrap()).unwrap();
        std::fs::write(&loose, CHT).unwrap();
        let mut list = read(d.path(), Platform::Gba, "Emerald");
        assert_eq!(list.len(), 2);
        list[0].enabled = true;
        write(d.path(), Platform::Gba, "Emerald", &list).unwrap();
        assert!(parse(&std::fs::read_to_string(&loose).unwrap())[0].enabled);
        assert!(read(d.path(), Platform::Gb, "Nothing").is_empty());
    }
}
