//! End-to-end flows across the board modules: save/load round-trips, scene
//! import/export, pointer handling, and wire-format interop.
//!
//! Each test pins behavior from two or more modules at once. Tests that need
//! sibling modules still under construction are marked `#[ignore]` with the
//! blocker named, so the suite stays green until every lane lands.

use lazyref_board::board::{Board, BoardEvent, Pointer};
use lazyref_board::scene;
use lazyref_board::serializer;
use lazyref_board::settings::Settings;
use lazyref_board::sticker::{decode_image, Sticker, StickerKind};
use std::path::PathBuf;

const PNG_MAGIC: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

fn png_1x1(r: u8, g: u8, b: u8) -> Vec<u8> {
    use image::ImageEncoder;
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(&[r, g, b, 255], 1, 1, image::ExtendedColorType::Rgba8)
        .expect("png encode");
    buf
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lazyref-board-itest-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn fixture_s1() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../purcore/tests/fixtures/S1_redbar-new.pur");
    std::fs::read(&path).expect("S1 fixture")
}

fn image_stats(board: &Board) -> (usize, usize) {
    let mut count = 0;
    let mut bytes = 0;
    for s in board.stickers() {
        if let StickerKind::Image { data, .. } = &s.kind {
            count += 1;
            bytes += data.len();
        }
    }
    (count, bytes)
}

#[test]
fn ref_roundtrip_two_images_plus_text() {
    use lazyref_board::io::{open_ref, save_ref};
    use lazyref_board::math::Mat3;

    let red = png_1x1(255, 0, 0);
    let blue = png_1x1(0, 0, 255);

    let mut a = Sticker::new(StickerKind::Image {
        data: red.clone(),
        width: 1,
        height: 1,
    });
    a.width = 1.0;
    a.height = 1.0;
    a.transform.pos = (10.0, 20.0);
    a.transform.rotation = 30.0;
    a.transform.scale = (-2.0, 3.0); // flipped horizontally
    a.crop = (0.0, 0.0, 1.0, 1.0);

    let mut b = Sticker::new(StickerKind::Image {
        data: blue.clone(),
        width: 1,
        height: 1,
    });
    b.width = 1.0;
    b.height = 1.0;
    b.transform.pos = (-5.0, 40.0);
    b.transform.rotation = -45.0;
    b.transform.scale = (1.5, -1.5); // flipped vertically
    b.crop = (0.0, 0.0, 1.0, 1.0);

    let mut t = Sticker::new(StickerKind::Text {
        text: "hello".to_string(),
        size_px: 24.0,
        color: (10, 20, 30),
        background: Some((1, 2, 3)),
        bold: true,
    });
    t.width = 100.0;
    t.height = 30.0;
    t.transform.pos = (7.0, 8.0);

    let canvas = Mat3([1.0, 0.0, 5.0, 0.0, 1.0, 7.0, 0.0, 0.0, 1.0]);
    let mut board = Board::new();
    board.canvas = canvas;
    board.add_sticker(a);
    board.add_sticker(b);
    board.add_sticker(t);

    let back = open_ref(&save_ref(&board)).expect("open");
    assert_eq!(back.stickers().len(), 3);
    assert_eq!(back.canvas_matrix(), canvas);

    let find_image = |data: &[u8]| {
        back.stickers()
            .iter()
            .find(|s| matches!(&s.kind, StickerKind::Image { data: d, .. } if d == data))
            .expect("image sticker survives")
    };
    let ra = find_image(&red);
    assert_eq!(ra.transform.pos, (10.0, 20.0));
    assert_eq!(ra.transform.rotation, 30.0);
    assert_eq!(ra.transform.scale, (-2.0, 3.0));
    assert_eq!(ra.crop, (0.0, 0.0, 1.0, 1.0));
    assert!(ra.transform.flipped_h());
    assert!(!ra.transform.flipped_v());
    let rb = find_image(&blue);
    assert_eq!(rb.transform.pos, (-5.0, 40.0));
    assert_eq!(rb.transform.rotation, -45.0);
    assert_eq!(rb.transform.scale, (1.5, -1.5));
    assert!(rb.transform.flipped_v());

    let text = back
        .stickers()
        .iter()
        .find(|s| matches!(&s.kind, StickerKind::Text { .. }))
        .expect("text sticker survives");
    match &text.kind {
        StickerKind::Text {
            text,
            size_px,
            color,
            background,
            bold,
        } => {
            assert_eq!(text, "hello");
            assert_eq!(*size_px, 24.0);
            assert_eq!(*color, (10, 20, 30));
            assert_eq!(*background, Some((1, 2, 3)));
            assert!(*bold);
        }
        StickerKind::Image { .. } => unreachable!(),
    }
    assert_eq!(text.transform.pos, (7.0, 8.0));
}

