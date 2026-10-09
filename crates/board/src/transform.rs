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
        Self {
            pos: (0.0, 0.0),
            rotation: 0.0,
            scale: (1.0, 1.0),
        }
    }
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.pos.0 += dx;
        self.pos.1 += dy;
    }
    pub fn rotate(&mut self, deg: f32) {
        self.rotation += deg;
    }
    pub fn scale_by(&mut self, sx: f32, sy: f32) {
        self.scale.0 *= sx;
        self.scale.1 *= sy;
    }
    pub fn reset(&mut self) {
        *self = Self::identity();
    }
    /// scale.0 < 0.
    pub fn flipped_h(&self) -> bool {
        self.scale.0 < 0.0
    }
    /// scale.1 < 0.
    pub fn flipped_v(&self) -> bool {
        self.scale.1 < 0.0
    }
    /// Negates scale.x only when the flip state actually changes.
    pub fn set_flipped_h(&mut self, on: bool) {
        if self.flipped_h() != on {
            self.scale.0 *= -1.0;
        }
    }
    /// Negates scale.y only when the flip state actually changes.
    pub fn set_flipped_v(&mut self, on: bool) {
        if self.flipped_v() != on {
            self.scale.1 *= -1.0;
        }
    }
    /// Local pipeline: scale about `center`, then rotate about `center`,
    /// then translate by `pos`.
    ///
    /// `concat` applies its right operand first, so the application order
    /// reads right-to-left: points hit scale, then rotation, then translation.
    pub fn matrix(&self, center: (f32, f32)) -> Mat3 {
        Mat3::translate(self.pos.0, self.pos.1)
            .concat(Mat3::rotate_about(self.rotation, center.0, center.1))
            .concat(Mat3::scale_about(self.scale.0, self.scale.1, center.0, center.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx(a: Mat3, b: Mat3) {
        for (x, y) in a.0.iter().zip(b.0.iter()) {
            assert!(
                (x - y).abs() <= 1e-5,
                "matrices differ: {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn identity_matrix() {
        assert_eq!(Transform::identity().matrix((5.0, 7.0)), Mat3::identity());
    }

    #[test]
    fn scale_center_fixed_point() {
        let t = Transform {
            pos: (0.0, 0.0),
            rotation: 0.0,
            scale: (2.0, 3.0),
        };
        assert_eq!(t.matrix((4.0, 5.0)).map_point(4.0, 5.0), (4.0, 5.0));
    }

    #[test]
    fn ops_then_reset_is_identity() {
        let mut t = Transform::identity();
        t.translate(10.0, -3.0);
        t.rotate(45.0);
        t.scale_by(2.0, 0.5);
        assert_ne!(t, Transform::identity());
        t.reset();
        assert_eq!(t, Transform::identity());
    }

    #[test]
    fn flip_toggle_roundtrip() {
        let mut t = Transform {
            pos: (0.0, 0.0),
            rotation: 0.0,
            scale: (2.0, 3.0),
        };
        assert!(!t.flipped_h());
        assert!(!t.flipped_v());
        t.set_flipped_h(true);
        t.set_flipped_v(true);
        assert!(t.flipped_h());
        assert!(t.flipped_v());
        assert_eq!(t.scale, (-2.0, -3.0));
        // Re-setting the same state must not negate again.
        t.set_flipped_h(true);
        t.set_flipped_v(true);
        assert_eq!(t.scale, (-2.0, -3.0));
        // Double toggle restores the original scale.
        t.set_flipped_h(false);
        t.set_flipped_v(false);
        assert_eq!(t.scale, (2.0, 3.0));
    }

    #[test]
    fn matrix_order_90deg_plus_translate() {
        let t = Transform {
            pos: (10.0, 20.0),
            rotation: 90.0,
            scale: (1.0, 1.0),
        };
        // Hand-computed T(10, 20) * R(90): point (1, 0) rotates CCW about
        // the origin to (0, 1), then shifts to (10, 21). Translation must
        // apply last — the reverse order would orbit (1, 0) to (-20, 11).
        assert_approx(
            t.matrix((0.0, 0.0)),
            Mat3([0.0, -1.0, 10.0, 1.0, 0.0, 20.0, 0.0, 0.0, 1.0]),
        );
        let (x, y) = t.matrix((0.0, 0.0)).map_point(1.0, 0.0);
        assert!((x - 10.0).abs() <= 1e-5 && (y - 21.0).abs() <= 1e-5);
    }
}
