//! Legacy v1.x `.pur` reader — the pre-2.0 binary format ("1.10", 224-byte header).
//!
//! Read-only by design (there is no legacy writer). Port of the reference reader
//! `PureRef-format/purformat/read.py`, field-for-field cross-checked in tests against the
//! dump in `tests/fixtures/legacy-dumps/S1_redbar.tsv`.
//!
//! ## Mapping decision: dedicated model, not [`crate::model::Scene`]
//! The legacy format predates the SQLite container: no groups/notes/drawings/metadata
//! tables, and transforms nest under images with crop-point lists and text children.
//! This reader returns [`LegacyScene`]; the app layer maps it to the board directly.
//!
//! ## Deliberate deviations from the Python reference (malformed input only)
//! On structurally broken files (missing PNG footer, impossible lengths) this panics where
//! Python would misparse silently or loop; well-formed files parse identically. Checksum
//! verification mirrors the container path: `stored == md5(file[108:])` hex UTF-16BE at
//! `[44:108]`.

use std::fs;
use std::path::Path;

use crate::be;
use crate::checksum::md5_hex_from;

/// PNG signature and IEND chunk (the reader locates image bytes by magic, as the toolkit does).
const PNG_HEAD: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const PNG_FOOT: [u8; 12] = [0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];

const TAG_IMAGE_ITEM: u32 = 34; // utf-16 byte length of "GraphicsImageItem"
const TAG_TEXT_ITEM: u32 = 32; // and of "GraphicsTextItem"
const HEADER_LENGTH: usize = 224;

/// Reads `path`. Verifies the legacy checksum by default (the desktop app rejects mismatches).
pub fn read(path: &Path, verify_checksum: bool) -> LegacyScene {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    read_bytes(&bytes, verify_checksum)
}

pub fn read_bytes(bytes: &[u8], verify_checksum: bool) -> LegacyScene {
    assert!(
        bytes.len() >= HEADER_LENGTH,
        "file too small for a legacy .pur header: {} B",
        bytes.len()
    );
    let stored = be::utf16_be_to_string(&bytes[44..108]);
    let computed = md5_hex_from(bytes, 108);
    if verify_checksum {
        assert!(
            stored == computed,
            "legacy checksum mismatch (stored={stored}, md5(file[108:])={computed})"
        );
    }
    Parser { b: bytes, pos: 0 }.parse(stored, computed)
}

struct Parser<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn parse(mut self, stored_checksum: String, computed_checksum: String) -> LegacyScene {
        let canvas = [
            be::f64(self.b, 112),
            be::f64(self.b, 120),
            be::f64(self.b, 128),
            be::f64(self.b, 136),
        ];
        let zoom = be::f64(self.b, 144);
        let x_canvas = be::i32(self.b, 216);
        let y_canvas = be::i32(self.b, 220);
        self.pos = HEADER_LENGTH;

        let mut images = self.read_images();
        let mut image_items: Vec<LegacyImageItem> = Vec::new();
        let mut texts: Vec<LegacyTextItem> = Vec::new();
        loop {
            // item area: tag lives at offset 8 (after the u64 end address)
            self.need(self.pos + 12);
            let tag = be::u32(self.b, self.pos + 8);
            if tag == TAG_IMAGE_ITEM {
                image_items.push(self.read_image_item());
            } else if tag == TAG_TEXT_ITEM {
                texts.push(self.read_text_item());
            } else {
                break;
            }
        }

        // Location string, then one (id, begin, end) reference row per image transform.
        let folder_location = self.read_string();
        for _ in 0..image_items.len() {
            self.need(self.pos + 20);
            let red_id = be::u32(self.b, self.pos);
            let ref_begin = be::u64(self.b, self.pos + 4);
            self.pos += 20;
            for item in &image_items {
                if red_id == item.id {
                    for image in images.iter_mut() {
                        if ref_begin == image.address_start {
                            image.transforms = vec![item.clone()];
                        }
                    }
                }
            }
        }

