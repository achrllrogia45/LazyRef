//! Pure `.pur` <-> board math (f64). No IO, no platform types.
//!
//! `[f64; 6]` layout is `W = [m11, m21, m31, m12, m22, m32]`
//! (row-vector affine: `x' = m11*x + m21*y + m31`).

/// Import state for one visible image item, ready for a sticker transform.
pub struct StickerDecomp {
    pub pos_x: f64,
    pub pos_y: f64,
    pub rotation_deg: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// Export result for one sticker: both `.pur` transforms as 6-tuples.
pub struct ItemTransforms {
    pub image_transform: [f64; 6],
    pub item_transform: [f64; 6],
}

/// Element types of [`rect_path5`]: MoveTo, LineTo x3, LineTo back to start.
pub const RECT_PATH_TYPES: [i32; 5] = [0, 1, 1, 1, 1];

/// Qt serialized 9 -> W 6-tuple.
pub fn qt9_to_w(qt: &[f64; 9]) -> [f64; 6] {
    unimplemented!()
}

/// W 6-tuple -> Qt serialized 9 (pure affine: m13 = m23 = 0, m33 = 1).
pub fn w_to_qt9(w: &[f64; 6]) -> [f64; 9] {
    unimplemented!()
}

/// Composition `m1 after m2` (apply `m2` first, then `m1`).
pub fn affine_mul(m1: &[f64; 6], m2: &[f64; 6]) -> [f64; 6] {
    unimplemented!()
}

/// Decompose a pixel->scene affine into sticker state:
/// `pos = W(c) - c`, `theta = atan2(m12, m11)`, `sx = hypot(m11, m12)`,
/// `sy = det/sx`. `c` is the sticker center; `det < 0` yields `sy < 0`.
pub fn decompose_to_sticker(w: &[f64; 6], c: (f64, f64)) -> StickerDecomp {
    unimplemented!()
}

/// Build the two `.pur` transforms for a sticker: `image_transform = T(-c)`,
/// `item.transform = affine(A, A*c + t)`, so that `T_item after T_img = W`.
/// `a` is `[m11, m21, m12, m22]`, `t` is `(m31, m32)`, `c` the crop-rect
/// center in pixel coords.
pub fn compose_from_sticker(a: [f64; 4], t: (f64, f64), c: (f64, f64)) -> ItemTransforms {
    unimplemented!()
}

/// `image_bounds` for a crop rect: 5 points in the centered frame.
pub fn rect_path5(cl: f64, ct: f64, cr: f64, cb: f64, cx: f64, cy: f64) -> [f64; 10] {
    unimplemented!()
}

/// Slot reuse for ordering: the i-th item reuses `existing[i]` while slots
/// last, then `new_slot(rank)` with rank 1, 2, … (`existing` pre-sorted ascending).
pub fn assign_slots<T: Clone>(existing: &[T], count: usize, new_slot: impl Fn(usize) -> T) -> Vec<T> {
    unimplemented!()
}
