//! A small ZIP reader for ROM archives: store and deflate, no encryption,
//! no Zip64. The shelf only needs the first Game Boy file inside.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use miniz_oxide::inflate::decompress_to_vec_with_limit;
use miniz_oxide::inflate::stream::{inflate, InflateState};
use miniz_oxide::{DataFormat, MZFlush};

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

/// Like [`catalog`], but reads only the end of the file: the directory
/// record and the central directory, never the ROMs themselves.
pub fn catalog_path(path: &Path) -> Vec<Entry> {
    catalog_file(path).unwrap_or_default()
}

fn catalog_file(path: &Path) -> Option<Vec<Entry>> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let tail_len = len.min(22 + 65535);
    f.seek(SeekFrom::Start(len - tail_len)).ok()?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail).ok()?;
    let eocd = find_eocd(&tail)?;
    let n = u16::from_le_bytes(tail[eocd + 10..eocd + 12].try_into().ok()?) as usize;
    let cd_len = u32::from_le_bytes(tail[eocd + 12..eocd + 16].try_into().ok()?) as u64;
    let cd_off = u32::from_le_bytes(tail[eocd + 16..eocd + 20].try_into().ok()?) as u64;
    if cd_len > 4 << 20 || cd_off.checked_add(cd_len)? > len {
        return None;
    }
    f.seek(SeekFrom::Start(cd_off)).ok()?;
    let mut cd = vec![0u8; cd_len as usize];
    f.read_exact(&mut cd).ok()?;
    let mut at = 0;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(entry) = cd_entry(&cd, &mut at) else {
            break;
        };
        if entry.uncompressed > 0 && entry.uncompressed <= ROM_MAX {
            out.push(entry);
        }
    }
    Some(out)
}

/// Seeks `f` to the start of `entry`'s stored bytes.
fn seek_body(f: &mut File, entry: &Entry) -> Option<()> {
    f.seek(SeekFrom::Start(entry.local_off as u64)).ok()?;
    let mut local = [0u8; 30];
    f.read_exact(&mut local).ok()?;
    if &local[..4] != LOCAL {
        return None;
    }
    let name_len = u16::from_le_bytes([local[26], local[27]]) as i64;
    let extra_len = u16::from_le_bytes([local[28], local[29]]) as i64;
    f.seek(SeekFrom::Current(name_len + extra_len)).ok()?;
    Some(())
}

/// Like [`extract`], reading only that entry from the file.
pub fn extract_path(path: &Path, entry: &Entry) -> Option<Vec<u8>> {
    if entry.compressed > ROM_MAX * 2 {
        return None;
    }
    let mut f = File::open(path).ok()?;
    seek_body(&mut f, entry)?;
    let mut body = vec![0u8; entry.compressed];
    f.read_exact(&mut body).ok()?;
    match entry.method {
        STORE => Some(body),
        DEFLATE => decompress_to_vec_with_limit(&body, entry.uncompressed).ok(),
        _ => None,
    }
    .filter(|b| b.len() == entry.uncompressed)
}

/// The first `n` bytes of `entry`, inflating only as much as that takes.
/// Enough for a cartridge header without unpacking the whole game.
pub fn head_path(path: &Path, entry: &Entry, n: usize) -> Option<Vec<u8>> {
    let n = n.min(entry.uncompressed);
    let mut f = File::open(path).ok()?;
    seek_body(&mut f, entry)?;
    // Deflate rarely needs more than a few KB of input for a header; a
    // generous slice still keeps this to one small read.
    let want = match entry.method {
        STORE => n,
        _ => entry.compressed.min(n.saturating_mul(4).max(64 << 10)),
    };
    let mut body = vec![0u8; want];
    f.read_exact(&mut body).ok()?;
    match entry.method {
        STORE => Some(body),
        DEFLATE => {
            let mut state = InflateState::new_boxed(DataFormat::Raw);
            let mut out = vec![0u8; n];
            let (mut read, mut wrote) = (0, 0);
            while wrote < n && read < body.len() {
                let r = inflate(&mut state, &body[read..], &mut out[wrote..], MZFlush::None);
                read += r.bytes_consumed;
                wrote += r.bytes_written;
                if r.status.is_err() || (r.bytes_consumed == 0 && r.bytes_written == 0) {
                    break;
                }
            }
            (wrote == n).then_some(out)
        }
        _ => None,
    }
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

    #[test]
    fn a_file_is_listed_and_read_without_loading_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.zip");
        std::fs::write(&path, store("Emerald.gba", b"0123456789")).unwrap();
        let entries = catalog_path(&path);
        assert_eq!(entries[0].name, "Emerald.gba");
        assert_eq!(
            extract_path(&path, &entries[0]).as_deref(),
            Some(&b"0123456789"[..])
        );
        assert_eq!(
            head_path(&path, &entries[0], 4).as_deref(),
            Some(&b"0123"[..])
        );
    }

    #[test]
    fn the_head_of_a_deflated_file_is_inflated_alone() {
        let body: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let packed = miniz_oxide::deflate::compress_to_vec(&body, 6);
        let mut zip = store("Big.gba", &packed);
        // Mark the entry deflated with its real size, in both headers.
        let size = (body.len() as u32).to_le_bytes();
        zip[8] = DEFLATE as u8;
        zip[22..26].copy_from_slice(&size);
        let cd = zip.windows(4).position(|w| w == CD).unwrap();
        zip[cd + 10] = DEFLATE as u8;
        zip[cd + 24..cd + 28].copy_from_slice(&size);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.zip");
        std::fs::write(&path, &zip).unwrap();
        let entry = &catalog_path(&path)[0];
        assert_eq!(head_path(&path, entry, 0x150).unwrap(), &body[..0x150]);
        assert_eq!(extract_path(&path, entry).unwrap(), body);
    }
}
