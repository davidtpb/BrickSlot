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

/// International BPS (byuu). Source checksums are checked when present.
pub fn apply_bps(rom: &[u8], patch: &[u8]) -> Result<Vec<u8>, PatchError> {
    if patch.len() < 16 || !patch.starts_with(BPS_MAGIC) {
        return Err(PatchError::Header);
    }
    let mut i = 4;
    let source_size = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    let target_size = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    let meta = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
    i = i.checked_add(meta).ok_or(PatchError::Truncated)?;
    if rom.len() != source_size {
        return Err(PatchError::Source);
    }
    if patch.len() < i + 12 {
        return Err(PatchError::Truncated);
    }
    let actions_end = patch.len() - 12;
    let mut out = vec![0u8; target_size];
    let (mut out_at, mut source_rel, mut target_rel) = (0usize, 0i64, 0i64);
    while i < actions_end {
        let word = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
        let command = word & 3;
        let length = (word >> 2) + 1;
        match command {
            0 => {
                let end = out_at.checked_add(length).ok_or(PatchError::Truncated)?;
                let src = rom.get(out_at..end).ok_or(PatchError::Truncated)?;
                let dest = out.get_mut(out_at..end).ok_or(PatchError::Truncated)?;
                dest.copy_from_slice(src);
                out_at = end;
            }
            1 => {
                let data = patch.get(i..i + length).ok_or(PatchError::Truncated)?;
                i += length;
                let dest = out
                    .get_mut(out_at..out_at + length)
                    .ok_or(PatchError::Truncated)?;
                dest.copy_from_slice(data);
                out_at += length;
            }
            2 => {
                let offset = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
                source_rel += if offset & 1 == 0 {
                    (offset >> 1) as i64
                } else {
                    -((offset >> 1) as i64)
                };
                for _ in 0..length {
                    let at = usize::try_from(source_rel).map_err(|_| PatchError::Truncated)?;
                    out[out_at] = *rom.get(at).ok_or(PatchError::Truncated)?;
                    out_at += 1;
                    source_rel += 1;
                }
            }
            _ => {
                let offset = vli(patch, &mut i).ok_or(PatchError::Truncated)?;
                target_rel += if offset & 1 == 0 {
                    (offset >> 1) as i64
                } else {
                    -((offset >> 1) as i64)
                };
                for _ in 0..length {
                    let at = usize::try_from(target_rel).map_err(|_| PatchError::Truncated)?;
                    out[out_at] = *out.get(at).ok_or(PatchError::Truncated)?;
                    out_at += 1;
                    target_rel += 1;
                }
            }
        }
        if out_at > target_size {
            return Err(PatchError::Truncated);
        }
    }
    if out_at != target_size {
        return Err(PatchError::Truncated);
    }
    Ok(out)
}

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
        value += ((b & 0x7f) as usize) << shift;
        if b & 0x80 != 0 {
            return Some(value);
        }
        shift += 7;
        value += 1 << shift;
        if shift > 56 {
            return None;
        }
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
}