        // Duplicate entries carry the *parent* transform id in 4 bytes; merge their
        // transforms into that original image, then drop them. Links (0xFFFFFFFF) stay.
        for i in 0..images.len() {
            if is_duplicate(&images[i]) {
                let parent_id = be::u32(&images[i].png, 0);
                for j in 0..images.len() {
                    if !images[j].transforms.is_empty() && parent_id == images[j].transforms[0].id {
                        let dup = images[i].transforms.clone();
                        images[j].transforms.extend(dup);
                    }
                }
            }
        }
        LegacyScene {
            canvas,
            zoom,
            x_canvas,
            y_canvas,
            folder_location,
            images: images
                .into_iter()
                .filter(|img| !is_duplicate(img))
                .collect(),
            texts,
            stored_checksum,
            computed_checksum,
        }
    }

    /// Image area: PNGs located by magic; every 4 bytes before a PNG (or before the items)
    /// is a duplicate.
    fn read_images(&mut self) -> Vec<LegacyImage> {
        let mut images = Vec::new();
        loop {
            let start = self.index_of(&PNG_HEAD, self.pos);
            if start < 0 {
                break;
            }
            let start = start as usize;
            if start - self.pos >= 4 {
                self.need(self.pos + 4);
                images.push(LegacyImage {
                    address_start: self.pos as u64,
                    address_end: (self.pos + 4) as u64,
                    png: self.b[self.pos..self.pos + 4].to_vec(),
                    transforms: Vec::new(),
                });
                self.pos += 4;
            } else {
                let foot = self.index_of(&PNG_FOOT, self.pos);
                assert!(
                    foot >= 0 && foot as usize >= start,
                    "{}",
                    self.malformed(&format!("PNG footer not found after {start}"))
                );
                let foot = foot as usize;
                // consumed length, relative to pos ([start] is absolute)
                let len = foot + PNG_FOOT.len() - self.pos;
                images.push(LegacyImage {
                    address_start: start as u64,
                    address_end: (self.pos + len) as u64,
                    png: self.b[start..self.pos + len].to_vec(),
                    transforms: Vec::new(),
                });
                self.pos += len;
            }
        }
        loop {
            // trailing duplicates between the last PNG and the first item
            self.need(self.pos + 12);
            let tag = be::u32(self.b, self.pos + 8);
            if tag == TAG_IMAGE_ITEM || tag == TAG_TEXT_ITEM {
                break;
            }
            self.need(self.pos + 4);
            images.push(LegacyImage {
                address_start: self.pos as u64,
                address_end: (self.pos + 4) as u64,
                png: self.b[self.pos..self.pos + 4].to_vec(),
                transforms: Vec::new(),
            });
            self.pos += 4;
        }
        images
    }

    fn read_text_item(&mut self) -> LegacyTextItem {
        let transform_end = self.end_address();
        self.pos += 12 + self.tag_length(); // tag + "GraphicsTextItem" (tag == byte length)
        let text = self.read_string();
        let matrix = self.read_matrix();
        let x = self.read_double();
        let y = self.read_double();
        self.pos += 8; // permanent 1.0 double
        let id = be::u32(self.b, self.pos);
        self.pos += 4;
        let z_layer = self.read_double();
        let hsv_foreground = be::i8(self.b, self.pos) == 2;
        self.pos += 1;
        let opacity = be::u16(self.b, self.pos);
        self.pos += 2;
        let mut rgb = [
            be::u16(self.b, self.pos) as i32,
            be::u16(self.b, self.pos + 2) as i32,
            be::u16(self.b, self.pos + 4) as i32,
        ];
        self.pos += 6;
        if hsv_foreground {
            rgb = hsv_to_rgb(rgb);
        }
        self.pos += 2; // unknown
        let hsv_background = be::i8(self.b, self.pos) == 2;
        self.pos += 1;
        let opacity_background = be::u16(self.b, self.pos);
        self.pos += 2;
        let mut rgb_background = [
            be::u16(self.b, self.pos) as i32,
            be::u16(self.b, self.pos + 2) as i32,
            be::u16(self.b, self.pos + 4) as i32,
        ];
        self.pos += 6;
        if hsv_background {
            rgb_background = hsv_to_rgb(rgb_background);
        }
        let children_count = be::u32(self.b, self.pos + 2); // 2-byte unknown, then u32 count
        self.seek_to(transform_end);
        let text_children = self.read_text_children(children_count);
        LegacyTextItem {
            text,
            matrix,
            x,
            y,
            id,
            z_layer,
            opacity: opacity as i32,
            rgb,
            opacity_background: opacity_background as i32,
            rgb_background,
            text_children,
        }
    }

    fn read_image_item(&mut self) -> LegacyImageItem {
        let transform_end = self.end_address();
        self.pos += 12 + self.tag_length(); // tag + "GraphicsImageItem"
        let mut source = "BruteForceLoaded".to_string(); // kept when the file stores -1 (absent)
        let mut name = "image".to_string();
        let brute_force_loaded = be::u32(self.b, self.pos) == 0;
        if brute_force_loaded {
            self.pos += 4;
        }
        if be::u32(self.b, self.pos) == 0xFFFF_FFFF {
            self.pos += 4;
        } else {
            source = self.read_string();
        }
        if !brute_force_loaded {
            if be::u32(self.b, self.pos) == 0xFFFF_FFFF {
                self.pos += 4;
            } else {
                name = self.read_string();
            }
        }
        self.pos += 8; // permanent 1.0 double
        let matrix = self.read_matrix();
        let x = self.read_double();
        let y = self.read_double();
        self.pos += 8; // second permanent 1.0 double
        let id = be::u32(self.b, self.pos);
        self.pos += 4;
        let z_layer = self.read_double();
        let matrix_before_crop = self.read_matrix();
        let x_crop = self.read_double();
        let y_crop = self.read_double();
        let scale_crop = self.read_double();
        let point_count = be::u32(self.b, self.pos) as usize;
        self.pos += 4;
        assert!(
            point_count <= (self.b.len() - self.pos) / 20,
            "{}",
            self.malformed(&format!("{point_count} crop points won't fit"))
        );
        let mut points_x = Vec::with_capacity(point_count);
        let mut points_y = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            self.pos += 4; // per-point flag
            points_x.push(self.read_double());
            points_y.push(self.read_double());
        }
        let children_count = be::u32(self.b, self.pos + 21); // fixed 25-byte tail, count last
        self.seek_to(transform_end);
        let text_children = self.read_text_children(children_count);
        LegacyImageItem {
            source,
            name,
            matrix,
            x,
            y,
            id,
            z_layer,
            matrix_before_crop,
            x_crop,
            y_crop,
            scale_crop,
            points_x,
            points_y,
            text_children,
        }
    }

    fn read_text_children(&mut self, count: u32) -> Vec<LegacyTextItem> {
        let mut children = Vec::new();
        let mut i = 0u32;
        while i < count {
            // u32 bound: a corrupt count must not wrap
            children.push(self.read_text_item());
            i += 1;
        }
        children
    }

    // ------------------------------------------------------------------ primitives

    fn end_address(&self) -> u64 {
        let end = be::u64(self.b, self.pos);
        assert!(
            end >= self.pos as u64 && end as usize <= self.b.len(),
            "{}",
            self.malformed(&format!("bad item end address {end}"))
        );
        end
    }

    /// Item type tag at offset 8, also the UTF-16 byte length of the "Graphics…Item" string.
    fn tag_length(&self) -> usize {
        let len = be::u32(self.b, self.pos + 8);
        assert!(
            self.pos + 12 + len as usize <= self.b.len(),
            "{}",
            self.malformed(&format!("bad tag length {len} at {}", self.pos))
        );
        len as usize
    }

    fn seek_to(&mut self, end: u64) {
        assert!(
            end >= self.pos as u64,
            "{}",
            self.malformed(&format!(
                "item end {end} before current position {}",
                self.pos
            ))
        );
        self.pos = end as usize;
    }

    fn read_string(&mut self) -> String {
        let len = be::u32(self.b, self.pos) as usize;
        self.pos += 4;
        assert!(
            self.pos + len <= self.b.len(),
            "{}",
            self.malformed(&format!("string length {len} at {}", self.pos))
        );
        let s = be::utf16_be_to_string(&self.b[self.pos..self.pos + len]);
        self.pos += len;
        s
    }

    fn read_double(&mut self) -> f64 {
        let v = be::f64(self.b, self.pos);
        self.pos += 8;
        v
    }

    /// The 6×f64 matrix carriers: m0, m1, (skip 1.0), m2, m3, (skip 1.0).
    fn read_matrix(&mut self) -> [f64; 4] {
        let m = [
            be::f64(self.b, self.pos),
            be::f64(self.b, self.pos + 8),
            be::f64(self.b, self.pos + 24),
            be::f64(self.b, self.pos + 32),
        ];
        self.pos += 48;
        m
    }

    fn index_of(&self, pattern: &[u8], from: usize) -> isize {
        if self.b.len() < pattern.len() {
            return -1;
        }
        'outer: for i in from..=self.b.len() - pattern.len() {
            for j in 0..pattern.len() {
                if self.b[i + j] != pattern[j] {
                    continue 'outer;
                }
            }
            return i as isize;
        }
        -1
    }

    fn need(&self, off: usize) {
        assert!(
            off <= self.b.len(),
            "{}",
            self.malformed(&format!("truncated at {off}"))
        );
    }

    fn malformed(&self, why: &str) -> String {
        format!("malformed legacy .pur: {why}")
    }
}

