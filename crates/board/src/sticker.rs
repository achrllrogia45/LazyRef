//! Stickers: image and text content with transform, canvas, crop, and hit-testing.

use crate::math::Mat3;
use crate::transform::Transform;

pub enum StickerKind {
    /// Original encoded bytes (PNG/JPEG/…); decoded lazily by the shell.
    Image { data: Vec<u8>, width: u32, height: u32 },
    Text {
        text: String,
        size_px: f32,
        color: (u8, u8, u8),
        background: Option<(u8, u8, u8)>,
        bold: bool,
    },
}

pub struct Sticker {
    pub transform: Transform,
    pub canvas: Mat3,
    pub width: f32,
    pub height: f32,
    /// (left, top, right, bottom) in content px; default = full content.
    pub crop: (f32, f32, f32, f32),
    pub kind: StickerKind,
}

impl Sticker {
    pub fn new(kind: StickerKind) -> Self {
        unimplemented!()
    }
    /// Content center `(w/2, h/2)`.
    pub fn center(&self) -> (f32, f32) {
        unimplemented!()
    }
    /// Flip-aware corner winding in local space, matches the app's bound order.
    pub fn bound_points(&self) -> [f32; 8] {
        unimplemented!()
    }
    pub fn cropped_points(&self) -> [f32; 8] {
        unimplemented!()
    }
    /// canvas * transform.matrix(center).
    pub fn final_matrix(&self) -> Mat3 {
        unimplemented!()
    }
    pub fn mapped_points(&self, src: &[f32]) -> Vec<f32> {
        unimplemented!()
    }
    /// Hit test: un-rotate by -rotation, trap to rect (matches the app).
    pub fn contains(&self, x: f32, y: f32) -> bool {
        unimplemented!()
    }
    /// Clamped like the app (min 1px, inside real bounds).
    pub fn set_crop(&mut self, crop: (f32, f32, f32, f32)) {
        unimplemented!()
    }
}

/// Decode dimensions of encoded PNG/JPEG bytes. None if undecodable.
pub fn decode_image(data: &[u8]) -> Option<(u32, u32)> {
    unimplemented!()
}
