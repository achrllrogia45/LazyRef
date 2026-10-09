//! Board icons: edit-handle operations and crop-gravity mapping.
//!
//! Pure transform ops (no Board dependency — the board calls these).

use crate::transform::Transform;

// Gravity constants for crop/scale handles (match the app's icon gravities).
pub const LEFT_TOP: u32 = 0;
pub const RIGHT_TOP: u32 = 1;
pub const LEFT_BOTTOM: u32 = 2;
pub const RIGHT_BOTTOM: u32 = 3;
pub const LEFT_CENTER: u32 = 4;
pub const RIGHT_CENTER: u32 = 5;
pub const TOP_CENTER: u32 = 6;
pub const BOTTOM_CENTER: u32 = 7;

pub enum IconOp {
    Delete,
    ScaleStep(f32),
    RotateLeft,
    RotateRight,
    FlipH,
    FlipV,
    FlipBoth,
}

/// Apply an icon operation to a sticker transform.
pub fn apply_op(t: &mut Transform, op: &IconOp) {
    unimplemented!()
}

/// Crop-edge drag under flips: map a gravity through the sticker's flip state.
pub fn flip_gravity(gravity: u32, flip_h: bool, flip_v: bool) -> u32 {
    unimplemented!()
}

/// Touch slop for icon hit-testing (matches the app's generous icon touch area).
pub const ICON_TOUCH_RADIUS: f32 = 48.0;
