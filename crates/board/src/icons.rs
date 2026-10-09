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

#[derive(Clone, Copy, Debug, PartialEq)]
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
///
/// Delete is a no-op marker: removing the sticker is the board's job.
pub fn apply_op(t: &mut Transform, op: &IconOp) {
    match *op {
        IconOp::Delete => {}
        // Zoom handle scales both axes together.
        IconOp::ScaleStep(f) => {
            t.scale.0 *= f;
            t.scale.1 *= f;
        }
        // Rotate handles turn a quarter turn per tap.
        IconOp::RotateLeft => t.rotation -= 90.0,
        IconOp::RotateRight => t.rotation += 90.0,
        // Flip handles toggle mirror state, stored as negative scale.
        IconOp::FlipH => t.scale.0 = -t.scale.0,
        IconOp::FlipV => t.scale.1 = -t.scale.1,
        IconOp::FlipBoth => {
            t.scale.0 = -t.scale.0;
            t.scale.1 = -t.scale.1;
        }
    }
}

/// Crop-edge drag under flips: map a gravity through the sticker's flip state.
///
/// A mirrored sticker drags the mirrored edge, so the gravity is remapped.
pub fn flip_gravity(gravity: u32, flip_h: bool, flip_v: bool) -> u32 {
    match (flip_h, flip_v) {
        (true, true) => match gravity {
            LEFT_TOP => RIGHT_BOTTOM,
            LEFT_BOTTOM => RIGHT_TOP,
            RIGHT_TOP => LEFT_BOTTOM,
            RIGHT_BOTTOM => LEFT_TOP,
            LEFT_CENTER => RIGHT_CENTER,
            RIGHT_CENTER => LEFT_CENTER,
            TOP_CENTER => BOTTOM_CENTER,
            BOTTOM_CENTER => TOP_CENTER,
            g => g,
        },
        (true, false) => match gravity {
            LEFT_TOP => RIGHT_TOP,
            LEFT_BOTTOM => RIGHT_BOTTOM,
            RIGHT_TOP => LEFT_TOP,
            RIGHT_BOTTOM => LEFT_BOTTOM,
            LEFT_CENTER => RIGHT_CENTER,
            RIGHT_CENTER => LEFT_CENTER,
            g => g,
        },
        (false, true) => match gravity {
            LEFT_TOP => LEFT_BOTTOM,
            LEFT_BOTTOM => LEFT_TOP,
            RIGHT_TOP => RIGHT_BOTTOM,
            RIGHT_BOTTOM => RIGHT_TOP,
            TOP_CENTER => BOTTOM_CENTER,
            BOTTOM_CENTER => TOP_CENTER,
            g => g,
        },
        (false, false) => gravity,
    }
}

