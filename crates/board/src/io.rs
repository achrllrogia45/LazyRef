//! Board open/save: `.ref` (native board file) and `.pur` (scene interchange).
//!
//! `.ref` = the [`crate::serializer`] codec over board state, bitmaps deduped
//! by SHA-256. `.pur` open maps scene rows to stickers (visible images in
//! z-order, notes become text stickers); save builds a fresh scene, splitting
//! each sticker's final matrix back into the two stored transforms. Groups,
//! drawings and child items have no board counterpart: skipped on open, not
//! recreated by a fresh save.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

use crate::board::Board;
use crate::math::{sha256_hex, Mat3};
use crate::scene::{
    affine_mul, compose_from_sticker, decompose_to_sticker, qt9_to_w, rect_path5, w_to_qt9,
    RECT_PATH_TYPES,
};
use crate::serializer::{self, RefEntry, RefImage, RefText, SERIAL_VERSION};
use crate::sticker::{decode_image, Sticker, StickerKind};

use purcore::blobs::{big_rational_of, PathElement, QPainterPath, QTransform};
use purcore::container;
use purcore::legacy;
use purcore::model::{Image, Item, ItemImage, Metadata, Scene};
use purcore::purfile::PurFile;
use purcore::scene_db::APPLICATION_VERSION;

/// Identity affine in W 6-tuple layout.
const IDENTITY_W: [f64; 6] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FileFormat {
    Ref,
    Pur,
}

/// Sniff `.pur` (v2 container or legacy v1.x) vs `.ref`. Never panics:
/// anything that is not recognizably a container header is a `.ref` candidate
/// and fails later in `decode` with a proper error when malformed.
pub fn detect(bytes: &[u8]) -> FileFormat {
    if is_v2(bytes) || is_legacy(bytes) {
        FileFormat::Pur
    } else {
        FileFormat::Ref
    }
}

/// v2 container header starts with QString "2.x": u32 length 6 + "2.X" UTF-16BE.
fn is_v2(b: &[u8]) -> bool {
    b.len() >= 10
        && b[0..4] == [0, 0, 0, 6]
        && b[4] == 0
        && b[6] == 0
        && b[8] == 0
        && b[5].is_ascii_digit()
        && b[5] >= b'2'
        && b[7] == b'.'
}

/// Legacy v1.x header starts with u32 8 + "1.10" UTF-16BE.
fn is_legacy(b: &[u8]) -> bool {
    b.len() >= 12 && b[0..4] == [0, 0, 0, 8] && b[4..12] == [0, b'1', 0, b'.', 0, b'1', 0, b'0']
}

pub fn open_ref(bytes: &[u8]) -> Result<Board, String> {
    let data = serializer::decode(bytes)?;
    let mut board = Board::new();
    board.set_canvas_matrix(Mat3(data.canvas));
    for (idx, img) in data.images.iter().enumerate() {
        let texts: Vec<&RefText> = data.texts.iter().filter(|t| t.image_index == idx).collect();
        if texts.is_empty() {
            for inst in &img.instances {
                board.add_sticker(image_sticker(&img.bitmap, inst));
            }
        } else {
            for (j, inst) in img.instances.iter().enumerate() {
                let t = texts.get(j).copied().unwrap_or(texts[0]);
                board.add_sticker(text_sticker(t, inst));
            }
        }
    }
    Ok(board)
}

fn image_sticker(bitmap: &[u8], inst: &RefEntry) -> Sticker {
    let (bw, bh) = (inst.bounds.2 - inst.bounds.0, inst.bounds.3 - inst.bounds.1);
    let mut s = Sticker::new(StickerKind::Image {
        data: bitmap.to_vec(),
        width: bw.max(0) as u32,
        height: bh.max(0) as u32,
    });
    // Decoded dims win when the bytes decode (normal case: bounds == dims).
    if let Some((w, h)) = decode_image(bitmap) {
        s.width = w as f32;
        s.height = h as f32;
    } else {
        s.width = bw as f32;
        s.height = bh as f32;
    }
    apply_entry(&mut s, inst);
    s
}

fn text_sticker(t: &RefText, inst: &RefEntry) -> Sticker {
    let mut s = Sticker::new(StickerKind::Text {
        text: t.text.clone(),
        size_px: t.size_px,
        color: t.color,
        background: t.background,
        bold: t.bold,
    });
    // The shell measures the real text box; the saved bounds restore it exactly.
    s.width = (inst.bounds.2 - inst.bounds.0) as f32;
    s.height = (inst.bounds.3 - inst.bounds.1) as f32;
    apply_entry(&mut s, inst);
    s
}

