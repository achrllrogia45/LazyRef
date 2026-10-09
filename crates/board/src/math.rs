//! 3x3 affine math (row-major `[a,b,tx, c,d,ty, 0,0,1]`, Android
//! `Matrix.getValues()` order) + hashing helpers.

use sha2::{Digest, Sha256};

/// Row-major 3x3 affine. `concat` applies `other` to points first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [f32; 9]);

impl Mat3 {
    pub fn identity() -> Self {
        Self([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
    }

    pub fn translate(x: f32, y: f32) -> Self {
        Self([1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0])
    }

    pub fn scale_about(sx: f32, sy: f32, px: f32, py: f32) -> Self {
        Self([sx, 0.0, px * (1.0 - sx), 0.0, sy, py * (1.0 - sy), 0.0, 0.0, 1.0])
    }

    /// Degrees, counter-clockwise positive.
    pub fn rotate_about(deg: f32, px: f32, py: f32) -> Self {
        let (s, c) = deg.to_radians().sin_cos();
        Self([c, -s, px - (c * px - s * py), s, c, py - (s * px + c * py), 0.0, 0.0, 1.0])
    }

    /// self * other: `other` applies to points first (Android post/concat semantics).
    pub fn concat(self, other: Mat3) -> Mat3 {
        let (a, b) = (self.0, other.0);
        let mut m = [0.0f32; 9];
        for r in 0..3 {
            for c in 0..3 {
                m[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
            }
        }
        Mat3(m)
    }

    pub fn invert(self) -> Option<Mat3> {
        let m = self.0;
        let det = m[0] * m[4] - m[1] * m[3];
        if det == 0.0 {
            return None;
        }
        Some(Mat3([
            m[4] / det,
            -m[1] / det,
            (m[1] * m[5] - m[4] * m[2]) / det,
            -m[3] / det,
            m[0] / det,
            (m[3] * m[2] - m[0] * m[5]) / det,
            0.0,
            0.0,
            1.0,
        ]))
    }

    /// In-place transform of flat point pairs.
    pub fn map_points(self, pts: &mut [f32]) {
        for pair in pts.chunks_exact_mut(2) {
            let (x, y) = self.map_point(pair[0], pair[1]);
            pair[0] = x;
            pair[1] = y;
        }
    }

    pub fn map_point(self, x: f32, y: f32) -> (f32, f32) {
        let m = self.0;
        (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
    }
}

/// `atan2(y, x)` in degrees (two-finger rotation gesture).
pub fn angle_deg(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    (y1 - y2).atan2(x1 - x2).to_degrees()
}

/// Euclidean distance (two-finger pinch gesture).
pub fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    (x1 - x2).hypot(y1 - y2)
}

/// Bounding box `(left, top, right, bottom)` of a mapped quad.
pub fn trap_to_rect(pts: &[f32; 8]) -> (f32, f32, f32, f32) {
    let mut l = f32::INFINITY;
    let mut t = f32::INFINITY;
    let mut r = f32::NEG_INFINITY;
    let mut b = f32::NEG_INFINITY;
    for pair in pts.chunks_exact(2) {
        // 0.1px rounding matches the app's bounds snap.
        let x = (pair[0] * 10.0).round() / 10.0;
        let y = (pair[1] * 10.0).round() / 10.0;
        l = l.min(x);
        t = t.min(y);
        r = r.max(x);
        b = b.max(y);
    }
    (l, t, r, b)
}

/// Lowercase hex SHA-256 (bitmap dedup keys).
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() <= 1e-4
    }

    fn approx_mat(a: Mat3, b: Mat3) -> bool {
        a.0.iter().zip(b.0.iter()).all(|(x, y)| approx(*x, *y))
    }

    #[test]
    fn identity_roundtrip() {
        let m = Mat3::identity();
        assert_eq!(m.map_point(3.0, -2.0), (3.0, -2.0));
        let mut pts = [1.0f32, 2.0, -5.0, 0.5];
        m.map_points(&mut pts);
        assert_eq!(pts, [1.0, 2.0, -5.0, 0.5]);
    }

    #[test]
    fn scale_about_pivot_fixed() {
        let m = Mat3::scale_about(2.0, 3.0, 5.0, 7.0);
        assert_eq!(m.map_point(5.0, 7.0), (5.0, 7.0));
        assert_eq!(m.map_point(6.0, 8.0), (7.0, 10.0));
    }

    #[test]
    fn rotate_90_about_origin() {
        let m = Mat3::rotate_about(90.0, 0.0, 0.0);
        let (x, y) = m.map_point(1.0, 0.0);
        assert!(approx(x, 0.0) && approx(y, 1.0), "got ({x}, {y})");
    }

    #[test]
    fn concat_applies_other_first() {
        let t = Mat3::translate(10.0, 0.0);
        let s = Mat3::scale_about(2.0, 2.0, 0.0, 0.0);
        assert_eq!(t.concat(s).map_point(1.0, 1.0), (12.0, 2.0));
        assert_eq!(s.concat(t).map_point(1.0, 1.0), (22.0, 2.0));
    }

    #[test]
    fn invert_rotation_translation() {
        let m = Mat3::translate(5.0, -3.0).concat(Mat3::rotate_about(30.0, 0.0, 0.0));
        let inv = m.invert().expect("invertible");
        assert!(approx_mat(m.concat(inv), Mat3::identity()));
        let (x, y) = m.map_point(1.0, 2.0);
        let (x2, y2) = inv.map_point(x, y);
        assert!(approx(x2, 1.0) && approx(y2, 2.0));
        assert!(Mat3::scale_about(0.0, 1.0, 0.0, 0.0).invert().is_none());
    }

    #[test]
    fn trap_to_rect_of_rotated_quad() {
        let m = Mat3::rotate_about(90.0, 0.0, 0.0);
        let mut quad = [0.0f32, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0];
        m.map_points(&mut quad);
        let (l, t, r, b) = trap_to_rect(&quad);
        assert!(approx(l, -1.0) && approx(t, 0.0) && approx(r, 0.0) && approx(b, 1.0));
    }

    #[test]
    fn angle_and_dist_known_values() {
        assert_eq!(dist(0.0, 0.0, 3.0, 4.0), 5.0);
        assert!(approx(angle_deg(1.0, 0.0, 0.0, 0.0), 0.0));
        assert!(approx(angle_deg(1.0, 1.0, 0.0, 0.0), 45.0));
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
