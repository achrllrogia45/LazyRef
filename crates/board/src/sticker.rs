//! Stickers: image and text content with transform, canvas, crop, and hit-testing.
//!
//! Corner winding is flip-aware: each flip state reorders the corners so mapped
//! quads keep a stable orientation. Hit-testing maps the local bounds through
//! the full pipeline, un-rotates by `-rotation`, then traps to an axis-aligned
//! rect for the containment check. Matrices are composed here from the pinned
//! `Mat3` layout directly (translate after rotate-after-scale about the content
//! center), so this lane runs standalone.

use std::io::Cursor;

use crate::math::Mat3;
use crate::transform::Transform;

#[derive(Clone, Debug)]
pub enum StickerKind {
    /// Original encoded bytes (PNG/JPEG/…); dimensions decoded at construction.
    Image { data: Vec<u8>, width: u32, height: u32 },
    Text {
        text: String,
        size_px: f32,
        color: (u8, u8, u8),
        background: Option<(u8, u8, u8)>,
        bold: bool,
    },
}

#[derive(Clone, Debug)]
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
    /// Image dimensions are decoded from `data` (kept as given when the bytes
    /// don't decode). Text starts with an empty box; the shell assigns the
    /// measured text box to `width`/`height` after construction.
    pub fn new(mut kind: StickerKind) -> Self {
        if let StickerKind::Image { data, width, height } = &mut kind {
            if let Some((w, h)) = decode_image(data) {
                *width = w;
                *height = h;
            }
        }
        let (width, height) = match &kind {
            StickerKind::Image { width, height, .. } => (*width as f32, *height as f32),
            StickerKind::Text { .. } => (0.0, 0.0),
        };
        Sticker {
            transform: Transform { pos: (0.0, 0.0), rotation: 0.0, scale: (1.0, 1.0) },
            canvas: Mat3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
            width,
            height,
            crop: (0.0, 0.0, width, height),
            kind,
        }
    }

    /// Content center `(w/2, h/2)`.
    pub fn center(&self) -> (f32, f32) {
        (self.width / 2.0, self.height / 2.0)
    }

    /// Flip-aware corner winding in local space, matches the app's bound order.
    pub fn bound_points(&self) -> [f32; 8] {
        wind(0.0, 0.0, self.width, self.height, self.flips())
    }

    pub fn cropped_points(&self) -> [f32; 8] {
        let (l, t, r, b) = self.crop;
        wind(l, t, r, b, self.flips())
    }

    /// canvas * transform.matrix(center).
    pub fn final_matrix(&self) -> Mat3 {
        mul(self.canvas, self.local_matrix())
    }

    pub fn mapped_points(&self, src: &[f32]) -> Vec<f32> {
        let m = self.final_matrix().0;
        src.chunks_exact(2)
            .flat_map(|p| [m[0] * p[0] + m[1] * p[1] + m[2], m[3] * p[0] + m[4] * p[1] + m[5]])
            .collect()
    }

    /// Hit test: un-rotate by -rotation, trap to rect (matches the app).
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let corners = self.bound_points();
        let m = self.final_matrix().0;
        let mut u = [0.0f32; 8];
        let (s, co) = self.transform.rotation.to_radians().sin_cos();
        for i in 0..4 {
            let (px, py) = (
                m[0] * corners[2 * i] + m[1] * corners[2 * i + 1] + m[2],
                m[3] * corners[2 * i] + m[4] * corners[2 * i + 1] + m[5],
            );
            u[2 * i] = px * co + py * s;
            u[2 * i + 1] = -px * s + py * co;
        }
        let (ux, uy) = (x * co + y * s, -x * s + y * co);
        let (l, t, r, b) = minmax(&u);
        // RectF.contains: inclusive on left/top, exclusive on right/bottom.
        l < r && t < b && ux >= l && ux < r && uy >= t && uy < b
    }

    /// Clamped like the app (min 1px, inside real bounds).
    pub fn set_crop(&mut self, crop: (f32, f32, f32, f32)) {
        let (l, t, r, b) = crop;
        let nr = r.min(self.width).max(1.0);
        let nb = b.min(self.height).max(1.0);
        let nl = l.max(0.0).min(nr - 1.0);
        let nt = t.max(0.0).min(nb - 1.0);
        self.crop = (nl, nt, nr, nb);
    }

    fn flips(&self) -> (bool, bool) {
        (self.transform.scale.0 < 0.0, self.transform.scale.1 < 0.0)
    }

    /// Scale about `center`, then rotate about `center`, then translate.
    fn local_matrix(&self) -> Mat3 {
        let t = &self.transform;
        let (cx, cy) = self.center();
        let (sx, sy) = t.scale;
        let s = Mat3([sx, 0.0, (1.0 - sx) * cx, 0.0, sy, (1.0 - sy) * cy, 0.0, 0.0, 1.0]);
        let (sn, co) = t.rotation.to_radians().sin_cos();
        let r = Mat3([
            co,
            -sn,
            cx - co * cx + sn * cy,
            sn,
            co,
            cy - sn * cx - co * cy,
            0.0,
            0.0,
            1.0,
        ]);
        let p = Mat3([1.0, 0.0, t.pos.0, 0.0, 1.0, t.pos.1, 0.0, 0.0, 1.0]);
        mul(mul(p, r), s)
    }
}