fn apply_entry(s: &mut Sticker, inst: &RefEntry) {
    s.transform.pos = inst.pos;
    s.transform.rotation = inst.rotation;
    s.transform.scale = inst.scale;
    s.crop = inst.crop;
}

pub fn save_ref(board: &Board) -> Vec<u8> {
    let date = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let mut images: Vec<RefImage> = Vec::new();
    let mut texts: Vec<RefText> = Vec::new();
    let mut index_by_sha: HashMap<String, usize> = HashMap::new();
    for s in board.stickers() {
        let (bw, bh) = (s.width as i32, s.height as i32);
        let entry = RefEntry {
            bounds: (0, 0, bw.max(0), bh.max(0)),
            pos: s.transform.pos,
            rotation: s.transform.rotation,
            scale: s.transform.scale,
            crop: s.crop,
            flip_h: s.transform.scale.0 < 0.0,
            flip_v: s.transform.scale.1 < 0.0,
        };
        match &s.kind {
            StickerKind::Image { data, .. } => {
                let sha = sha256_hex(data);
                match index_by_sha.get(&sha) {
                    Some(&i) => images[i].instances.push(entry),
                    None => {
                        index_by_sha.insert(sha.clone(), images.len());
                        images.push(RefImage {
                            sha,
                            bitmap: data.clone(),
                            instances: vec![entry],
                        });
                    }
                }
            }
            StickerKind::Text {
                text,
                size_px,
                color,
                background,
                bold,
            } => {
                // Text stickers ride in their own image slot with an empty
                // bitmap (the shell re-renders text at load); the sidecar
                // carries the editable fields.
                let sha = format!("text-{}-{}", images.len(), &sha256_hex(text.as_bytes())[..16]);
                let image_index = images.len();
                images.push(RefImage {
                    sha,
                    bitmap: Vec::new(),
                    instances: vec![entry],
                });
                texts.push(RefText {
                    image_index,
                    text: text.clone(),
                    size_px: *size_px,
                    color: *color,
                    background: *background,
                    bold: *bold,
                });
            }
        }
    }
    serializer::encode(&serializer::RefBoardData {
        version: SERIAL_VERSION,
        date,
        canvas: board.canvas_matrix().0,
        images,
        texts,
    })
}

pub fn open_pur(bytes: &[u8], scratch: &Path) -> Result<Board, String> {
    if is_v2(bytes) {
        open_pur_v2(bytes, scratch)
    } else if is_legacy(bytes) {
        open_pur_legacy(bytes)
    } else {
        Err("not a .pur file (use open_ref for .ref boards)".to_string())
    }
}

fn open_pur_v2(bytes: &[u8], scratch: &Path) -> Result<Board, String> {
    if bytes.len() < 108 {
        return Err(format!("file too small for a v2 container: {} B", bytes.len()));
    }
    let ok = std::panic::catch_unwind(|| container::verify_checksum(bytes))
        .map_err(|_| "malformed .pur container header".to_string())?;
    if !ok {
        return Err("container checksum mismatch (md5(file[104:]))".to_string());
    }
    let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        PurFile::new(scratch).read_bytes(bytes, false)
    }))
    .map_err(|_| "cannot parse embedded scene database".to_string())?;
    Ok(board_from_scene(&scene))
}

fn open_pur_legacy(bytes: &[u8]) -> Result<Board, String> {
    if bytes.len() < 224 {
        return Err(format!("file too small for a legacy .pur header: {} B", bytes.len()));
    }
    let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        legacy::read_bytes(bytes, false)
    }))
    .map_err(|_| "malformed legacy .pur".to_string())?;
    if !scene.checksum_matches() {
        return Err("legacy checksum mismatch (md5(file[108:]))".to_string());
    }
    Ok(board_from_legacy(&scene))
}

