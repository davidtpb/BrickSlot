use std::path::{Path, PathBuf};

use slot_store::Platform;

const THUMBNAILS: &str = "https://thumbnails.libretro.com";

/// The same collection as git repositories, for when the server is down.
const MIRROR: &str = "https://raw.githubusercontent.com/libretro-thumbnails";

/// A game with no label yet, and where its label goes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Wanted {
    pub platform: Platform,
    pub stem: String,
    pub dest: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Tally {
    pub found: usize,
    pub missing: usize,
}

pub fn system_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Gba => "Nintendo - Game Boy Advance",
        Platform::Gb => "Nintendo - Game Boy",
        Platform::Gbc => "Nintendo - Game Boy Color",
    }
}

/// The names to try for a ROM, most exact first: as it is, with accents
/// dropped, and without the trailing revision or version tag that No-Intro
/// names often carry and the thumbnails often do not.
pub fn candidates(stem: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: String| {
        let name = thumbnail_safe(&name);
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    };
    add(stem.to_string());
    let plain = strip_accents(stem);
    add(plain.clone());
    for name in [stem.to_string(), plain] {
        let trimmed = without_trailing_tag(&name);
        if trimmed != name {
            add(trimmed);
        }
    }
    out
}

/// libretro-thumbnails stores `&*/:`<>?\|"` as `_`.
pub fn thumbnail_safe(name: &str) -> String {
    name.chars()
        .map(|c| if "&*/:`<>?\\|\"".contains(c) { '_' } else { c })
        .collect()
}

fn strip_accents(name: &str) -> String {
    use unicode_normalization::char::is_combining_mark;
    use unicode_normalization::UnicodeNormalization;
    name.nfd().filter(|c| !is_combining_mark(*c)).collect()
}

fn without_trailing_tag(name: &str) -> String {
    let t = name.trim_end();
    match t
        .strip_suffix(')')
        .and_then(|s| s.rfind('(').map(|i| (s, i)))
    {
        Some((s, i)) => {
            let tag = &s[i + 1..];
            if tag.starts_with("Rev") || tag.starts_with('v') || tag.starts_with("Beta") {
                s[..i].trim_end().to_string()
            } else {
                t.to_string()
            }
        }
        None => t.to_string(),
    }
}

fn escape(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Where a label might be: the thumbnail server, then its GitHub mirror.
pub fn urls(platform: Platform, name: &str) -> [String; 2] {
    let repo = system_name(platform).replace(' ', "_");
    [
        url(platform, name),
        format!("{MIRROR}/{repo}/master/Named_Boxarts/{}.png", escape(name)),
    ]
}

pub fn url(platform: Platform, name: &str) -> String {
    format!(
        "{THUMBNAILS}/{}/Named_Boxarts/{}.png",
        escape(system_name(platform)),
        escape(name)
    )
}

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Fetches each wanted label, trying its candidate names in turn. Labels
/// already on the card are never touched.
pub fn fetch_all(wanted: &[Wanted], get: impl Fn(&str) -> Option<Vec<u8>>) -> Tally {
    let mut tally = Tally::default();
    for w in wanted {
        if w.dest.exists() {
            continue;
        }
        let png = candidates(&w.stem).iter().find_map(|name| {
            urls(w.platform, name)
                .iter()
                .find_map(|u| get(u).filter(|b| b.starts_with(PNG_MAGIC)))
        });
        let saved = png.is_some_and(|png| save(&w.dest, &png));
        match saved {
            true => tally.found += 1,
            false => tally.missing += 1,
        }
    }
    tally
}

fn save(dest: &Path, png: &[u8]) -> bool {
    if let Some(dir) = dest.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match slot_store::atomic_write(dest, png) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("slot: labels: {}: {e}", dest.display());
            false
        }
    }
}

/// An HTTPS GET for the device, None on anything but a 200.
pub fn http_get(url: &str) -> Option<Vec<u8>> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .ok()?;
    if response.status() != 200 {
        return None;
    }
    let mut body = Vec::new();
    let reader = response.into_reader();
    std::io::Read::read_to_end(&mut std::io::Read::take(reader, 8 << 20), &mut body).ok()?;
    Some(body)
}
