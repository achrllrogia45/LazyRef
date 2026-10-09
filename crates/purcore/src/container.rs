//! Container layer for PureRef 2.x `.pur` files.
//!
//! Layout:
//!
//! ```text
//! [ container chunk C: 108 B header + thumbnail JPEG ][ DB[C_len : db_size] ][ DB[0 : C_len] head copy ]
//! ```
//!
//! `C_len = 108 + thumbnailLength` (u32 @ [104:108]), `db_size` = qint64 @ [14:22] = file length
//! after removing the container, and `checksum = md5(file[104:])` hex stored UTF-16BE at [40:104]
//! (u32@[36:40] = 64). The DB is reassembled as `file[db_size:] + file[C_len:db_size]`.

use crate::be;
use crate::checksum::md5_hex_from;

pub const FILE_VERSION: &str = "2.1";
pub const APP_VERSION: &str = "2.1.3";

/// 108-byte container header (thumbnail starts at this offset).
pub const HEADER_LENGTH: usize = 108;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    V2,
    Legacy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerHeader {
    pub file_version: String,  // [0:10]  QString "2.1"
    pub color_a: i16,          // [10:12] i16 — legacy color pair, 0 in all samples
    pub color_b: i16,          // [12:14] i16
    pub db_size: i64,          // [14:22] qint64 BE — DB byte size; also the tail head-copy offset
    pub app_version: String,   // [22:36] QString "2.1.3"
    pub checksum: String,      // [40:104] md5 hex of file[104:]
    pub thumbnail_length: u32, // [104:108]
    pub chunk_length: usize,   // 108 + thumbnailLength
}

/// Sniffs the file format from the version string at [0:10].
///
/// The app compares dotted-numeric versions against "2.0"; the two known
/// families are 1.x (legacy) and 2.x (containers), so a major-component check is sufficient.
pub fn detect_format(bytes: &[u8]) -> Format {
    let major = read_qstring(bytes, 0)
        .0
        .split('.')
        .next()
        .unwrap_or("")
        .parse::<i32>()
        .unwrap_or(0);
    if major >= 2 {
        Format::V2
    } else {
        Format::Legacy
    }
}

pub fn parse_header(bytes: &[u8]) -> ContainerHeader {
    assert!(
        bytes.len() >= HEADER_LENGTH,
        "file too small for a 2.x container: {} B",
        bytes.len()
    );
    let mut off = 0;

    let (file_version, n1) = read_qstring(bytes, off);
    off += n1;
    let color_a = be::i16(bytes, off);
    let color_b = be::i16(bytes, off + 2);
    off += 4;
    let db_size = be::u64(bytes, off) as i64;
    off += 8;
    let (app_version, n2) = read_qstring(bytes, off);
    off += n2;
    let checksum_field_length = be::u32(bytes, off);
    off += 4;
    assert!(
        checksum_field_length == 64,
        "unexpected checksum field length {checksum_field_length} (expected 64)"
    );
    let checksum = be::utf16_be_to_string(&bytes[off..off + 64]);
    off += 64;
    let thumbnail_length = be::u32(bytes, off);
    assert!(
        HEADER_LENGTH + thumbnail_length as usize <= bytes.len(),
        "bad thumbnail length {thumbnail_length} (file {} B)",
        bytes.len()
    );
    ContainerHeader {
        file_version,
        color_a,
        color_b,
        db_size,
        app_version,
        checksum,
        thumbnail_length,
        chunk_length: HEADER_LENGTH + thumbnail_length as usize,
    }
}

/// True when the stored checksum matches `md5(file[104:])` (what the app validates on load).
pub fn verify_checksum(bytes: &[u8]) -> bool {
    assert!(
        bytes.len() >= HEADER_LENGTH,
        "file too small: {} B",
        bytes.len()
    );
    md5_hex_from(bytes, 104) == parse_header(bytes).checksum
}

pub fn extract_thumbnail(bytes: &[u8]) -> Vec<u8> {
    let h = parse_header(bytes);
    bytes[HEADER_LENGTH..HEADER_LENGTH + h.thumbnail_length as usize].to_vec()
}