/// Visible image items, root-level, in (z, sort_order) order (last = topmost).
fn board_from_scene(scene: &Scene) -> Board {
    let mut board = Board::new();
    let images: HashMap<i64, &Image> = scene.images.iter().map(|im| (im.id, im)).collect();
    let image_rows: HashMap<i64, &ItemImage> =
        scene.items_images.iter().map(|ii| (ii.id, ii)).collect();
    let items_by_id: HashMap<i64, &Item> = scene.items.iter().map(|it| (it.id, it)).collect();
    let mut visible: Vec<&Item> = scene
        .items
        .iter()
        .filter(|it| it.parent == -1 && image_rows.contains_key(&it.id))
        .collect();
    visible.sort_by(|a, b| {
        z_key(a)
            .partial_cmp(&z_key(b))
            .unwrap_or(Ordering::Equal)
    });
    for item in visible {
        let ii = image_rows[&item.id];
        let data = match ii.image.and_then(|id| images.get(&id)).and_then(|im| {
            if im.source_type == Some(1) {
                im.data.as_ref()
            } else {
                None
            }
        }) {
            Some(d) => d,
            None => continue, // linked image, movie, or dangling ref: row survives resave, not shown
        };
        if ii.playback_state.unwrap_or(0) != 0 {
            continue;
        }
        let (w, h) = match decode_image(data) {
            Some(d) => d,
            None => continue,
        };
        // M = T_item after T_img is the pixel->scene affine; decompose at the center.
        let item_w = item
            .transform
            .as_ref()
            .map(|t| qt9_to_w(&t.values()))
            .unwrap_or(IDENTITY_W);
        let img_w = ii
            .image_transform
            .as_ref()
            .map(|t| qt9_to_w(&t.values()))
            .unwrap_or(IDENTITY_W);
        let m = affine_mul(&item_w, &img_w);
        let c = (w as f64 / 2.0, h as f64 / 2.0);
        let d = decompose_to_sticker(&m, c);
        let mut s = Sticker::new(StickerKind::Image {
            data: data.clone(),
            width: w,
            height: h,
        });
        s.transform.pos = (d.pos_x as f32, d.pos_y as f32);
        s.transform.rotation = d.rotation_deg as f32;
        s.transform.scale = (d.scale_x as f32, d.scale_y as f32);
        s.set_crop(bounds_from_path(ii.image_bounds.as_ref(), c, w as f64, h as f64));
        board.add_sticker(s);
    }
    let mut notes: Vec<&purcore::model::Note> = scene.notes.iter().collect();
    notes.sort_by_key(|n| n.id);
    for note in notes {
        match items_by_id.get(&note.id) {
            Some(item) if item.parent != -1 => continue,
            _ => {}
        }
        let w = items_by_id
            .get(&note.id)
            .and_then(|it| it.transform.as_ref())
            .map(|t| qt9_to_w(&t.values()))
            .unwrap_or(IDENTITY_W);
        let d = decompose_to_sticker(&w, (0.0, 0.0));
        let mut s = Sticker::new(StickerKind::Text {
            text: note.text.clone().unwrap_or_default(),
            size_px: 24.0,
            color: note.text_color.as_deref().and_then(hex_to_color).unwrap_or((0, 0, 0)),
            background: note.background_color.as_deref().and_then(hex_to_color),
            bold: false,
        });
        s.transform.pos = (d.pos_x as f32, d.pos_y as f32);
        s.transform.rotation = d.rotation_deg as f32;
        s.transform.scale = (d.scale_x as f32, d.scale_y as f32);
        board.add_sticker(s);
    }
    board
}

fn z_key(item: &Item) -> (f64, f64) {
    (
        item.z.unwrap_or(0.0),
        item.sort_order.as_ref().map(|s| s.value()).unwrap_or(0.0),
    )
}

/// `image_bounds` path -> crop rect in pixels. The path lives in the centered
/// frame, so shift by the center; empty path = full image.
fn bounds_from_path(
    path: Option<&QPainterPath>,
    c: (f64, f64),
    w: f64,
    h: f64,
) -> (f32, f32, f32, f32) {
    let els = path.map(|p| p.elements.as_slice()).unwrap_or(&[]);
    if els.is_empty() {
        return (0.0, 0.0, w as f32, h as f32);
    }
    let (mut l, mut t, mut r, mut b) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for e in els {
        l = l.min(e.x);
        t = t.min(e.y);
        r = r.max(e.x);
        b = b.max(e.y);
    }
    ((l + c.0) as f32, (t + c.1) as f32, (r + c.0) as f32, (b + c.1) as f32)
}

