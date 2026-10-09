//! pur-core selftest — blob codecs, container, legacy dump diff, SQLite storage classes
//! (the latin-1 TEXT hazard), and the byte-identical `writeBytes` rebuild (reads the S1
//! scene + thumbnail and rewrites the whole file from scratch).
//!
//! Fixtures are module-relative (`tests/fixtures/...`) via `CARGO_MANIFEST_DIR`.

use std::path::PathBuf;

use purcore::blobs::{self, BigRational, PathElement, QPainterPath, QRectF, QTransform};
use purcore::container::{self, Format};
use purcore::legacy;
use purcore::purfile::PurFile;
use rusqlite::Connection;

// -------------------------------------------------------------------- fixtures

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn scratch_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("purcore-selftest-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn hex_to_bytes(h: &str) -> Vec<u8> {
    (0..h.len() / 2)
        .map(|i| u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

struct Fix {
    name: String,
    bytes: Vec<u8>,
}

fn load_fixtures() -> Vec<Fix> {
    std::fs::read_to_string(fixture("blobs.tsv"))
        .expect("fixture TSV not found")
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let (n, h) = line.split_once('\t').unwrap();
            Fix {
                name: n.to_string(),
                bytes: hex_to_bytes(h),
            }
        })
        .collect()
}

// ================================================================ blob codecs
// port of PurCoreSelftest.selftestBlobs

#[test]
fn blob_codecs() {
    let fx = load_fixtures();

    // decode -> encode must be byte-identical for every fixture
    for f in &fx {
        let name = f.name.as_str();
        let rt: Option<Vec<u8>> = if name.ends_with("items_transform")
            || name.ends_with("items_images_transform")
            || name.ends_with("metadata_view_transform")
        {
            Some(blobs::encode_transform(&blobs::decode_transform(&f.bytes)))
        } else if name.ends_with("items_sort_order") {
            Some(blobs::encode_big_rational(&blobs::decode_big_rational(
                &f.bytes,
            )))
        } else if name.ends_with("items_images_bounds") {
            Some(blobs::encode_path(&blobs::decode_path(&f.bytes)))
        } else if name.ends_with("metadata_scene_rect") {
            Some(blobs::encode_rect_f(&blobs::decode_rect_f(&f.bytes)))
        } else {
            None
        };
        let rt = rt.unwrap_or_else(|| panic!("{}: unknown codec", f.name));
        assert_eq!(rt, f.bytes, "{}: roundtrip ({} B)", f.name, f.bytes.len());
        assert_eq!(
            blobs::stored_text_to_bytes(&blobs::bytes_to_stored_text(&f.bytes)),
            f.bytes,
            "{}: latin-1 storage roundtrip",
            f.name
        );
    }

    // known values (corpus extraction)
    let one = |n: &str| fx.iter().find(|f| f.name == n).unwrap().bytes.clone();

    assert_eq!(
        blobs::decode_transform(&one("s1.items_transform")),
        QTransform {
            m11: 15.625,
            m12: 0.0,
            m13: 0.0,
            m21: 0.0,
            m22: 15.625,
            m23: 0.0,
            m31: 500.0,
            m32: 375.0,
            m33: 1.0
        },
        "s1.items_transform = 15.625 scale @ (500,375)"
    );
    assert_eq!(
        blobs::decode_transform(&one("s1.items_images_transform")),
        QTransform {
            m11: 1.0,
            m12: 0.0,
            m13: 0.0,
            m21: 0.0,
            m22: 1.0,
            m23: 0.0,
            m31: -32.0,
            m32: -24.0,
            m33: 1.0
        },
        "s1.items_images_transform = identity @ (-32,-24)"
    );
    assert_eq!(
        blobs::decode_path(&one("s1.items_images_bounds")),
        QPainterPath {
            elements: vec![
                PathElement {
                    r#type: 0,
                    x: -32.0,
                    y: -24.0
                },
                PathElement {
                    r#type: 1,
                    x: 32.0,
                    y: -24.0
                },
                PathElement {
                    r#type: 1,
                    x: 32.0,
                    y: 24.0
                },
                PathElement {
                    r#type: 1,
                    x: -32.0,
                    y: 24.0
                },
                PathElement {
                    r#type: 1,
                    x: -32.0,
                    y: -24.0
                },
            ],
            c_start: 0,
            fill_rule: 0
        },
        "s1.items_images_bounds = 64x48 rect path"
    );
    let s1so = blobs::decode_big_rational(&one("s1.items_sort_order"));
    assert!(
        s1so == BigRational {
            num_sign: 1,
            num_limbs: vec![1],
            den_sign: 1,
            den_limbs: vec![1]
        } && s1so.value() == 1.0,
        "s1 sort_order = 1/1"
    );
    let s2so: Vec<BigRational> = fx
        .iter()
        .filter(|f| f.name == "s2.items_sort_order")
        .map(|f| blobs::decode_big_rational(&f.bytes))
        .collect();
    assert!(
        s2so.len() == 2 && s2so[0].value() == 1.0 && s2so[1].value() == 2.0,
        "s2 sort_orders = 1/1, 2/1"
    );
    assert_eq!(
        blobs::decode_rect_f(&one("s2.metadata_scene_rect")),
        QRectF {
            x: -10000.0,
            y: -10000.0,
            w: 11000.0,
            h: 10333.333333333334
        },
        "s2.metadata_scene_rect known value"
    );
    assert_eq!(
        blobs::decode_transform(&one("s2.metadata_view_transform")),
        QTransform::IDENTITY,
        "s2.metadata_view_transform = identity"
    );

    // envelope details
    assert_eq!(
        &one("s1.items_transform")[0..5],
        &[0u8, 0, 0, 80, 0],
        "QTransform envelope: typeId 80 (raw), is_null 0"
    );
    let sort = one("s1.items_sort_order");
    assert_eq!(
        &sort[0..4],
        &[0u8, 0, 4, 0],
        "BigRational envelope: typeId 1024"
    );
    assert_eq!(
        std::str::from_utf8(&sort[9..20]).unwrap(),
        "BigRational",
        "BigRational envelope: type name"
    );
    let r32 =
        blobs::decode_big_rational(&blobs::encode_big_rational(&blobs::big_rational_of(3, 2)));
    assert!(
        r32 == BigRational {
            num_sign: 1,
            num_limbs: vec![3],
            den_sign: 1,
            den_limbs: vec![2]
        } && r32.value() == 1.5,
        "bigRationalOf(3/2) fresh encode"
    );
    let empty = blobs::encode_path(&QPainterPath {
        elements: vec![],
        c_start: 0,
        fill_rule: 0,
    });
    assert!(
        empty.len() == 26 && blobs::decode_path(&empty).elements.is_empty(),
        "empty QPainterPath = bare u32 0 (26 B)"
    );
}