/// Touch slop for icon hit-testing (matches the app's generous icon touch area).
pub const ICON_TOUCH_RADIUS: f32 = 48.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Transform {
        Transform {
            pos: (0.0, 0.0),
            rotation: 0.0,
            scale: (1.0, 1.0),
        }
    }

    #[test]
    fn gravities_distinct() {
        let g = [
            LEFT_TOP,
            RIGHT_TOP,
            LEFT_BOTTOM,
            RIGHT_BOTTOM,
            LEFT_CENTER,
            RIGHT_CENTER,
            TOP_CENTER,
            BOTTOM_CENTER,
        ];
        for (i, a) in g.iter().enumerate() {
            for (j, b) in g.iter().enumerate() {
                assert_eq!(a == b, i == j);
            }
        }
    }

    #[test]
    fn touch_radius_pinned() {
        assert_eq!(ICON_TOUCH_RADIUS, 48.0);
    }

    #[test]
    fn scale_step_multiplies_both_axes() {
        let mut x = fresh();
        apply_op(&mut x, &IconOp::ScaleStep(1.1));
        assert_eq!(x.scale, (1.1, 1.1));
        assert_eq!(x.rotation, 0.0);
    }

    #[test]
    fn rotate_handles_quarter_turns() {
        let mut x = fresh();
        apply_op(&mut x, &IconOp::RotateRight);
        assert_eq!(x.rotation, 90.0);
        apply_op(&mut x, &IconOp::RotateLeft);
        assert_eq!(x.rotation, 0.0);
        apply_op(&mut x, &IconOp::RotateLeft);
        assert_eq!(x.rotation, -90.0);
    }

    #[test]
    fn flip_handles_toggle_sign() {
        let mut x = fresh();
        apply_op(&mut x, &IconOp::FlipH);
        assert_eq!(x.scale, (-1.0, 1.0));
        apply_op(&mut x, &IconOp::FlipH);
        assert_eq!(x.scale, (1.0, 1.0));
        apply_op(&mut x, &IconOp::FlipV);
        assert_eq!(x.scale, (1.0, -1.0));
        apply_op(&mut x, &IconOp::FlipBoth);
        assert_eq!(x.scale, (-1.0, 1.0));
    }

    #[test]
    fn delete_is_noop_marker() {
        let mut x = fresh();
        apply_op(&mut x, &IconOp::Delete);
        assert_eq!(x, fresh());
    }

    #[test]
    fn flip_gravity_all_states() {
        // (gravity, flip_h, flip_v, expected): 8 gravities x 4 flip states.
        let cases = [
            (LEFT_TOP, false, false, LEFT_TOP),
            (RIGHT_TOP, false, false, RIGHT_TOP),
            (LEFT_BOTTOM, false, false, LEFT_BOTTOM),
            (RIGHT_BOTTOM, false, false, RIGHT_BOTTOM),
            (LEFT_CENTER, false, false, LEFT_CENTER),
            (RIGHT_CENTER, false, false, RIGHT_CENTER),
            (TOP_CENTER, false, false, TOP_CENTER),
            (BOTTOM_CENTER, false, false, BOTTOM_CENTER),
            (LEFT_TOP, true, false, RIGHT_TOP),
            (RIGHT_TOP, true, false, LEFT_TOP),
            (LEFT_BOTTOM, true, false, RIGHT_BOTTOM),
            (RIGHT_BOTTOM, true, false, LEFT_BOTTOM),
            (LEFT_CENTER, true, false, RIGHT_CENTER),
            (RIGHT_CENTER, true, false, LEFT_CENTER),
            (TOP_CENTER, true, false, TOP_CENTER),
            (BOTTOM_CENTER, true, false, BOTTOM_CENTER),
            (LEFT_TOP, false, true, LEFT_BOTTOM),
            (LEFT_BOTTOM, false, true, LEFT_TOP),
            (RIGHT_TOP, false, true, RIGHT_BOTTOM),
            (RIGHT_BOTTOM, false, true, RIGHT_TOP),
            (LEFT_CENTER, false, true, LEFT_CENTER),
            (RIGHT_CENTER, false, true, RIGHT_CENTER),
            (TOP_CENTER, false, true, BOTTOM_CENTER),
            (BOTTOM_CENTER, false, true, TOP_CENTER),
            (LEFT_TOP, true, true, RIGHT_BOTTOM),
            (LEFT_BOTTOM, true, true, RIGHT_TOP),
            (RIGHT_TOP, true, true, LEFT_BOTTOM),
            (RIGHT_BOTTOM, true, true, LEFT_TOP),
            (LEFT_CENTER, true, true, RIGHT_CENTER),
            (RIGHT_CENTER, true, true, LEFT_CENTER),
            (TOP_CENTER, true, true, BOTTOM_CENTER),
            (BOTTOM_CENTER, true, true, TOP_CENTER),
        ];
        for (g, h, v, want) in cases {
            assert_eq!(flip_gravity(g, h, v), want, "g={g} h={h} v={v}");
        }
    }
}