/// Legacy scene -> board. Each transform on an image becomes a sticker; the
/// legacy (x, y) is the scene position of the image center, so the pixel->scene
/// affine is W(p) = M*p + ((x,y) - M*c). Crop points live in the centered
/// frame, like v2 bounds. Rotation direction for rotated legacy files assumes
/// the flat [m0,m1,m2,m3] list pairs row-major (x' = m0*x + m1*y); the corpus
/// fixture is axis-aligned so either pairing reads it identically.
fn board_from_legacy(scene: &legacy::LegacyScene) -> Board {
    let mut board = Board::new();
    for img in &scene.images {
        if img.png.len() == 4 {
            continue; // link placeholder (FF FF FF FF): no bytes to show
        }
        let (w, h) = match decode_image(&img.png) {
            Some(d) => d,
            None => continue,
        };
        let mut ts = img.transforms.clone();
        ts.sort_by(|a, b| {
            a.z_layer
                .partial_cmp(&b.z_layer)
                .unwrap_or(Ordering::Equal)
        });
        for t in &ts {
            let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
            let m = t.matrix;
            let w6 = [
                m[0],
                m[1],
                t.x - (m[0] * cx + m[1] * cy),
                m[2],
                m[3],
                t.y - (m[2] * cx + m[3] * cy),
            ];
            let d = decompose_to_sticker(&w6, (cx, cy));
            let mut s = Sticker::new(StickerKind::Image {
                data: img.png.clone(),
                width: w,
                height: h,
            });
            s.transform.pos = (d.pos_x as f32, d.pos_y as f32);
            s.transform.rotation = d.rotation_deg as f32;
            s.transform.scale = (d.scale_x as f32, d.scale_y as f32);
            if t.points_x.is_empty() {
                s.set_crop((0.0, 0.0, w as f32, h as f32));
            } else {
                let l = t.points_x.iter().fold(f64::INFINITY, |a, &v| a.min(v)) + cx;
                let r = t.points_x.iter().fold(f64::NEG_INFINITY, |a, &v| a.max(v)) + cx;
                let tp = t.points_y.iter().fold(f64::INFINITY, |a, &v| a.min(v)) + cy;
                let b = t.points_y.iter().fold(f64::NEG_INFINITY, |a, &v| a.max(v)) + cy;
                s.set_crop((l as f32, tp as f32, r as f32, b as f32));
            }
            board.add_sticker(s);
        }
    }
    for text in &scene.texts {
        push_legacy_text(&mut board, text);
    }
    board
}

/// Legacy text items (top-level and children): position/scale from the matrix,
/// 16-bit colors downshifted to 8-bit. Size is not stored; the shell measures.
fn push_legacy_text(board: &mut Board, text: &legacy::LegacyTextItem) {
    let m = text.matrix;
    let w6 = [m[0], m[1], text.x, m[2], m[3], text.y];
    let d = decompose_to_sticker(&w6, (0.0, 0.0));
    let mut s = Sticker::new(StickerKind::Text {
        text: text.text.clone(),
        size_px: 16.0,
        color: (
            (text.rgb[0] >> 8) as u8,
            (text.rgb[1] >> 8) as u8,
            (text.rgb[2] >> 8) as u8,
        ),
        background: if text.opacity_background > 0 {
            Some((
                (text.rgb_background[0] >> 8) as u8,
                (text.rgb_background[1] >> 8) as u8,
                (text.rgb_background[2] >> 8) as u8,
            ))
        } else {
            None
        },
        bold: false,
    });
    s.transform.pos = (d.pos_x as f32, d.pos_y as f32);
    s.transform.rotation = d.rotation_deg as f32;
    s.transform.scale = (d.scale_x as f32, d.scale_y as f32);
    board.add_sticker(s);
    for child in &text.text_children {
        push_legacy_text(board, child);
    }
}