// ================================================================ container
// port of PurCoreSelftest.selftestContainer

#[test]
fn container_v2() {
    let bytes = std::fs::read(fixture("S1_redbar-new.pur")).unwrap();

    assert!(
        container::detect_format(&bytes) == Format::V2,
        "detected as v2 container"
    );
    let h = container::parse_header(&bytes);
    assert!(
        h.file_version == "2.1"
            && h.app_version == "2.1.3"
            && h.checksum.len() == 32
            && h.chunk_length == 108 + h.thumbnail_length as usize,
        "header: version={} app={} dbSize={} chunk={}",
        h.file_version,
        h.app_version,
        h.db_size,
        h.chunk_length
    );
    assert!(
        container::verify_checksum(&bytes),
        "checksum md5(file[104:]) matches stored"
    );

    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] = tampered[last].wrapping_add(1);
    assert!(
        !container::verify_checksum(&tampered),
        "tampered byte fails checksum"
    );

    let thumb = container::extract_thumbnail(&bytes);
    assert!(
        thumb.len() == h.thumbnail_length as usize
            && thumb.len() > 2
            && thumb[0] == 0xFF
            && thumb[1] == 0xD8,
        "thumbnail extracted ({} B, JPEG SOI)",
        thumb.len()
    );
    let db = container::extract_database(&bytes);
    assert!(
        db.len() as i64 == h.db_size && &db[0..15] == b"SQLite format 3",
        "DB reassembled ({} B, SQLite magic)",
        db.len()
    );
    assert_eq!(
        container::build(&db, &thumb),
        bytes,
        "container rebuild byte-identical ({} B)",
        bytes.len()
    );
}

