//! `.ref` board persistence: MessagePack wire format, bitmaps deduped by SHA-256.
//!
//! Hand-rolled MessagePack subset (no new deps): ints, int64, float32,
//! UTF-8 strings, byte arrays, arrays, bools.

pub const SERIAL_VERSION: i32 = 1;

pub struct RefEntry {
    /// Real pixel bounds (left, top, right, bottom).
    pub bounds: (i32, i32, i32, i32),
    pub pos: (f32, f32),
    pub rotation: f32,
    pub scale: (f32, f32),
    /// Crop rect (left, top, right, bottom) in content px.
    pub crop: (f32, f32, f32, f32),
    pub flip_h: bool,
    pub flip_v: bool,
}

pub struct RefImage {
    pub sha: String,
    pub bitmap: Vec<u8>,
    pub instances: Vec<RefEntry>,
}

/// Text-sticker sidecar, keyed by position in [`RefBoardData::images`].
pub struct RefText {
    pub image_index: usize,
    pub text: String,
    pub size_px: f32,
    pub color: (u8, u8, u8),
    pub background: Option<(u8, u8, u8)>,
    pub bold: bool,
}

pub struct RefBoardData {
    pub version: i32,
    pub date: i64,
    /// Board canvas matrix values (9 floats).
    pub canvas: [f32; 9],
    pub images: Vec<RefImage>,
    pub texts: Vec<RefText>,
}

pub fn encode(board: &RefBoardData) -> Vec<u8> {
    unimplemented!()
}

pub fn decode(bytes: &[u8]) -> Result<RefBoardData, String> {
    unimplemented!()
}
