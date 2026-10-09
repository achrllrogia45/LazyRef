//! 3x3 affine math (row-major `[a,b,tx, c,d,ty, 0,0,1]`, Android
//! `Matrix.getValues()` order) + hashing helpers.

/// Row-major 3x3 affine. `concat` applies `other` to points first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [f32; 9]);

impl Mat3 {
    pub fn identity() -> Self {
        unimplemented!()
    }
    pub fn translate(x: f32, y: f32) -> Self {
        unimplemented!()
    }
    pub fn scale_about(sx: f32, sy: f32, px: f32, py: f32) -> Self {
        unimplemented!()
    }
    pub fn rotate_about(deg: f32, px: f32, py: f32) -> Self {
        unimplemented!()
    }
    /// self * other: `other` applies to points first (Android post/concat semantics).
    pub fn concat(self, other: Mat3) -> Mat3 {
        unimplemented!()
    }
    pub fn invert(self) -> Option<Mat3> {
        unimplemented!()
    }
    /// In-place transform of flat point pairs.
    pub fn map_points(self, pts: &mut [f32]) {
        unimplemented!()
    }
    pub fn map_point(self, x: f32, y: f32) -> (f32, f32) {
        unimplemented!()
    }
}

/// `atan2(y, x)` in degrees (two-finger rotation gesture).
pub fn angle_deg(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    unimplemented!()
}

/// Euclidean distance (two-finger pinch gesture).
pub fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    unimplemented!()
}

/// Bounding box `(left, top, right, bottom)` of a mapped quad.
pub fn trap_to_rect(pts: &[f32; 8]) -> (f32, f32, f32, f32) {
    unimplemented!()
}

/// Lowercase hex SHA-256 (bitmap dedup keys).
pub fn sha256_hex(bytes: &[u8]) -> String {
    unimplemented!()
}
