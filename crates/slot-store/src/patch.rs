//! IPS and BPS patches applied to a ROM before the core sees it.

const IPS_MAGIC: &[u8] = b"PATCH";
const IPS_EOF: &[u8] = b"EOF";
const BPS_MAGIC: &[u8] = b"BPS1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchError {
    Header,
    Truncated,
    Source,
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchError::Header => write!(f, "not a patch"),
            PatchError::Truncated => write!(f, "the patch was cut short"),
            PatchError::Source => write!(f, "the patch is for a different ROM"),
        }
    }
}

impl std::error::Error for PatchError {}

pub fn apply_ips(rom: &mut Vec<u8>, patch: &[u8]) -> Result<(), PatchError> {
    if patch.len() < 8 || !patch.starts_with(IPS_MAGIC) {
        return Err(PatchError::Header);
    }
    let mut i = 5;
    loop {
        let rest = patch.get(i..).ok_or(PatchError::Truncated)?;
        if rest.len() >= 3 && rest.starts_with(IPS_EOF) {
            i += 3;
            if let Some(tail) = patch.get(i..i + 3) {
                let size = be24(tail);
                rom.truncate(size);
            }
            return Ok(());
        }
        if rest.len() < 5 {
            return Err(PatchError::Truncated);
        }
        let offset = be24(&rest[..3]);
        let size = u16::from_be_bytes([rest[3], rest[4]]) as usize;
        i += 5;
        if size == 0 {
            let rle = patch.get(i..i + 3).ok_or(PatchError::Truncated)?;
            let count = u16::from_be_bytes([rle[0], rle[1]]) as usize;
            let fill = rle[2];
            i += 3;
            grow(rom, offset + count);
            rom[offset..offset + count].fill(fill);
        } else {
            let data = patch.get(i..i + size).ok_or(PatchError::Truncated)?;
            i += size;
            grow(rom, offset + size);
            rom[offset..offset + size].copy_from_slice(data);
        }
    }
}

/// The largest ROM a BPS patch may make or start from. GBA carts top out
/// at 32MB, so anything past this is a broken or hostile patch.
pub const BPS_MAX: usize = 64 << 20;

/// International BPS (byuu). The source, target and patch CRC32s in the
/// footer are all checked, so a patch for another ROM, or a damaged one,
/// is refused rather than producing a broken game.
pub fn apply_bps(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError> {
    if patch.len() < 16 || !patch.starts_with(BPS_MAGIC) {
        return Err(PatchError::Header);
    }
    let actions_end = patch.len() - 12;
    let footer = &patch[actions_end..];
    let want_source = le32(&footer[0..4]);
    let want_target = le32(&footer[4..8]);
    let want_patch = le32(&footer[8..12]);
    if crc32(&patch[..patch.len() - 4]) != want_patch {
        return Err(PatchError::Truncated);
    }
    let mut i = 4;
    let source_size = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    let target_size = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    let meta = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    i = i.checked_add(meta).ok_or(PatchError::Truncated)?;
    if i > actions_end || target_size > BPS_MAX {
        return Err(PatchError::Truncated);
    }
    if rom.len() != source_size || crc32(rom) != want_source {
        return Err(PatchError::Source);
    }
    let mut out = vec![0u8; target_size];
    let (mut out_at, mut source_rel, mut target_rel) = (0usize, 0usize, 0usize);
    while i < actions_end {
        let word = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
        let length = (word >> 2).checked_add(1).ok_or(PatchError::Truncated)?;
        let end = out_at.checked_add(length).ok_or(PatchError::Truncated)?;
        if end > target_size {
            return Err(PatchError::Truncated);
        }
        match word & 3 {
            0 => {
                let src = rom.get(out_at..end).ok_or(PatchError::Truncated)?;
                out[out_at..end].copy_from_slice(src);
            }
            1 => {
                let data_end = i.checked_add(length).ok_or(PatchError::Truncated)?;
                if data_end > actions_end {
                    return Err(PatchError::Truncated);
                }
                out[out_at..end].copy_from_slice(&patch[i..data_end]);
                i = data_end;
            }
            2 => {
                source_rel = relative(source_rel, vli(patch, &mut i))?;
                let src_end = source_rel
                    .checked_add(length)
                    .ok_or(PatchError::Truncated)?;
                let src = rom.get(source_rel..src_end).ok_or(PatchError::Truncated)?;
                out[out_at..end].copy_from_slice(src);
                source_rel = src_end;
            }
            _ => {
                target_rel = relative(target_rel, vli(patch, &mut i))?;
                // Byte by byte: the copy may overlap what it is writing.
                for at in out_at..end {
                    if target_rel >= at {
                        return Err(PatchError::Truncated);
                    }
                    out[at] = out[target_rel];
                    target_rel += 1;
                }
            }
        }
        out_at = end;
    }
    if out_at != target_size || crc32(&out) != want_target {
        return Err(PatchError::Truncated);
    }
    Ok(out)
}

/// Moves a BPS copy cursor by a signed offset, refusing to go below zero.
fn relative(at: usize, offset: Option<usize>) -> Result<usize, PatchError> {
    let offset = offset.ok_or(PatchError::Truncated)?;
    let step = offset >> 1;
    match offset & 1 {
        0 => at.checked_add(step),
        _ => at.checked_sub(step),
    }
    .ok_or(PatchError::Truncated)
}

fn le32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// The CRC-32 (IEEE) that BPS footers carry.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ b as u32) & 0xff) as usize];
    }
    !crc
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