/// Reassembles the embedded SQLite database: `file[db_size:] ++ file[C_len:db_size]`.
pub fn extract_database(bytes: &[u8]) -> Vec<u8> {
    let h = parse_header(bytes);
    extract_database_with(bytes, &h)
}

pub fn extract_database_with(bytes: &[u8], h: &ContainerHeader) -> Vec<u8> {
    let db_size = h.db_size;
    let c_len = h.chunk_length;
    assert!(
        db_size >= 0
            && db_size as u64 >= c_len as u64
            && db_size as usize <= bytes.len()
            && db_size as usize + c_len == bytes.len(),
        "inconsistent container: dbSize={db_size} cLen={c_len} file={} B",
        bytes.len()
    );
    let db_size = db_size as usize;
    let mut out = Vec::with_capacity(db_size);
    out.extend_from_slice(&bytes[db_size..]);
    out.extend_from_slice(&bytes[c_len..db_size]);
    out
}

/// Builds a complete v2 file from a reassembled SQLite database and a thumbnail.
///
/// Inverse of [`extract_database`] + [`extract_thumbnail`]; reproduces app-written files
/// byte-for-byte (verified on 4/4 corpus files). `db_bytes.len()` becomes the db-size field,
/// so `db_bytes` must be at least `108 + thumbnail.len()` (always true in practice — the app's
/// smallest observed DB is 9 pages / 36864 B).
///
/// ponytail: the optional SceneStateData fields the app appends after the JPEG (only when the
/// saved scene-state is valid) are not emitted — absent in every corpus file; add when a real
/// file needs them.
pub fn build(db_bytes: &[u8], thumbnail: &[u8]) -> Vec<u8> {
    let db_size = db_bytes.len();
    let c_len = HEADER_LENGTH + thumbnail.len();
    assert!(
        db_size >= c_len,
        "DB ({db_size} B) smaller than container chunk ({c_len} B) — cannot splice head copy"
    );
    let mut out = vec![0u8; db_size + c_len];

    write_qstring(&mut out, 0, FILE_VERSION);
    // colorA / colorB stay 0 — always 0 in samples
    out[14..22].copy_from_slice(&(db_size as u64).to_be_bytes());
    write_qstring(&mut out, 22, APP_VERSION);
    be::put_u32(&mut out, 36, 64); // checksum field byte length (32 chars UTF-16BE)
                                   // [40:104] checksum placeholder — patched below
    be::put_u32(&mut out, 104, thumbnail.len() as u32);
    out[HEADER_LENGTH..HEADER_LENGTH + thumbnail.len()].copy_from_slice(thumbnail);
    out[c_len..db_size].copy_from_slice(&db_bytes[c_len..db_size]); // DB after the chunk
    out[db_size..db_size + c_len].copy_from_slice(&db_bytes[..c_len]); // head copy at the tail

    let checksum = md5_hex_from(&out, 104);
    assert!(checksum.len() == 32, "unexpected checksum encoding size");
    // UTF-16BE of an ASCII hex string: interleaved 0x00 high bytes (buffer already zeroed)
    for (i, b) in checksum.bytes().enumerate() {
        out[40 + 2 * i + 1] = b;
    }
    out
}

// --------------------------------------------------------------- QString io

/// Reads a `QDataStream << QString` field: `[u32 BE byteLength][byteLength bytes UTF-16BE]`.
fn read_qstring(b: &[u8], off: usize) -> (String, usize) {
    assert!(off + 4 <= b.len(), "truncated QString length at {off}");
    let len = be::u32(b, off) as usize;
    assert!(
        off + 4 + len <= b.len(),
        "bad QString length {len} at {off}"
    );
    (be::utf16_be_to_string(&b[off + 4..off + 4 + len]), 4 + len)
}

fn write_qstring(b: &mut [u8], off: usize, s: &str) {
    let sb: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect();
    be::put_u32(b, off, sb.len() as u32);
    b[off + 4..off + 4 + sb.len()].copy_from_slice(&sb);
}