/// Write the board as a fresh `.pur` scene (new ids, fresh z/sort slots).
/// Returns the file bytes; atomic temp+rename at the target path is the shell's job.
pub fn save_pur(board: &Board, scratch: &Path, thumbnail: &[u8]) -> Vec<u8> {
    let mut scene = Scene::default();
    let mut next_image_id = 0i64;
    let mut next_item_id = 0i64;
    let mut image_id_by_md5: HashMap<String, i64> = HashMap::new();
    for (i, s) in board.stickers().iter().enumerate() {
        let id = next_item_id;
        next_item_id += 1;
        let z = (i + 1) as f64;
        let sort_order = big_rational_of((i + 1) as i64, 1);
        match &s.kind {
            StickerKind::Image { data, .. } => {
                let md5 = purcore::checksum::md5_hex_from(data, 0);
                let image_id = match image_id_by_md5.get(&md5) {
                    Some(&id) => id,
                    None => {
                        let image_id = next_image_id;
                        next_image_id += 1;
                        let (dw, dh) =
                            decode_image(data).unwrap_or((s.width as u32, s.height as u32));
                        scene.images.push(Image {
                            id: image_id,
                            width: Some(dw as i32),
                            height: Some(dh as i32),
                            checksum: Some(md5.clone()),
                            format: Some(sniff_format(data).to_string()),
                            source: None,
                            source_type: Some(1),
                            origin: None,
                            data: Some(data.clone()),
                        });
                        image_id_by_md5.insert(md5, image_id);
                        image_id
                    }
                };
                // W from the final matrix, split into the two stored transforms.
                let m = s.final_matrix().0;
                let w6 = [
                    m[0] as f64,
                    m[1] as f64,
                    m[2] as f64,
                    m[3] as f64,
                    m[4] as f64,
                    m[5] as f64,
                ];
                let (cl, ct, cr, cb) = s.crop;
                let c = (
                    ((cl + cr) / 2.0) as f64,
                    ((ct + cb) / 2.0) as f64,
                );
                let comp = compose_from_sticker(
                    [w6[0], w6[1], w6[3], w6[4]],
                    (w6[2], w6[5]),
                    c,
                );
                scene.items.push(Item {
                    id,
                    name: None,
                    transform: Some(qt_from_w6(&comp.item_transform)),
                    sort_order: Some(sort_order),
                    z: Some(z),
                    opacity: Some(1.0),
                    parent: -1,
                    locked: Some(0),
                    comment: None,
                });
                scene.items_images.push(ItemImage {
                    id,
                    image: Some(image_id),
                    image_transform: Some(qt_from_w6(&comp.image_transform)),
                    image_bounds: Some(path_from_crop(
                        cl as f64, ct as f64, cr as f64, cb as f64, c.0, c.1,
                    )),
                    flags: Some(1),
                    playback_state: Some(0),
                    playback_frame: Some(0),
                    playback_speed: Some(1.0),
                });
            }
            StickerKind::Text {
                text,
                size_px: _,
                color,
                background,
                bold: _,
            } => {
                let m = s.final_matrix().0;
                let w6 = [
                    m[0] as f64,
                    m[1] as f64,
                    m[2] as f64,
                    m[3] as f64,
                    m[4] as f64,
                    m[5] as f64,
                ];
                scene.items.push(Item {
                    id,
                    name: None,
                    transform: Some(qt_from_w6(&w6)),
                    sort_order: Some(sort_order),
                    z: Some(z),
                    opacity: Some(1.0),
                    parent: -1,
                    locked: Some(0),
                    comment: None,
                });
                scene.notes.push(purcore::model::Note {
                    id,
                    text: Some(text.clone()),
                    background_color: background.map(color_to_hex),
                    text_color: Some(color_to_hex(*color)),
                    style: Some(0),
                    fixed_size: None,
                });
            }
        }
    }
    scene.metadata = Some(Metadata {
        id: 0,
        thumbnail: if thumbnail.is_empty() {
            None
        } else {
            Some(thumbnail.to_vec())
        },
        saved: Some(1),
        last_load_checksum: None,
        last_load_path: None,
        last_save_path: None,
        vertical_scroll: Some(0),
        scene_rect: None,
        view_transform: None,
        application_version: Some(APPLICATION_VERSION.to_string()),
        horizontal_scroll: Some(0),
    });
    PurFile::new(scratch).write_bytes(&scene, thumbnail)
}

fn qt_from_w6(w: &[f64; 6]) -> QTransform {
    let q = w_to_qt9(w);
    QTransform {
        m11: q[0],
        m12: q[1],
        m13: q[2],
        m21: q[3],
        m22: q[4],
        m23: q[5],
        m31: q[6],
        m32: q[7],
        m33: q[8],
    }
}

fn path_from_crop(cl: f64, ct: f64, cr: f64, cb: f64, cx: f64, cy: f64) -> QPainterPath {
    let pts = rect_path5(cl, ct, cr, cb, cx, cy);
    QPainterPath {
        elements: pts
            .chunks_exact(2)
            .enumerate()
            .map(|(k, p)| PathElement {
                r#type: RECT_PATH_TYPES[k],
                x: p[0],
                y: p[1],
            })
            .collect(),
        c_start: 0,
        fill_rule: 0,
    }
}

/// PNG magic -> "PNG", JPEG SOI -> "JPEG" (Qt uppercases its format names).
fn sniff_format(data: &[u8]) -> &'static str {
    if data.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        "PNG"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "JPEG"
    } else {
        "PNG"
    }
}