// ============================================================ scene read + rebuild

/// Reads the golden container and checks the parsed scene fields.
#[test]
fn scene_read_from_golden() {
    let pf = PurFile::new(scratch_dir());
    let scene = pf.read(&fixture("S1_redbar-new.pur"), true);

    assert_eq!(scene.images.len(), 1);
    let img = &scene.images[0];
    assert_eq!(img.id, 0);
    assert_eq!(img.width, Some(64));
    assert_eq!(img.height, Some(48));
    assert_eq!(img.format.as_deref(), Some("PNG"));
    assert_eq!(img.source_type, Some(1));
    let data = img.data.as_ref().expect("image data");
    assert_eq!(&data[..4], &[0x89, 0x50, 0x4E, 0x47], "embedded PNG bytes");

    assert_eq!(scene.items.len(), 1);
    let it = &scene.items[0];
    assert_eq!(
        it.name.as_deref(),
        Some("a_red64x48.png")
    );
    assert_eq!(it.transform.unwrap().m11, 15.625);
    assert_eq!(it.sort_order.as_ref().unwrap().value(), 1.0);
    assert_eq!(it.parent, -1);
    assert_eq!(it.locked, Some(0));
    assert_eq!(it.z, Some(1.0));

    assert_eq!(scene.items_images.len(), 1);
    let ii = &scene.items_images[0];
    assert_eq!(ii.image, Some(0));
    assert_eq!(ii.flags, Some(1));
    assert_eq!(ii.image_bounds.as_ref().unwrap().elements.len(), 5);

    let md = scene.metadata.as_ref().expect("metadata row");
    assert_eq!(md.id, 0);
    assert_eq!(md.thumbnail.as_ref().unwrap().len(), 1699);
    assert_eq!(md.saved, Some(1));
    assert_eq!(md.application_version.as_deref(), Some("2.1.3"));
    assert_eq!(
        md.scene_rect,
        Some(QRectF {
            x: -10000.0,
            y: -10000.0,
            w: 11000.0,
            h: 10750.0
        })
    );
    assert_eq!(md.view_transform, Some(QTransform::IDENTITY));
    assert_eq!(md.horizontal_scroll, Some(0));
    assert_eq!(md.vertical_scroll, Some(0));

    assert!(scene.groups.is_empty() && scene.drawings.is_empty() && scene.notes.is_empty());
}

/// BYTE-IDENTICAL rebuild: parse S1 scene + thumbnail -> writeBytes -> == original file.
/// Exercises the full write path: DDL + pragmas + 3 store commits + the SQLite version
/// stamp (see `purfile::REFERENCE_SQLITE_VERSION`) + container splice.
#[test]
fn write_bytes_byte_identical() {
    let bytes = std::fs::read(fixture("S1_redbar-new.pur")).unwrap();
    let pf = PurFile::new(scratch_dir());
    let scene = pf.read_bytes(&bytes, true);
    let thumb = container::extract_thumbnail(&bytes);

    let rebuilt = pf.write_bytes(&scene, &thumb);
    assert_eq!(rebuilt.len(), bytes.len(), "rebuilt file size");
    assert!(
        rebuilt == bytes,
        "writeBytes must reproduce the original file byte-for-byte"
    );
}

/// Round-trip through a real file: write() is atomic (temp + rename) and reads back equal.
#[test]
fn write_and_reread_file() {
    let bytes = std::fs::read(fixture("S1_redbar-new.pur")).unwrap();
    let pf = PurFile::new(scratch_dir());
    let scene = pf.read_bytes(&bytes, true);
    let thumb = container::extract_thumbnail(&bytes);

    let out = scratch_dir().join("roundtrip.pur");
    pf.write(&scene, &out, &thumb);
    assert_eq!(
        std::fs::read(&out).unwrap(),
        bytes,
        "file write is byte-identical"
    );
    let scene2 = pf.read(&out, true);
    assert_eq!(scene2, scene, "scene survives a file round-trip");
    let _ = std::fs::remove_file(&out);
}