#[test]
fn pur_open_s1_fixture_has_png_sticker() {
    use lazyref_board::io::open_pur;

    let board = open_pur(&fixture_s1(), &scratch_dir("open")).expect("open S1");
    assert!(!board.stickers().is_empty(), "S1 imports stickers");
    let (count, _) = image_stats(&board);
    assert!(count >= 1, "S1 imports image stickers");
    let has_png = board.stickers().iter().any(|s| match &s.kind {
        StickerKind::Image { data, .. } => data.starts_with(&PNG_MAGIC),
        _ => false,
    });
    assert!(has_png, "imported image bytes are PNG");
    let s = &board.stickers()[0];
    for v in [s.transform.scale.0.abs(), s.transform.scale.1.abs()] {
        assert!((v - 15.625).abs() < 0.05, "S1 sticker scale ~15.625, got {v}");
    }
}

#[test]
fn pur_save_reopen_preserves_images() {
    use lazyref_board::io::{open_pur, save_pur};

    let scratch = scratch_dir("resave");
    let board = open_pur(&fixture_s1(), &scratch).expect("open");
    let before = image_stats(&board);
    assert!(before.0 >= 1);
    let out = save_pur(&board, &scratch, &[]);
    let back = open_pur(&out, &scratch).expect("reopen");
    assert_eq!(image_stats(&back), before, "image count and byte length survive");
}

#[test]
fn scene_compose_decompose_roundtrip() {
    let deg = 30.0f64;
    let (s, p, c) = (2.5f64, (10.0f64, -7.0f64), (64.0f64, 32.0f64));
    let (sin, cos) = deg.to_radians().sin_cos();
    let a = [s * cos, -s * sin, s * sin, s * cos];
    // Sticker matrix is affine(A, t) with t = c + p - A*c.
    let t = (
        c.0 + p.0 - (a[0] * c.0 + a[1] * c.1),
        c.1 + p.1 - (a[2] * c.0 + a[3] * c.1),
    );
    let it = scene::compose_from_sticker(a, t, c);
    let w = scene::affine_mul(&it.item_transform, &it.image_transform);
    let d = scene::decompose_to_sticker(&w, c);
    let eps = 1e-9;
    assert!((d.pos_x - p.0).abs() < eps, "pos_x round-trips");
    assert!((d.pos_y - p.1).abs() < eps, "pos_y round-trips");
    let mut dr = (d.rotation_deg - deg).abs() % 360.0;
    if dr > 180.0 {
        dr = 360.0 - dr;
    }
    assert!(dr < eps, "rotation round-trips");
    assert!((d.scale_x - s).abs() < eps, "scale_x round-trips");
    assert!((d.scale_y - s).abs() < eps, "scale_y round-trips");
}