/// 3x3 product: `other` applies to points first.
fn mul(a: Mat3, b: Mat3) -> Mat3 {
    let (a, b) = (a.0, b.0);
    let mut c = [0.0f32; 9];
    for i in 0..3 {
        for j in 0..3 {
            c[3 * i + j] = a[3 * i] * b[j] + a[3 * i + 1] * b[3 + j] + a[3 * i + 2] * b[6 + j];
        }
    }
    Mat3(c)
}

/// Corner winding per flip state: (flip_h, flip_v).
fn wind(l: f32, t: f32, r: f32, b: f32, flip: (bool, bool)) -> [f32; 8] {
    match flip {
        (false, false) => [l, t, r, t, l, b, r, b],
        (false, true) => [l, b, r, b, l, t, r, t],
        (true, false) => [r, t, l, t, r, b, l, b],
        (true, true) => [r, b, l, b, r, t, l, t],
    }
}

/// Bounding box `(left, top, right, bottom)` of a quad.
fn minmax(pts: &[f32; 8]) -> (f32, f32, f32, f32) {
    let (mut l, mut t, mut r, mut b) = (pts[0], pts[1], pts[0], pts[1]);
    for i in 1..4 {
        l = l.min(pts[2 * i]);
        t = t.min(pts[2 * i + 1]);
        r = r.max(pts[2 * i]);
        b = b.max(pts[2 * i + 1]);
    }
    (l, t, r, b)
}