fn color_to_hex(c: (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", c.0, c.1, c.2)
}

fn hex_to_color(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    let (r, g, b) = match s.len() {
        6 => (
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
        ),
        3 => {
            let n = |i: usize| u8::from_str_radix(&s[i..i + 1], 16).ok().map(|v| v * 17);
            (n(0)?, n(1)?, n(2)?)
        }
        _ => return None,
    };
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    fn png_1x1(r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(&[r, g, b, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        buf
    }

    fn s1_bytes() -> Vec<u8> {
        std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../purcore/tests/fixtures/S1_redbar-new.pur"),
        )
        .expect("S1 fixture")
    }

    fn legacy_bytes() -> Vec<u8> {
        std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../purcore/tests/fixtures/legacy/S1_redbar.pur"),
        )
        .expect("legacy fixture")
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lazyref-board-io-{}-{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_sniffs_all_three() {
        assert_eq!(detect(&s1_bytes()), FileFormat::Pur);
        assert_eq!(detect(&legacy_bytes()), FileFormat::Pur);
        let reffy = save_ref(&Board::new());
        assert_eq!(detect(&reffy), FileFormat::Ref);
        assert_eq!(detect(&[]), FileFormat::Ref);
        assert_eq!(detect(b"junk"), FileFormat::Ref);
    }

    #[test]
    fn ref_roundtrip_image_and_text() {
        let mut a = Sticker::new(StickerKind::Image {
            data: png_1x1(1, 2, 3),
            width: 1,
            height: 1,
        });
        a.transform.pos = (10.0, 20.0);
        a.transform.rotation = 30.0;
        a.transform.scale = (-2.0, 3.0);
        let mut t = Sticker::new(StickerKind::Text {
            text: "hi".to_string(),
            size_px: 24.0,
            color: (1, 2, 3),
            background: None,
            bold: true,
        });
        t.width = 50.0;
        t.height = 12.0;
        let mut board = Board::new();
        board.add_sticker(a);
        board.add_sticker(t);
        let back = open_ref(&save_ref(&board)).expect("open");
        assert_eq!(back.stickers().len(), 2);
        let ra = &back.stickers()[0];
        assert_eq!(ra.transform.pos, (10.0, 20.0));
        assert_eq!(ra.transform.scale, (-2.0, 3.0));
        match &back.stickers()[1].kind {
            StickerKind::Text { text, bold, .. } => {
                assert_eq!(text, "hi");
                assert!(*bold);
            }
            StickerKind::Image { .. } => panic!("expected text"),
        }
    }

    #[test]
    fn pur_open_s1_has_scaled_png_sticker() {
        let board = open_pur(&s1_bytes(), &scratch("open")).expect("open S1");
        assert_eq!(board.stickers().len(), 1);
        match &board.stickers()[0].kind {
            StickerKind::Image { data, .. } => {
                assert_eq!(&data[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
            }
            _ => panic!("expected image"),
        }
        let s = &board.stickers()[0];
        for v in [s.transform.scale.0.abs(), s.transform.scale.1.abs()] {
            assert!((v - 15.625).abs() < 1e-3, "scale ~15.625, got {v}");
        }
    }

    #[test]
    fn pur_save_reopen_preserves_images() {
        let dir = scratch("resave");
        let board = open_pur(&s1_bytes(), &dir).expect("open");
        let out = save_pur(&board, &dir, &[]);
        let back = open_pur(&out, &dir).expect("reopen");
        assert_eq!(back.stickers().len(), board.stickers().len());
        let bytes = |b: &Board| {
            b.stickers()
                .iter()
                .filter_map(|s| match &s.kind {
                    StickerKind::Image { data, .. } => Some(data.len()),
                    _ => None,
                })
                .sum::<usize>()
        };
        assert_eq!(bytes(&back), bytes(&board));
    }

    #[test]
    fn legacy_open_s1_has_sticker() {
        let board = open_pur(&legacy_bytes(), &scratch("legacy")).expect("open legacy");
        assert_eq!(board.stickers().len(), 1);
        let s = &board.stickers()[0];
        for v in [s.transform.scale.0.abs(), s.transform.scale.1.abs()] {
            assert!((v - 15.625).abs() < 1e-3, "scale ~15.625, got {v}");
        }
    }

    #[test]
    fn pur_checksum_mismatch_is_err() {
        let mut bad = s1_bytes();
        bad[200] ^= 0xFF; // inside the thumbnail: breaks the checksum only
        assert!(open_pur(&bad, &scratch("bad")).is_err());
        assert!(open_ref(b"junk").is_err());
    }
}