fn is_duplicate(img: &LegacyImage) -> bool {
    img.png.len() == 4 && !img.png.iter().all(|b| *b == 0xFF)
}

/// `colorsys.hsv_to_rgb(h/35900, s/65535, v/65535)` then `int(x*65535)` — Python's exact
/// arithmetic order, so results match bit-for-bit (verified by the L3-edge fixture).
fn hsv_to_rgb(hsv: [i32; 3]) -> [i32; 3] {
    colorsys_hsv_to_rgb(
        hsv[0] as f64 / 35900.0,
        hsv[1] as f64 / 65535.0,
        hsv[2] as f64 / 65535.0,
    )
    .map(|x| (x * 65535.0) as i32)
}

fn colorsys_hsv_to_rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    if s == 0.0 {
        return [v, v, v];
    }
    let i0 = (h * 6.0) as i32;
    let f = h * 6.0 - i0 as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match ((i0 % 6) + 6) % 6 {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// A parsed legacy v1.x scene. Images carry their duplicate/link entries merged in (each
/// [`LegacyImage`] holds the original plus any duplicates as [`LegacyImageItem`]s); [`texts`]
/// are the top-level text items, each with a `text_children` tree.
///
/// [`texts`]: LegacyScene::texts
#[derive(Debug, Clone)]
pub struct LegacyScene {
    pub canvas: [f64; 4], // 4 doubles: x1, y1, x2, y2
    pub zoom: f64,
    pub x_canvas: i32,
    pub y_canvas: i32,
    pub folder_location: String,
    pub images: Vec<LegacyImage>,
    pub texts: Vec<LegacyTextItem>,
    pub stored_checksum: String,   // header [44:108], UTF-16BE
    pub computed_checksum: String, // md5(file[108:]) hex — what the desktop app enforces
}

impl LegacyScene {
    pub fn checksum_matches(&self) -> bool {
        self.stored_checksum == self.computed_checksum
    }
}

/// One image blob and the transforms that display it. `png` is the PNG file's bytes; for a
/// duplicate entry it is 4 bytes holding the parent transform id; an image link is 4 bytes
/// `FF FF FF FF` and is kept (unlike duplicates, which are merged and dropped).
#[derive(Debug, Clone)]
pub struct LegacyImage {
    pub address_start: u64, // file offsets of the blob
    pub address_end: u64,
    pub png: Vec<u8>,
    pub transforms: Vec<LegacyImageItem>, // assigned by the reference table, merged by dedup
}

/// Image transform: placement, crop rectangle and crop points.
#[derive(Debug, Clone)]
pub struct LegacyImageItem {
    pub source: String,
    pub name: String,
    pub matrix: [f64; 4], // 2x2 (m0, m1, m2, m3) scale/rotation
    pub x: f64,
    pub y: f64,
    pub id: u32, // referenced by the file's trailing ref table
    pub z_layer: f64,
    pub matrix_before_crop: [f64; 4],
    pub x_crop: f64,
    pub y_crop: f64,
    pub scale_crop: f64,
    pub points_x: Vec<f64>, // crop polygon; both lists have the same length
    pub points_y: Vec<f64>,
    pub text_children: Vec<LegacyTextItem>,
}

/// Text item (top-level or child). Colors are 16-bit RGB; HSV-encoded ones are converted.
#[derive(Debug, Clone)]
pub struct LegacyTextItem {
    pub text: String,
    pub matrix: [f64; 4],
    pub x: f64,
    pub y: f64,
    pub id: u32,
    pub z_layer: f64,
    pub opacity: i32, // u16
    pub rgb: [i32; 3],
    pub opacity_background: i32, // u16
    pub rgb_background: [i32; 3],
    pub text_children: Vec<LegacyTextItem>,
}