#[test]
#[should_panic(expected = "legacy v1.x")]
fn read_rejects_legacy() {
    PurFile::new(scratch_dir()).read(&fixture("legacy/S1_redbar.pur"), true);
}

// ================================================================ legacy v1.x
// port of PurCoreSelftest.selftestLegacy

#[test]
fn legacy_v1() {
    let bytes = std::fs::read(fixture("legacy/S1_redbar.pur")).unwrap();

    assert!(
        container::detect_format(&bytes) == Format::Legacy,
        "detected as legacy v1.x"
    );
    let scene = legacy::read_bytes(&bytes, true); // verify_checksum = true must not panic
    assert!(
        scene.checksum_matches(),
        "checksum md5(file[108:]) matches stored"
    );

    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] = tampered[last].wrapping_add(1);
    assert!(
        !legacy::read_bytes(&tampered, false).checksum_matches(),
        "tampered byte fails checksum"
    );

    // field-for-field dump diff against the Python reference dump
    let tsv = std::fs::read_to_string(fixture("legacy-dumps/S1_redbar.tsv")).unwrap();
    let want: Vec<&str> = tsv.lines().filter(|l| !l.is_empty()).collect();
    let got = legacy_dump(&scene, "S1_redbar.pur");
    assert_eq!(
        got.len(),
        want.len(),
        "dump row count (Python reference: {})",
        want.len()
    );
    let mut diffs = 0;
    for i in 0..want.len().min(got.len()) {
        if want[i] != got[i] {
            diffs += 1;
            if diffs <= 5 {
                eprintln!(
                    "     row {i}:\n       want: {}\n       got : {}",
                    want[i], got[i]
                );
            }
        }
    }
    assert_eq!(diffs, 0, "field-for-field dump matches Python reference");
}

#[test]
#[should_panic(expected = "legacy checksum mismatch")]
fn legacy_tampered_rejected_on_read() {
    let bytes = std::fs::read(fixture("legacy/S1_redbar.pur")).unwrap();
    let mut tampered = bytes.clone();
    let last = tampered.len() - 1;
    tampered[last] = tampered[last].wrapping_add(1);
    legacy::read_bytes(&tampered, true);
}

// -------------------------------------------------------------------- legacy dump
// same rows, same order, same escaping/format helpers as the Python harness

fn legacy_dump(s: &legacy::LegacyScene, file_name: &str) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    macro_rules! row {
        ($k:expr, $v:expr) => {
            rows.push(format!("{}\t{}", $k, $v))
        };
    }
    row!("file", file_name);
    for (i, v) in s.canvas.iter().enumerate() {
        row!(format!("canvas.{i}"), legacy_fmt_double(*v));
    }
    row!("zoom", legacy_fmt_double(s.zoom));
    row!("xCanvas", s.x_canvas);
    row!("yCanvas", s.y_canvas);
    row!("folderLocation", legacy_esc(&s.folder_location));
    row!("checksum.stored", &s.stored_checksum);
    row!("checksum.computed", &s.computed_checksum);
    row!("checksum.matches", s.checksum_matches());
    row!("image.count", s.images.len());
    for (i, img) in s.images.iter().enumerate() {
        row!(format!("image.{i}.address.0"), img.address_start);
        row!(format!("image.{i}.address.1"), img.address_end);
        row!(format!("image.{i}.png.length"), img.png.len());
        row!(
            format!("image.{i}.png.md5"),
            purcore::checksum::md5_hex(&img.png)
        );
        row!(
            format!("image.{i}.head4"),
            img.png
                .iter()
                .take(4)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        row!(format!("image.{i}.transform.count"), img.transforms.len());
        for (j, t) in img.transforms.iter().enumerate() {
            legacy_dump_image_item(&mut rows, &format!("image.{i}.transform.{j}"), t);
        }
    }
    row!("text.count", s.texts.len());
    for (i, t) in s.texts.iter().enumerate() {
        legacy_dump_text_item(&mut rows, &format!("text.{i}"), t);
    }
    rows
}

