//! RetroArch's RZIP, the format its "SaveRAM Compression" writes `.srm`
//! files in: a 20-byte header, then chunks each compressed on its own.
//!
//! The header is `#RZIPv`, a version byte (1 for zlib chunks, 2 for
//! Zstandard), `#`, the chunk size as a little-endian u32 and the whole
//! uncompressed size as a little-endian u64. Each chunk is a little-endian
//! u32 length and that many compressed bytes, at most `chunk size` bytes
//! once inflated.

use std::io::Read;

const MAGIC: &[u8; 6] = b"#RZIPv";
const HEADER: usize = 20;
const ZLIB: u8 = 1;
const ZSTD: u8 = 2;
/// RetroArch's own chunk size.
const CHUNK: usize = 128 * 1024;
/// RetroArch refuses larger chunks, and so does this.
const CHUNK_MAX: usize = 64 * 1024 * 1024;

/// Whether `data` is an RZIP file.
pub fn is_rzip(data: &[u8]) -> bool {
    data.len() >= HEADER && data.starts_with(MAGIC) && data[7] == b'#'
}

#[derive(Debug, PartialEq, Eq)]
pub enum RzipError {
    Header,
    Version(u8),
    Chunk(usize),
    Size { want: u64, got: usize },
}

impl std::fmt::Display for RzipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RzipError::Header => write!(f, "broken RZIP header"),
            RzipError::Version(v) => write!(f, "unknown RZIP version {v}"),
            RzipError::Chunk(at) => write!(f, "broken RZIP chunk at byte {at}"),
            RzipError::Size { want, got } => {
                write!(f, "RZIP said {want} bytes and held {got}")
            }
        }
    }
}

impl std::error::Error for RzipError {}

/// Inflates an RZIP file.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, RzipError> {
    if !is_rzip(data) {
        return Err(RzipError::Header);
    }
    let version = data[6];
    if version != ZLIB && version != ZSTD {
        return Err(RzipError::Version(version));
    }
    let chunk = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let total = u64::from_le_bytes(data[12..20].try_into().unwrap());
    if chunk == 0 || chunk > CHUNK_MAX || total == 0 {
        return Err(RzipError::Header);
    }
    let mut out = Vec::with_capacity(total.min(64 << 20) as usize);
    let mut at = HEADER;
    while at < data.len() && (out.len() as u64) < total {
        let len = data
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize)
            .ok_or(RzipError::Chunk(at))?;
        let body = data
            .get(at + 4..at + 4 + len)
            .filter(|_| len > 0 && len <= 2 * chunk + 64)
            .ok_or(RzipError::Chunk(at))?;
        let piece = match version {
            ZLIB => inflate(body),
            _ => unzstd(body),
        }
        .filter(|p| !p.is_empty() && p.len() <= chunk)
        .ok_or(RzipError::Chunk(at))?;
        out.extend_from_slice(&piece);
        at += 4 + len;
    }
    if out.len() as u64 != total {
        return Err(RzipError::Size {
            want: total,
            got: out.len(),
        });
    }
    Ok(out)
}

/// A chunk is a zlib stream; raw deflate is taken too, to be safe.
fn inflate(body: &[u8]) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_zlib(body)
        .or_else(|_| miniz_oxide::inflate::decompress_to_vec(body))
        .ok()
}

fn unzstd(body: &[u8]) -> Option<Vec<u8>> {
    let mut reader = body;
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut reader).ok()?;
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).ok()?;
    Some(out)
}

/// Compresses `data` as RZIP with zlib chunks (version 1), which every
/// RetroArch that knows RZIP reads.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER + data.len() / 2);
    out.extend_from_slice(MAGIC);
    out.push(ZLIB);
    out.push(b'#');
    out.extend_from_slice(&(CHUNK as u32).to_le_bytes());
    out.extend_from_slice(&(data.len() as u64).to_le_bytes());
    for piece in data.chunks(CHUNK) {
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(piece, 6);
        out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        out.extend_from_slice(&packed);
    }
    out
}

/// The save as the core wants it: inflated if it is RZIP, as it is if not.
/// A broken RZIP file is an error rather than garbage for the game.
pub fn plain(data: Vec<u8>) -> Result<Vec<u8>, RzipError> {
    match is_rzip(&data) {
        true => decompress(&data),
        false => Ok(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn save(n: usize) -> Vec<u8> {
        (0..n)
            .map(|i| (i * 7 % 251) as u8 ^ (i / 1000) as u8)
            .collect()
    }

    #[test]
    fn a_save_comes_back_whole_through_rzip() {
        for n in [1, 512, 32 * 1024, CHUNK, CHUNK + 1, 3 * CHUNK + 17] {
            let data = save(n);
            let packed = compress(&data);
            assert!(is_rzip(&packed));
            assert_eq!(&packed[..8], b"#RZIPv\x01#");
            assert_eq!(decompress(&packed).unwrap(), data, "{n} bytes");
        }
    }

    #[test]
    fn zstandard_chunks_are_read() {
        let data = save(70_000);
        let mut file = Vec::new();
        file.extend_from_slice(b"#RZIPv\x02#");
        file.extend_from_slice(&(CHUNK as u32).to_le_bytes());
        file.extend_from_slice(&(data.len() as u64).to_le_bytes());
        let packed = ruzstd::encoding::compress_to_vec(
            &data[..],
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        file.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        file.extend_from_slice(&packed);
        assert_eq!(decompress(&file).unwrap(), data);
    }

    #[test]
    fn a_plain_save_is_left_alone_and_a_broken_one_refused() {
        let data = save(1024);
        assert_eq!(plain(data.clone()).unwrap(), data);
        let mut packed = compress(&save(5000));
        packed.truncate(packed.len() - 10);
        assert!(plain(packed).is_err());
        let mut packed = compress(&save(10));
        packed[6] = 9;
        assert_eq!(decompress(&packed), Err(RzipError::Version(9)));
    }
}
