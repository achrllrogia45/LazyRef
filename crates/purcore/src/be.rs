//! Big-endian read/write helpers.
//!
//! All readers index a slice directly and panic on out-of-range access.

pub(crate) fn i8(b: &[u8], off: usize) -> i8 {
    b[off] as i8
}

pub(crate) fn u16(b: &[u8], off: usize) -> u16 {
    u16::from_be_bytes(b[off..off + 2].try_into().unwrap())
}

pub(crate) fn i16(b: &[u8], off: usize) -> i16 {
    i16::from_be_bytes(b[off..off + 2].try_into().unwrap())
}

pub(crate) fn u32(b: &[u8], off: usize) -> u32 {
    u32::from_be_bytes(b[off..off + 4].try_into().unwrap())
}

pub(crate) fn i32(b: &[u8], off: usize) -> i32 {
    i32::from_be_bytes(b[off..off + 4].try_into().unwrap())
}

pub(crate) fn u64(b: &[u8], off: usize) -> u64 {
    u64::from_be_bytes(b[off..off + 8].try_into().unwrap())
}

pub(crate) fn f64(b: &[u8], off: usize) -> f64 {
    f64::from_bits(u64(b, off))
}

pub(crate) fn put_u32(b: &mut [u8], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_be_bytes());
}

/// Decode UTF-16BE bytes (Java `Charsets.UTF_16BE` semantics: U+FFFD for malformed units).
pub(crate) fn utf16_be_to_string(b: &[u8]) -> String {
    let units: Vec<u16> = (0..b.len() / 2).map(|i| u16(b, 2 * i)).collect();
    String::from_utf16_lossy(&units)
}