fn legacy_dump_image_item(rows: &mut Vec<String>, prefix: &str, t: &legacy::LegacyImageItem) {
    macro_rules! row {
        ($k:expr, $v:expr) => {
            rows.push(format!("{prefix}.{}\t{}", $k, $v))
        };
    }
    row!("source", legacy_esc(&t.source));
    row!("name", legacy_esc(&t.name));
    for (i, v) in t.matrix.iter().enumerate() {
        row!(format!("matrix.{i}"), legacy_fmt_double(*v));
    }
    row!("x", legacy_fmt_double(t.x));
    row!("y", legacy_fmt_double(t.y));
    row!("id", t.id);
    row!("zLayer", legacy_fmt_double(t.z_layer));
    for (i, v) in t.matrix_before_crop.iter().enumerate() {
        row!(format!("matrixBeforeCrop.{i}"), legacy_fmt_double(*v));
    }
    row!("xCrop", legacy_fmt_double(t.x_crop));
    row!("yCrop", legacy_fmt_double(t.y_crop));
    row!("scaleCrop", legacy_fmt_double(t.scale_crop));
    row!("point.count", t.points_x.len());
    for k in 0..t.points_x.len() {
        row!(format!("point.{k}.x"), legacy_fmt_double(t.points_x[k]));
        row!(format!("point.{k}.y"), legacy_fmt_double(t.points_y[k]));
    }
    row!("child.count", t.text_children.len());
    for (i, c) in t.text_children.iter().enumerate() {
        legacy_dump_text_item(rows, &format!("{prefix}.child.{i}"), c);
    }
}

fn legacy_dump_text_item(rows: &mut Vec<String>, prefix: &str, t: &legacy::LegacyTextItem) {
    macro_rules! row {
        ($k:expr, $v:expr) => {
            rows.push(format!("{prefix}.{}\t{}", $k, $v))
        };
    }
    row!("text", legacy_esc(&t.text));
    for (i, v) in t.matrix.iter().enumerate() {
        row!(format!("matrix.{i}"), legacy_fmt_double(*v));
    }
    row!("x", legacy_fmt_double(t.x));
    row!("y", legacy_fmt_double(t.y));
    row!("id", t.id);
    row!("zLayer", legacy_fmt_double(t.z_layer));
    row!("opacity", t.opacity);
    for (i, v) in t.rgb.iter().enumerate() {
        row!(format!("rgb.{i}"), v);
    }
    row!("opacityBackground", t.opacity_background);
    for (i, v) in t.rgb_background.iter().enumerate() {
        row!(format!("rgbBackground.{i}"), v);
    }
    row!("child.count", t.text_children.len());
    for (i, c) in t.text_children.iter().enumerate() {
        legacy_dump_text_item(rows, &format!("{prefix}.child.{i}"), c);
    }
}

/// Canonical double for the dump: shortest-round-trip digits in `<d>.<frac>e<exp>` form
/// (Rust `{:e}` gives the same shortest digits as JVM `Double.toString`/Python repr, only
/// the notation differs, so this normalizes it). Must match `fmt` in dump_legacy.py.
fn legacy_fmt_double(v: f64) -> String {
    let s = format!("{v:e}");
    let (sign, rest) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s.as_str()),
    };
    let (mant, exp10) = match rest.split_once('e') {
        Some((m, e)) => (m, e.parse::<i64>().unwrap()),
        None => (rest, 0),
    };
    let (ip, fp) = match mant.split_once('.') {
        Some((a, b)) => (a, b),
        None => (mant, ""),
    };
    let digits = format!("{ip}{fp}");
    let point_pos = ip.len();
    let db = digits.as_bytes();
    let mut z = 0;
    while z < db.len() && db[z] == b'0' {
        z += 1;
    }
    if z == db.len() {
        return format!("{sign}0.0");
    }
    let mut mant = digits[z..].to_string();
    while mant.len() > 1 && mant.ends_with('0') {
        mant.pop();
    }
    let exp = point_pos as i64 - 1 - z as i64 + exp10;
    let frac = if mant.len() > 1 { &mant[1..] } else { "0" };
    format!("{sign}{}.{}e{}", &mant[..1], frac, exp)
}

