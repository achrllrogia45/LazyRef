//! Per-sticker local transform: position + rotation + scale.
//!
//! Matrix order matches the app: scale about the content center, then rotate
//! about the center, then translate.

use crate::math::Mat3;

/// All angles in degrees, counter-clockwise positive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    pub pos: (f32, f32),
    pub rotation: f32,
    pub scale: (f32, f32),
}

impl Transform {
    /// pos 0, rot 0, scale 1.
    pub fn identity() -> Self {
        unimplemented!()
    }
    pub fn translate(&mut self, dx: f32, dy: f32) {
        unimplemented!()
    }
    pub fn rotate(&mut self, deg: f32) {
        unimplemented!()
    }
    pub fn scale_by(&mut self, sx: f32, sy: f32) {
        unimplemented!()
    }
    pub fn reset(&mut self) {
        unimplemented!()
    }
    /// scale.0 < 0.
    pub fn flipped_h(&self) -> bool {
        unimplemented!()
    }
    pub fn flipped_v(&self) -> bool {
        unimplemented!()
    }
    pub fn set_flipped_h(&mut self, on: bool) {
        unimplemented!()
    }
    pub fn set_flipped_v(&mut self, on: bool) {
        unimplemented!()
    }
    /// Local pipeline: scale about `center`, then rotate about `center`,
    /// then translate by `pos`.
    pub fn matrix(&self, center: (f32, f32)) -> Mat3 {
        unimplemented!()
    }
}