#[test]
fn board_drag_and_canvas_pan_flow() {
    let mut s = Sticker::new(StickerKind::Image {
        data: png_1x1(1, 2, 3),
        width: 1,
        height: 1,
    });
    s.width = 200.0;
    s.height = 100.0;
    s.set_crop((0.0, 0.0, 200.0, 100.0));
    s.transform.pos = (100.0, 100.0);

    let mut board = Board::new();
    let idx = board.add_sticker(s);
    assert_eq!(board.handling, Some(idx));

    // Press inside the sticker and drag by (+20, +10).
    let down = board.handle_event(BoardEvent::Down(Pointer { id: 1, x: 150.0, y: 130.0 }));
    assert!(!down.is_empty());
    assert_eq!(board.handling, Some(idx));
    board.handle_event(BoardEvent::Move(Pointer { id: 1, x: 170.0, y: 140.0 }));
    let pos = board.stickers()[idx].transform.pos;
    assert!(
        (pos.0 - 120.0).abs() < 1e-4 && (pos.1 - 110.0).abs() < 1e-4,
        "drag moves sticker, got {pos:?}"
    );
    board.handle_event(BoardEvent::Up(Pointer { id: 1, x: 170.0, y: 140.0 }));

    // Press empty canvas and drag: the canvas pans, the sticker stays.
    let before_canvas = board.canvas_matrix();
    board.handle_event(BoardEvent::Down(Pointer { id: 2, x: -1000.0, y: -1000.0 }));
    board.handle_event(BoardEvent::Move(Pointer { id: 2, x: -970.0, y: -990.0 }));
    board.handle_event(BoardEvent::Up(Pointer { id: 2, x: -970.0, y: -990.0 }));
    assert_ne!(board.canvas_matrix(), before_canvas, "canvas drag pans");
    assert!((board.stickers()[idx].transform.pos.0 - 120.0).abs() < 1e-4);
}

// Minimal MessagePack writers mirroring the documented wire order. These build
// the interop probe bytes by hand so a field-order drift between the writer
// and reader lanes fails here instead of silently.
fn mp_int(b: &mut Vec<u8>, v: i32) {
    if (0..128).contains(&v) {
        b.push(v as u8);
    } else if (-32..0).contains(&v) {
        b.push((v as i8) as u8);
    } else {
        b.push(0xd2);
        b.extend_from_slice(&v.to_be_bytes());
    }
}

fn mp_u8(b: &mut Vec<u8>, v: u8) {
    if v < 128 {
        b.push(v);
    } else {
        b.push(0xcc);
        b.push(v);
    }
}

fn mp_u16(b: &mut Vec<u8>, v: u16) {
    b.push(0xcd);
    b.extend_from_slice(&v.to_be_bytes());
}

fn mp_i64(b: &mut Vec<u8>, v: i64) {
    b.push(0xd3);
    b.extend_from_slice(&v.to_be_bytes());
}

fn mp_f32(b: &mut Vec<u8>, v: f32) {
    b.push(0xca);
    b.extend_from_slice(&v.to_be_bytes());
}

fn mp_str(b: &mut Vec<u8>, s: &str) {
    if s.len() < 32 {
        b.push(0xa0 | s.len() as u8);
    } else if s.len() < 256 {
        b.push(0xd9);
        b.push(s.len() as u8);
    } else {
        b.push(0xda);
        b.extend_from_slice(&(s.len() as u16).to_be_bytes());
    }
    b.extend_from_slice(s.as_bytes());
}

fn mp_bin(b: &mut Vec<u8>, data: &[u8]) {
    if data.len() < 256 {
        b.push(0xc4);
        b.push(data.len() as u8);
    } else {
        b.push(0xc5);
        b.extend_from_slice(&(data.len() as u16).to_be_bytes());
    }
    b.extend_from_slice(data);
}

fn mp_array(b: &mut Vec<u8>, n: usize) {
    if n < 16 {
        b.push(0x90 | n as u8);
    } else {
        b.push(0xdc);
        b.extend_from_slice(&(n as u16).to_be_bytes());
    }
}