/// TSV escaping; must match `esc` in dump_legacy.py (backslash first).
fn legacy_esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ================================================================ blob binding
// port of BlobBindingTest.kt (the R1 guard: blob columns must be TEXT, never BLOB)

#[test]
fn storage_classes_preserved() {
    with_temp_db(|db| {
        db.execute("CREATE TABLE t (v)", []).unwrap();
        db.execute(
            "INSERT INTO t (v) VALUES (?)",
            rusqlite::params![rusqlite::types::Value::Null],
        )
        .unwrap();
        db.execute("INSERT INTO t (v) VALUES (?)", rusqlite::params![42i64])
            .unwrap();
        db.execute("INSERT INTO t (v) VALUES (?)", rusqlite::params![2.5f64])
            .unwrap();
        db.execute(
            "INSERT INTO t (v) VALUES (?)",
            rusqlite::params!["text value"],
        )
        .unwrap();
        db.execute(
            "INSERT INTO t (v) VALUES (?)",
            rusqlite::params![vec![0u8, 127, 255, 0]],
        )
        .unwrap();

        let rows: Vec<(rusqlite::types::Value, String)> =
            query_rows(db, "SELECT v, typeof(v) FROM t", |r| {
                (r.get(0).unwrap(), r.get(1).unwrap())
            });
        assert_eq!(
            rows.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
            ["null", "integer", "real", "text", "blob"]
        );
        assert_eq!(rows[0].0, rusqlite::types::Value::Null);
        assert_eq!(rows[1].0, rusqlite::types::Value::Integer(42));
        assert_eq!(rows[2].0, rusqlite::types::Value::Real(2.5));
        assert_eq!(rows[3].0, rusqlite::types::Value::Text("text value".into()));
        assert_eq!(
            rows[4].0,
            rusqlite::types::Value::Blob(vec![0, 127, 255, 0])
        );
    });
}

#[test]
fn latin1_blobs_stay_text_and_reread_byte_identical() {
    let fixtures = load_fixtures();
    assert_eq!(fixtures.len(), 16, "golden blob set");

    with_temp_db(|db| {
        db.execute("CREATE TABLE b (name TEXT, data TEXT)", [])
            .unwrap();
        for f in &fixtures {
            db.execute(
                "INSERT INTO b (name, data) VALUES (?, ?)",
                rusqlite::params![f.name, blobs::bytes_to_stored_text(&f.bytes)],
            )
            .unwrap();
        }
        // every blob column must be stored as TEXT, never BLOB (the R1 hazard)
        let types: Vec<String> = query_rows(db, "SELECT DISTINCT typeof(data) FROM b", |r| {
            r.get(0).unwrap()
        });
        assert_eq!(types, ["text"]);

        // and must decode back to the exact serialized bytes (compare by row: names repeat
        // in the golden set — S2 has two items, so e.g. s2.items_transform appears twice)
        let rows: Vec<(String, String)> =
            query_rows(db, "SELECT name, data FROM b ORDER BY rowid", |r| {
                (r.get(0).unwrap(), r.get(1).unwrap())
            });
        assert_eq!(rows.len(), fixtures.len());
        for (i, f) in fixtures.iter().enumerate() {
            assert_eq!(&rows[i].0, &f.name);
            assert_eq!(
                blobs::stored_text_to_bytes(&rows[i].1),
                f.bytes,
                "{}",
                f.name
            );
        }
    });
}

fn with_temp_db(block: impl FnOnce(&Connection)) {
    let dir = scratch_dir();
    let db_file = dir.join(format!(
        "blob-binding-{}-{}.db",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let db = Connection::open(&db_file).unwrap();
    block(&db);
    drop(db);
    let _ = std::fs::remove_file(&db_file);
}

fn query_rows<T>(db: &Connection, sql: &str, map: impl Fn(&rusqlite::Row) -> T) -> Vec<T> {
    let mut stmt = db.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| Ok(map(r))).unwrap();
    rows.map(|r| r.unwrap()).collect()
}
