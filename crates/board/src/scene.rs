//! Pure `.pur` <-> board math (f64). No IO, no platform types.
//!
//! `[f64; 6]` layout is `W = [m11, m21, m31, m12, m22, m32]`
//! (row-vector affine: `x' = m11*x + m21*y + m31`).

/// Import state for one visible image item, ready for a sticker transform.
#[derive(Debug)]
pub struct StickerDecomp {
    pub pos_x: f64,
    pub pos_y: f64,
    pub rotation_deg: f64,
    pub scale_x: f64,
    pub scale_y: f64,
}

/// Export result for one sticker: both `.pur` transforms as 6-tuples.
#[derive(Debug)]
pub struct ItemTransforms {
    pub image_transform: [f64; 6],
    pub item_transform: [f64; 6],
}

/// Element types of [`rect_path5`]: MoveTo, LineTo x3, LineTo back to start.
pub const RECT_PATH_TYPES: [i32; 5] = [0, 1, 1, 1, 1];

/// Qt serialized 9 -> W 6-tuple.
pub fn qt9_to_w(qt: &[f64; 9]) -> [f64; 6] {
    [qt[0], qt[3], qt[6], qt[1], qt[4], qt[7]]
}

/// W 6-tuple -> Qt serialized 9 (pure affine: m13 = m23 = 0, m33 = 1).
pub fn w_to_qt9(w: &[f64; 6]) -> [f64; 9] {
    [w[0], w[3], 0.0, w[1], w[4], 0.0, w[2], w[5], 1.0]
}

/// Composition `m1 after m2` (apply `m2` first, then `m1`).
pub fn affine_mul(m1: &[f64; 6], m2: &[f64; 6]) -> [f64; 6] {
    [
        m1[0] * m2[0] + m1[1] * m2[3],
        m1[0] * m2[1] + m1[1] * m2[4],
        m1[0] * m2[2] + m1[1] * m2[5] + m1[2],
        m1[3] * m2[0] + m1[4] * m2[3],
        m1[3] * m2[1] + m1[4] * m2[4],
        m1[3] * m2[2] + m1[4] * m2[5] + m1[5],
    ]
}

/// Decompose a pixel->scene affine into sticker state:
/// `pos = W(c) - c`, `theta = atan2(m12, m11)`, `sx = hypot(m11, m12)`,
/// `sy = det/sx`. `c` is the sticker center; `det < 0` yields `sy < 0`.
pub fn decompose_to_sticker(w: &[f64; 6], c: (f64, f64)) -> StickerDecomp {
    let (m11, m21, m31) = (w[0], w[1], w[2]);
    let (m12, m22, m32) = (w[3], w[4], w[5]);
    let sx = m11.hypot(m12);
    let det = m11 * m22 - m21 * m12;
    StickerDecomp {
        pos_x: m11 * c.0 + m21 * c.1 + m31 - c.0,
        pos_y: m12 * c.0 + m22 * c.1 + m32 - c.1,
        rotation_deg: m12.atan2(m11).to_degrees(),
        scale_x: sx,
        scale_y: if sx == 0.0 { 0.0 } else { det / sx },
    }
}

/// Build the two `.pur` transforms for a sticker: `image_transform = T(-c)`,
/// `item.transform = affine(A, A*c + t)`, so that `T_item after T_img = W`.
/// `a` is `[m11, m21, m12, m22]`, `t` is `(m31, m32)`, `c` the crop-rect
/// center in pixel coords.
pub fn compose_from_sticker(a: [f64; 4], t: (f64, f64), c: (f64, f64)) -> ItemTransforms {
    ItemTransforms {
        image_transform: [1.0, 0.0, -c.0, 0.0, 1.0, -c.1],
        item_transform: [
            a[0],
            a[1],
            a[0] * c.0 + a[1] * c.1 + t.0,
            a[2],
            a[3],
            a[2] * c.0 + a[3] * c.1 + t.1,
        ],
    }
}

/// `image_bounds` for a crop rect: 5 points in the centered frame.
pub fn rect_path5(cl: f64, ct: f64, cr: f64, cb: f64, cx: f64, cy: f64) -> [f64; 10] {
    [
        cl - cx,
        ct - cy,
        cr - cx,
        ct - cy,
        cr - cx,
        cb - cy,
        cl - cx,
        cb - cy,
        cl - cx,
        ct - cy,
    ]
}