#[test]
fn msgpack_wire_order_interop() {
    let png = png_1x1(9, 9, 9);
    let mut b = Vec::new();
    mp_int(&mut b, 1); // version
    mp_i64(&mut b, 1_700_000_000_000); // date
    for v in [1.0f32, 0.0, 5.0, 0.0, 1.0, 7.0, 0.0, 0.0, 1.0] {
        mp_f32(&mut b, v); // canvas
    }
    mp_array(&mut b, 1); // images
    mp_str(&mut b, &"ab".repeat(32)); // sha
    mp_bin(&mut b, &png); // bitmap
    mp_array(&mut b, 1); // entries
    for v in [0, 0, 1, 1] {
        mp_int(&mut b, v); // bounds
    }
    mp_f32(&mut b, 10.5);
    mp_f32(&mut b, -3.25); // pos
    mp_f32(&mut b, 30.0); // rotation
    mp_f32(&mut b, 2.0);
    mp_f32(&mut b, 3.0); // scale
    for v in [0.0f32, 0.0, 1.0, 1.0] {
        mp_f32(&mut b, v); // crop
    }
    b.push(0xc3); // flipH true
    b.push(0xc2); // flipV false
    mp_array(&mut b, 1); // texts
    mp_u16(&mut b, 0); // image_index
    mp_str(&mut b, "hello"); // text
    mp_f32(&mut b, 24.0); // size
    for v in [10u8, 20, 30] {
        mp_u8(&mut b, v); // color
    }
    mp_u8(&mut b, 1); // has_bg
    for v in [1u8, 2, 3] {
        mp_u8(&mut b, v); // bg
    }
    mp_u8(&mut b, 1); // bold

    let d = serializer::decode(&b).expect("decode hand-built wire bytes");
    assert_eq!(d.version, serializer::SERIAL_VERSION);
    assert_eq!(d.date, 1_700_000_000_000);
    assert_eq!(d.canvas, [1.0, 0.0, 5.0, 0.0, 1.0, 7.0, 0.0, 0.0, 1.0]);
    assert_eq!(d.images.len(), 1);
    assert_eq!(d.images[0].sha, "ab".repeat(32));
    assert_eq!(d.images[0].bitmap, png);
    assert_eq!(d.images[0].instances.len(), 1);
    let e = &d.images[0].instances[0];
    assert_eq!(e.bounds, (0, 0, 1, 1));
    assert_eq!(e.pos, (10.5, -3.25));
    assert_eq!(e.rotation, 30.0);
    assert_eq!(e.scale, (2.0, 3.0));
    assert_eq!(e.crop, (0.0, 0.0, 1.0, 1.0));
    assert!(e.flip_h);
    assert!(!e.flip_v);
    assert_eq!(d.texts.len(), 1);
    let t = &d.texts[0];
    assert_eq!(t.image_index, 0);
    assert_eq!(t.text, "hello");
    assert_eq!(t.size_px, 24.0);
    assert_eq!(t.color, (10, 20, 30));
    assert_eq!(t.background, Some((1, 2, 3)));
    assert!(t.bold);
}

#[test]
fn add_image_decode_fit_add_select() {
    // Add-image flow: decode bytes, fit to settings, add to board, selected.
    let png = png_1x1(200, 100, 50);
    let (w, h) = decode_image(&png).expect("decodable");
    assert_eq!((w, h), (1, 1));

    let settings = Settings::defaults();
    assert!(settings.downscale_on_import);
    let raw = image::load_from_memory(&png).expect("load").to_rgba8().into_raw();
    assert_eq!(raw.len(), 4);
    let (fit, fw, fh) =
        lazyref_board::settings::fit_image(&raw, w, h, settings.max_resolution, settings.multistep_scale);
    assert_eq!((fw, fh), (1, 1));
    assert_eq!(fit.len(), 4);

    let mut s = Sticker::new(StickerKind::Image {
        data: png,
        width: fw,
        height: fh,
    });
    s.width = fw as f32;
    s.height = fh as f32;
    let mut board = Board::new();
    let idx = board.add_sticker(s);
    assert_eq!(board.handling, Some(idx));
    assert_eq!(board.stickers().len(), 1);
}

#[test]
fn contract_consts_pinned() {
    assert_eq!(serializer::SERIAL_VERSION, 1);
    assert_eq!(scene::RECT_PATH_TYPES, [0, 1, 1, 1, 1]);
    assert_eq!(lazyref_board::icons::ICON_TOUCH_RADIUS, 48.0);
}
