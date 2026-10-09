//! MD5 helpers.
//!
//! PureRef stores `md5(file[104:])` as a 32-char lowercase hex string, UTF-16BE, at
//! container [40:104] (recon/32 §2); the legacy v1.x header uses `md5(file[108:])`
//! at [44:108]. Both hex-encode lowercase.

use md5::{Digest, Md5};

/// Lowercase hex MD5 of the whole slice.
pub fn md5_hex(bytes: &[u8]) -> String {
    md5_hex_from(bytes, 0)
}

/// Lowercase hex MD5 of `bytes[offset..]` (no copy — used for the file-tail checksum).
pub fn md5_hex_from(bytes: &[u8], offset: usize) -> String {
    let mut d = Md5::new();
    d.update(&bytes[offset..]);
    d.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