/// Slot reuse for ordering: the i-th item reuses `existing[i]` while slots
/// last, then `new_slot(rank)` with rank 1, 2, … (`existing` pre-sorted ascending).
pub fn assign_slots<T: Clone>(existing: &[T], count: usize, new_slot: impl Fn(usize) -> T) -> Vec<T> {
    (0..count)
        .map(|i| {
            if i < existing.len() {
                existing[i].clone()
            } else {
                new_slot(i - existing.len() + 1)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    fn close_arr(a: &[f64], b: &[f64], eps: f64) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| close(*x, *y, eps))
    }

    #[test]
    fn qt_round_trip() {
        let qt = [2.0, 0.5, 0.0, -0.25, 3.0, 0.0, 100.0, -50.0, 1.0];
        let w = qt9_to_w(&qt);
        assert_eq!(w, [2.0, -0.25, 100.0, 0.5, 3.0, -50.0]);
        assert_eq!(w_to_qt9(&w), qt);
    }

    #[test]
    fn decompose_identity() {
        let d = decompose_to_sticker(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0], (7.0, 3.0));
        assert_eq!((d.pos_x, d.pos_y), (0.0, 0.0));
        assert_eq!(d.rotation_deg, 0.0);
        assert_eq!((d.scale_x, d.scale_y), (1.0, 1.0));
    }

    #[test]
    fn decompose_vertical_flip() {
        let d = decompose_to_sticker(&[2.0, 0.0, 0.0, 0.0, -3.0, 0.0], (0.0, 0.0));
        assert_eq!(d.scale_x, 2.0);
        assert!(d.scale_y < 0.0);
        assert_eq!(d.scale_y, -3.0);
    }

    #[test]
    fn decompose_compose_round_trip() {
        let c = (50.0, 30.0);
        // (theta_deg, sx, sy, tx, ty)
        let cases: [(f64, f64, f64, f64, f64); 5] = [
            (0.0, 1.0, 1.0, 0.0, 0.0),
            (37.0, 1.0, 1.0, 10.0, 20.0),
            (0.0, 2.0, 3.0, -5.0, 7.5),
            (25.0, 1.5, -2.0, 100.0, -100.0),
            (-100.0, 0.75, 2.5, -4.0, 9.0),
        ];
        for (deg, sx, sy, tx, ty) in cases {
            let th = deg.to_radians();
            let w = [
                sx * th.cos(),
                -sy * th.sin(),
                tx,
                sx * th.sin(),
                sy * th.cos(),
                ty,
            ];
            let d = decompose_to_sticker(&w, c);
            assert!(close(d.scale_x, sx, 1e-12), "sx {d:?} vs {sx}");
            assert!(close(d.scale_y, sy, 1e-12), "sy {d:?} vs {sy}");
            let rad = d.rotation_deg.to_radians();
            let a = [
                d.scale_x * rad.cos(),
                -d.scale_y * rad.sin(),
                d.scale_x * rad.sin(),
                d.scale_y * rad.cos(),
            ];
            let t = (
                d.pos_x + c.0 - (a[0] * c.0 + a[1] * c.1),
                d.pos_y + c.1 - (a[2] * c.0 + a[3] * c.1),
            );
            let comp = compose_from_sticker(a, t, c);
            assert_eq!(
                comp.image_transform,
                [1.0, 0.0, -c.0, 0.0, 1.0, -c.1]
            );
            let back = affine_mul(&comp.item_transform, &comp.image_transform);
            assert!(close_arr(&back, &w, 1e-9), "{back:?} vs {w:?}");
        }
    }

    #[test]
    fn affine_mul_translation() {
        let ta = [1.0, 0.0, 5.0, 0.0, 1.0, 7.0];
        let tb = [1.0, 0.0, 10.0, 0.0, 1.0, 20.0];
        assert_eq!(affine_mul(&ta, &tb), [1.0, 0.0, 15.0, 0.0, 1.0, 27.0]);
        let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        assert_eq!(affine_mul(&id, &ta), ta);
        assert_eq!(affine_mul(&ta, &id), ta);
    }

    #[test]
    fn rect_path_closure() {
        let p = rect_path5(10.0, 20.0, 110.0, 220.0, 60.0, 120.0);
        assert_eq!(
            p,
            [-50.0, -100.0, 50.0, -100.0, 50.0, 100.0, -50.0, 100.0, -50.0, -100.0]
        );
        assert_eq!((p[0], p[1]), (p[8], p[9]));
        assert_eq!(RECT_PATH_TYPES, [0, 1, 1, 1, 1]);
    }

    #[test]
    fn slot_assignment() {
        let v = assign_slots(&[1.0, 4.0], 5, |rank| 4.0 + rank as f64);
        assert_eq!(v, vec![1.0, 4.0, 5.0, 6.0, 7.0]);
        let v = assign_slots::<f64>(&[], 1, |rank| 8.0 + rank as f64);
        assert_eq!(v, vec![9.0]);
    }
}
