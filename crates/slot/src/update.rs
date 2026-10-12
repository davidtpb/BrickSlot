//! Download a GitHub release zip onto the card and copy the binary, cores
//! and TrimUI updater out of it.

use std::io::Read;
use std::path::Path;

const REPO: &str = "davidtpb/BrickSlot";
const AGENT: &str = concat!("brickslot/", env!("CARGO_PKG_VERSION"));

const KEEP: &[&str] = &[
    "System/slot",
    "System/mgba_libretro.so",
    "System/gpsp_libretro.so",
    "System/syncthing",
    ".tmp_update/updater",
];

pub fn run(root: &Path) -> (String, String) {
    match apply(root) {
        Ok(tag) => (
            "Update ready".into(),
            format!("{tag} is on the card. Restart to use it"),
        ),
        Err(e) => ("Update failed".into(), e),
    }
}

fn apply(root: &Path) -> Result<String, String> {
    let json = get(&format!(
        "https://api.github.com/repos/{REPO}/releases/latest"
    ))?;
    let tag = json_string(&json, "tag_name").ok_or("the release had no tag")?;
    let url = asset_url(&json).ok_or("no card zip on the latest release")?;
    let zip = get_bytes(&url)?;
    unpack(root, &zip)?;
    Ok(tag)
}

fn asset_url(json: &str) -> Option<String> {
    let mut urls: Vec<String> = Vec::new();
    let mut rest = json;
    while let Some(at) = rest.find("\"browser_download_url\"") {
        rest = &rest[at + 23..];
        let rest = rest.trim_start_matches([' ', ':', '"']);
        let end = rest.find('"')?;
        urls.push(rest[..end].to_string());
    }
    urls.into_iter().find(|u| u.contains("trimui") && u.ends_with(".zip"))
        .or_else(|| {
            let mut rest = json;
            let mut found = None;
            while let Some(at) = rest.find("\"browser_download_url\"") {
                rest = &rest[at + 23..];
                let s = rest.trim_start_matches([' ', ':', '"']);
                let Some(end) = s.find('"') else { break };
                let url = &s[..end];
                if url.ends_with(".zip") {
                    found = Some(url.to_string());
                    break;
                }
                rest = &s[end..];
            }
            found
        })
}

fn json_string(json: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let at = json.find(&pat)?;
    let rest = json[at + pat.len()..].trim_start_matches([' ', ':']);
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn get(url: &str) -> Result<String, String> {
    ureq::get(url)
        .set("User-Agent", AGENT)
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())
}

fn get_bytes(url: &str) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    ureq::get(url)
        .set("User-Agent", AGENT)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| e.to_string())?;
    Ok(body)
}

fn unpack(root: &Path, zip: &[u8]) -> Result<(), String> {
    let entries = slot_store::zip::catalog(zip);
    let mut wrote = 0;
    for entry in &entries {
        let name = entry.name.trim_start_matches('/');
        let rel = KEEP.iter().find(|k| name.ends_with(*k));
        let Some(rel) = rel else {
            continue;
        };
        let Some(bytes) = slot_store::zip::extract(zip, entry) else {
            continue;
        };
        let dest = root.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        slot_store::atomic_write(&dest, &bytes).map_err(|e| e.to_string())?;
        wrote += 1;
    }
    if wrote == 0 {
        return Err("the zip had no brickslot files".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_and_trimui_zip_are_picked_from_a_release_body() {
        let json = r#"{"tag_name":"v1.2.3","assets":[
            {"browser_download_url":"https://example/slot-v1.2.3-h700.zip"},
            {"browser_download_url":"https://example/brickslot-v1.2.3-trimui.zip"}
        ]}"#;
        assert_eq!(json_string(json, "tag_name").as_deref(), Some("v1.2.3"));
        assert!(asset_url(json).unwrap().contains("trimui"));
    }
}