/// Decode dimensions of encoded PNG/JPEG bytes (header only, no full decode).
/// None if undecodable.
pub fn decode_image(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;
    use image::codecs::png::PngEncoder;

    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        PngEncoder::new(&mut buf)
            .write_image(&vec![0u8; (w * h * 4) as usize], w, h, image::ExtendedColorType::Rgba8)
            .unwrap();
        buf
    }

    fn text_box(w: f32, h: f32) -> Sticker {
        let mut s = Sticker::new(StickerKind::Text {
            text: String::new(),
            size_px: 12.0,
            color: (0, 0, 0),
            background: None,
            bold: false,
        });
        s.width = w;
        s.height = h;
        s.crop = (0.0, 0.0, w, h);
        s
    }

    #[test]
    fn bound_winding_all_flip_states() {
        let mut s = text_box(100.0, 50.0);
        s.transform.scale = (1.0, 1.0);
        assert_eq!(s.bound_points(), [0.0, 0.0, 100.0, 0.0, 0.0, 50.0, 100.0, 50.0]);
        s.transform.scale = (1.0, -1.0);
        assert_eq!(s.bound_points(), [0.0, 50.0, 100.0, 50.0, 0.0, 0.0, 100.0, 0.0]);
        s.transform.scale = (-1.0, 1.0);
        assert_eq!(s.bound_points(), [100.0, 0.0, 0.0, 0.0, 100.0, 50.0, 0.0, 50.0]);
        s.transform.scale = (-2.0, -3.0);
        assert_eq!(s.bound_points(), [100.0, 50.0, 0.0, 50.0, 100.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn cropped_winding_follows_flip() {
        let mut s = text_box(100.0, 50.0);
        s.set_crop((10.0, 5.0, 90.0, 40.0));
        assert_eq!(s.cropped_points(), [10.0, 5.0, 90.0, 5.0, 10.0, 40.0, 90.0, 40.0]);
        s.transform.scale = (-1.0, -1.0);
        assert_eq!(s.cropped_points(), [90.0, 40.0, 10.0, 40.0, 90.0, 5.0, 10.0, 5.0]);
    }

    #[test]
    fn center_is_half_size() {
        assert_eq!(text_box(100.0, 50.0).center(), (50.0, 25.0));
    }

    #[test]
    fn final_matrix_is_canvas_times_local() {
        let mut s = text_box(100.0, 50.0);
        s.transform.pos = (10.0, 20.0);
        assert_eq!(
            s.final_matrix().0,
            [1.0, 0.0, 10.0, 0.0, 1.0, 20.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            s.mapped_points(&[0.0, 0.0, 100.0, 0.0, 0.0, 50.0, 100.0, 50.0]),
            vec![10.0, 20.0, 110.0, 20.0, 10.0, 70.0, 110.0, 70.0]
        );
    }

    #[test]
    fn contains_plain() {
        let mut s = text_box(100.0, 50.0);
        s.transform.pos = (10.0, 20.0);
        assert!(s.contains(15.0, 25.0));
        assert!(s.contains(60.0, 45.0));
        assert!(!s.contains(200.0, 200.0));
        assert!(s.contains(10.0, 25.0)); // left/top edges inclusive
        assert!(!s.contains(110.0, 25.0)); // right/bottom edges exclusive
    }

    #[test]
    fn contains_rotated_30deg() {
        let mut s = text_box(100.0, 50.0);
        s.transform.pos = (10.0, 20.0);
        assert!(s.contains(10.0, 20.0)); // corner, inclusive at rest
        s.transform.rotation = 30.0;
        assert!(s.contains(60.0, 45.0)); // mapped center stays inside
        assert!(!s.contains(10.0, 20.0)); // old corner rotated away
        assert!(!s.contains(500.0, 500.0));
    }

    #[test]
    fn set_crop_clamping() {
        let mut s = text_box(100.0, 50.0);
        s.set_crop((10.0, 10.0, 90.0, 40.0));
        assert_eq!(s.crop, (10.0, 10.0, 90.0, 40.0));
        s.set_crop((-5.0, -5.0, 500.0, 500.0));
        assert_eq!(s.crop, (0.0, 0.0, 100.0, 50.0));
        s.set_crop((0.0, 0.0, 0.0, 0.0));
        assert_eq!(s.crop, (0.0, 0.0, 1.0, 1.0)); // min 1px
        s.set_crop((90.0, 40.0, 10.0, 5.0));
        assert_eq!(s.crop, (9.0, 4.0, 10.0, 5.0)); // left/top clamp into crop-1
    }

    #[test]
    fn decode_image_png_and_garbage() {
        assert_eq!(decode_image(&png_bytes(1, 1)), Some((1, 1)));
        assert_eq!(decode_image(&png_bytes(7, 5)), Some((7, 5)));
        assert_eq!(decode_image(b"not an image"), None);
        assert_eq!(decode_image(&[]), None);
    }

    #[test]
    fn new_image_takes_decoded_size() {
        let s = Sticker::new(StickerKind::Image { data: png_bytes(7, 5), width: 0, height: 0 });
        assert_eq!((s.width, s.height), (7.0, 5.0));
        assert_eq!(s.crop, (0.0, 0.0, 7.0, 5.0));
        match s.kind {
            StickerKind::Image { width, height, .. } => assert_eq!((width, height), (7, 5)),
            _ => panic!("expected image kind"),
        }
        let bad = Sticker::new(StickerKind::Image { data: vec![1, 2, 3], width: 0, height: 0 });
        assert_eq!((bad.width, bad.height), (0.0, 0.0));
    }

    #[test]
    fn new_text_stores_fields_empty_box() {
        let s = Sticker::new(StickerKind::Text {
            text: "hi".into(),
            size_px: 24.0,
            color: (255, 0, 0),
            background: Some((0, 0, 0)),
            bold: true,
        });
        assert_eq!((s.width, s.height), (0.0, 0.0)); // shell assigns measured box
        match &s.kind {
            StickerKind::Text { text, size_px, color, background, bold } => {
                assert_eq!(text, "hi");
                assert_eq!(
                    (*size_px, *color, *background, *bold),
                    (24.0, (255, 0, 0), Some((0, 0, 0)), true)
                );
            }
            _ => panic!("expected text kind"),
        }
    }
}
