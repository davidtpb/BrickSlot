//! A small ZIP reader for ROM archives: store and deflate, no encryption,
//! no Zip64. The shelf only needs the first Game Boy file inside.

use miniz_oxide::inflate::decompress_to_vec_with_limit;

const EOCD: &[u8; 4] = b"PK\x05\x06";
const CD: &[u8; 4] = b"PK\x01\x02";
const LOCAL: &[u8; 4] = b"PK\x03\x04";
const STORE: u16 = 0;
const DEFLATE: u16 = 8;
/// Larger than any Game Boy ROM we load.
const ROM_MAX: usize = 32 << 20;

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    method: u16,
    compressed: usize,
    uncompressed: usize,
    local_off: usize,
}

pub fn is_zip(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

/// Names of the files in `data`, in archive order.
pub fn names(data: &[u8]) -> Vec<String> {
    catalog(data).into_iter().map(|e| e.name).collect()
}

pub fn catalog(data: &[u8]) -> Vec<Entry> {
    let Some(eocd) = find_eocd(data) else {
        return Vec::new();
    };
    let n = u16::from_le_bytes(data[eocd + 10..eocd + 12].try_into().unwrap()) as usize;
    let cd_off = u32::from_le_bytes(data[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    let mut at = cd_off;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(entry) = cd_entry(data, &mut at) else {
            break;
        };
        if entry.uncompressed > 0 && entry.uncompressed <= ROM_MAX {
            out.push(entry);
        }
    }
    out
}

pub fn extract(data: &[u8], entry: &Entry) -> Option<Vec<u8>> {
    let local = entry.local_off;
    if data.get(local..local + 4) != Some(&LOCAL[..]) {
        return None;
    }
    let name_len = u16::from_le_bytes(data.get(local + 26..local + 28)?.try_into().ok()?) as usize;
    let extra_len = u16::from_le_bytes(data.get(local + 28..local + 30)?.try_into().ok()?) as usize;
    let start = local
        .checked_add(30)?
        .checked_add(name_len)?
        .checked_add(extra_len)?;
    let body = data.get(start..start.checked_add(entry.compressed)?)?;
    match entry.method {
        STORE => Some(body.to_vec()),
        DEFLATE => decompress_to_vec_with_limit(body, entry.uncompressed).ok(),
        _ => None,
    }
    .filter(|b| b.len() == entry.uncompressed)
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let from = data.len().saturating_sub(22 + 65535);
    let mut i = data.len() - 22;
    loop {
        if i < from {
            return None;
        }
        if data[i..].starts_with(EOCD) {
            let comment = u16::from_le_bytes(data[i + 20..i + 22].try_into().ok()?) as usize;
            if i + 22 + comment == data.len() {
                return Some(i);
            }
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

fn cd_entry(data: &[u8], at: &mut usize) -> Option<Entry> {
    let start = *at;
    if data.get(start..start + 4) != Some(&CD[..]) {
        return None;
    }
    let flag = u16::from_le_bytes(data.get(start + 8..start + 10)?.try_into().ok()?);
    if flag & 1 != 0 {
        return None;
    }
    let method = u16::from_le_bytes(data.get(start + 10..start + 12)?.try_into().ok()?);
    let compressed =
        u32::from_le_bytes(data.get(start + 20..start + 24)?.try_into().ok()?) as usize;
    let uncompressed =
        u32::from_le_bytes(data.get(start + 24..start + 28)?.try_into().ok()?) as usize;
    let name_len = u16::from_le_bytes(data.get(start + 28..start + 30)?.try_into().ok()?) as usize;
    let extra_len = u16::from_le_bytes(data.get(start + 30..start + 32)?.try_into().ok()?) as usize;
    let comment_len =
        u16::from_le_bytes(data.get(start + 32..start + 34)?.try_into().ok()?) as usize;
    let local_off = u32::from_le_bytes(data.get(start + 42..start + 46)?.try_into().ok()?) as usize;
    let name = data.get(start + 46..start + 46 + name_len)?;
    let name = String::from_utf8_lossy(name).replace('\\', "/");
    *at = start + 46 + name_len + extra_len + comment_len;
    Some(Entry {
        name,
        method,
        compressed,
        uncompressed,
        local_off,
    })
}

/// A stored (uncompressed) ZIP with one file. For tests and tiny fixtures.
pub fn store(name: &str, body: &[u8]) -> Vec<u8> {
    let n = name.len() as u16;
    let len = body.len() as u32;
    let mut local = Vec::new();
    local.extend_from_slice(LOCAL);
    local.extend_from_slice(&[20, 0, 0, 0]);
    local.extend_from_slice(&STORE.to_le_bytes());
    local.extend_from_slice(&[0u8; 4]);
    local.extend_from_slice(&[0u8; 4]);
    local.extend_from_slice(&len.to_le_bytes());
    local.extend_from_slice(&len.to_le_bytes());
    local.extend_from_slice(&n.to_le_bytes());
    local.extend_from_slice(&0u16.to_le_bytes());
    local.extend_from_slice(name.as_bytes());
    local.extend_from_slice(body);
    let mut cd = Vec::new();
    cd.extend_from_slice(CD);
    cd.extend_from_slice(&[20, 0, 20, 0, 0, 0]);
    cd.extend_from_slice(&STORE.to_le_bytes());
    cd.extend_from_slice(&[0u8; 8]);
    cd.extend_from_slice(&len.to_le_bytes());
    cd.extend_from_slice(&len.to_le_bytes());
    cd.extend_from_slice(&n.to_le_bytes());
    cd.extend_from_slice(&0u16.to_le_bytes());
    cd.extend_from_slice(&0u16.to_le_bytes());
    cd.extend_from_slice(&0u16.to_le_bytes());
    cd.extend_from_slice(&0u16.to_le_bytes());
    cd.extend_from_slice(&0u32.to_le_bytes());
    cd.extend_from_slice(&0u32.to_le_bytes());
    cd.extend_from_slice(name.as_bytes());
    let mut out = local;
    let cd_off = out.len() as u32;
    let cd_len = cd.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(EOCD);
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&cd_len.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_file_comes_back() {
        let zip = store("Games/Emerald.gba", b"rom-bytes");
        let entries = catalog(&zip);
        assert_eq!(entries[0].name, "Games/Emerald.gba");
        assert_eq!(
            extract(&zip, &entries[0]).as_deref(),
            Some(&b"rom-bytes"[..])
        );
    }
}