fn grow(rom: &mut Vec<u8>, need: usize) {
    if rom.len() < need {
        rom.resize(need, 0);
    }
}

fn be24(b: &[u8]) -> usize {
    ((b[0] as usize) << 16) | ((b[1] as usize) << 8) | b[2] as usize
}

fn vli(data: &[u8], at: &mut usize) -> Option<usize> {
    let mut shift = 0;
    let mut value = 0usize;
    loop {
        let b = *data.get(*at)?;
        *at += 1;
        if shift > 56 {
            return None;
        }
        value = value.checked_add(((b & 0x7f) as usize) << shift)?;
        if b & 0x80 != 0 {
            return Some(value);
        }
        shift += 7;
        value = value.checked_add(1 << shift)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ips_overwrites_and_grows() {
        let mut rom = b"HELLO".to_vec();
        let mut patch = IPS_MAGIC.to_vec();
        patch.extend_from_slice(&[0, 0, 1, 0, 3]);
        patch.extend_from_slice(b"XYZ");
        patch.extend_from_slice(IPS_EOF);
        apply_ips(&mut rom, &patch).unwrap();
        assert_eq!(rom, b"HXYZO");
    }

    #[test]
    fn ips_rle_fills() {
        let mut rom = vec![0u8; 4];
        let mut patch = IPS_MAGIC.to_vec();
        patch.extend_from_slice(&[0, 0, 1, 0, 0, 0, 2, b'A']);
        patch.extend_from_slice(IPS_EOF);
        apply_ips(&mut rom, &patch).unwrap();
        assert_eq!(rom, b"\0AA\0");
    }

    fn put_vli(out: &mut Vec<u8>, mut n: usize) {
        loop {
            let x = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                out.push(0x80 | x);
                return;
            }
            out.push(x);
            n -= 1;
        }
    }

    /// A BPS patch from `source` to `target` with the given actions, footer
    /// CRCs filled in.
    fn bps(source: &[u8], target_size: usize, target: &[u8], actions: &[u8]) -> Vec<u8> {
        let mut p = BPS_MAGIC.to_vec();
        put_vli(&mut p, source.len());
        put_vli(&mut p, target_size);
        put_vli(&mut p, 0);
        p.extend_from_slice(actions);
        p.extend_from_slice(&crc32(source).to_le_bytes());
        p.extend_from_slice(&crc32(target).to_le_bytes());
        let c = crc32(&p);
        p.extend_from_slice(&c.to_le_bytes());
        p
    }

    /// SourceRead 1 byte, TargetRead "XYZ", SourceRead 1 byte.
    fn hello_actions() -> Vec<u8> {
        let mut a = Vec::new();
        put_vli(&mut a, 0); // SourceRead, length 1
        put_vli(&mut a, (2 << 2) | 1); // TargetRead, length 3
        a.extend_from_slice(b"XYZ");
        put_vli(&mut a, 0); // SourceRead, length 1
        a
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn bps_applies_and_checks_its_crcs() {
        let patch = bps(b"HELLO", 5, b"HXYZO", &hello_actions());
        assert_eq!(apply_bps(b"HELLO", &patch).unwrap(), b"HXYZO");
    }

    #[test]
    fn bps_for_a_different_rom_of_the_same_size_is_refused() {
        let patch = bps(b"HELLO", 5, b"HXYZO", &hello_actions());
        assert_eq!(apply_bps(b"JELLO", &patch), Err(PatchError::Source));
    }

    #[test]
    fn a_damaged_bps_is_refused() {
        let mut patch = bps(b"HELLO", 5, b"HXYZO", &hello_actions());
        let at = patch.len() - 14;
        patch[at] ^= 1;
        assert!(apply_bps(b"HELLO", &patch).is_err());
    }

    #[test]
    fn a_bps_that_writes_past_its_target_is_an_error_not_a_panic() {
        // SourceCopy of 4 bytes into a 2 byte target.
        let mut a = Vec::new();
        put_vli(&mut a, (3 << 2) | 2);
        put_vli(&mut a, 0);
        let patch = bps(b"HELLO", 2, b"HE", &a);
        assert!(apply_bps(b"HELLO", &patch).is_err());
        // TargetCopy reading ahead of what has been written.
        let mut a = Vec::new();
        put_vli(&mut a, (1 << 2) | 3);
        put_vli(&mut a, 0);
        let patch = bps(b"HELLO", 2, b"HE", &a);
        assert!(apply_bps(b"HELLO", &patch).is_err());
    }

    #[test]
    fn a_bps_asking_for_a_huge_target_is_refused_before_allocating() {
        let patch = bps(b"HELLO", usize::MAX >> 8, b"", &hello_actions());
        assert!(apply_bps(b"HELLO", &patch).is_err());
    }
}
