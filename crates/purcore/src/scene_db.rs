//! Scene database: the 7-table schema (recon/31 §2 / recon/35 §1, verbatim DDL) and row <->
//! [`Scene`] mapping (Kotlin `SceneDb.kt`; rusqlite is the engine, replacing the Kotlin
//! `SqliteEngine` seam).
//!
//! Blob columns are stored as TEXT via the latin-1 quirk ([`crate::blobs`]); every read
//! decodes, every write re-encodes field-for-field so desktop->Android->desktop round-trips
//! stay byte-stable.

use rusqlite::types::Value;
use rusqlite::Connection;

use crate::blobs::{self};
use crate::model::*;

/// `PRAGMA application_id` — 0x3812c3fe (recon/31 §1).
pub const APPLICATION_ID: i64 = 940753918;

/// `PRAGMA user_version` — 0x30da5 (recon/31 §1).
pub const USER_VERSION: i64 = 200101;

/// Application version written by PureRef 2.1.3.
pub const APPLICATION_VERSION: &str = "2.1.3";

/// Verbatim DDL from recon/35 §1 (byte-identical across two independent live DBs, including
/// no space after commas).
///
/// Order matches the rootpage order in app-written corpus files (verified 9/9) — the app's
/// new-DB path creates tables in this order and the byte-exact page layout depends on it.
/// (The Kotlin list order differs; it never wrote a DB from scratch, so it never noticed.)
pub const SCHEMA: [&str; 7] = [
    "CREATE TABLE images (width INTEGER,checksum TEXT,format TEXT,source TEXT,height INTEGER,data BLOB,source_type INTEGER,origin TEXT,id INTEGER PRIMARY KEY);",
    "CREATE TABLE items_groups (lock_mode INTEGER,background_color TEXT,id INTEGER PRIMARY KEY);",
    "CREATE TABLE items_drawings (strokes BLOB,id INTEGER PRIMARY KEY);",
    "CREATE TABLE items_notes (fixed_size TEXT,background_color TEXT,text_color TEXT,style INTEGER,text TEXT,id INTEGER PRIMARY KEY);",
    "CREATE TABLE items_images (playback_speed REAL,playback_frame INTEGER,playback_state INTEGER,image_bounds BLOB,image_transform BLOB,flags INTEGER,image INTEGER,id INTEGER PRIMARY KEY);",
    "CREATE TABLE items (comment INTEGER,opacity REAL,z REAL,locked INTEGER,parent INTEGER,sort_order BLOB,transform BLOB,name TEXT,id INTEGER PRIMARY KEY);",
    "CREATE TABLE metadata (thumbnail BLOB,saved INTEGER,last_load_checksum TEXT,last_load_path TEXT,last_save_path TEXT,vertical_scroll INTEGER,scene_rect TEXT,view_transform TEXT,application_version TEXT,horizontal_scroll INTEGER,id INTEGER PRIMARY KEY);",
];

/// New-database setup: pragmas + DDL, matching the app's `getConnection` new-DB path.
///
/// The setup pragmas run via `execute_batch` but NOT in a transaction (SQLite rejects
/// changing `synchronous` / `journal_mode` / `auto_vacuum` inside one); the data inserts in
/// [`write`] stay atomic per store.
pub fn create_schema(conn: &Connection) {
    conn.execute_batch(
        "PRAGMA encoding = 'UTF-8';\n\
         PRAGMA journal_mode = DELETE;\n\
         PRAGMA synchronous = EXTRA;\n\
         PRAGMA auto_vacuum = 1;",
    )
    .expect("pragma setup");
    for ddl in SCHEMA {
        conn.execute(ddl, []).expect("create table");
    }
    conn.execute_batch(&format!(
        "PRAGMA application_id = {APPLICATION_ID};\nPRAGMA user_version = {USER_VERSION};"
    ))
    .expect("pragma application_id/user_version");
}

// ------------------------------------------------------------------ reading

pub fn read(conn: &Connection) -> Scene {
    Scene {
        images: query(conn, "SELECT id,width,height,checksum,format,source,source_type,origin,data FROM images ORDER BY id")
            .iter()
            .map(|r| Image {
                id: req_long(&r[0], "images.id"),
                width: as_int(&r[1]),
                height: as_int(&r[2]),
                checksum: as_string(&r[3]),
                format: as_string(&r[4]),
                source: as_string(&r[5]),
                source_type: as_int(&r[6]),
                origin: as_string(&r[7]),
                data: as_blob(&r[8], "images.data"),
            })
            .collect(),
        items: query(conn, "SELECT id,name,transform,sort_order,z,opacity,parent,locked,comment FROM items ORDER BY id")
            .iter()
            .map(|r| Item {
                id: req_long(&r[0], "items.id"),
                name: as_string(&r[1]),
                transform: as_blob(&r[2], "items.transform").map(|b| blobs::decode_transform(&b)),
                sort_order: as_blob(&r[3], "items.sort_order").map(|b| blobs::decode_big_rational(&b)),
                z: as_double(&r[4]),
                opacity: as_double(&r[5]),
                parent: as_long(&r[6]).unwrap_or(-1),
                locked: as_int(&r[7]),
                comment: as_string(&r[8]),
            })
            .collect(),
        items_images: query(conn, "SELECT id,image,image_transform,image_bounds,flags,playback_state,playback_frame,playback_speed FROM items_images ORDER BY id")
            .iter()
            .map(|r| ItemImage {
                id: req_long(&r[0], "items_images.id"),
                image: as_long(&r[1]),
                image_transform: as_blob(&r[2], "items_images.image_transform")
                    .map(|b| blobs::decode_transform(&b)),
                image_bounds: as_blob(&r[3], "items_images.image_bounds")
                    .map(|b| blobs::decode_path(&b)),
                flags: as_int(&r[4]),
                playback_state: as_int(&r[5]),
                playback_frame: as_int(&r[6]),
                playback_speed: as_double(&r[7]),
            })
            .collect(),
        groups: query(conn, "SELECT id,background_color,lock_mode FROM items_groups ORDER BY id")
            .iter()
            .map(|r| Group {
                id: req_long(&r[0], "items_groups.id"),
                background_color: as_string(&r[1]),
                lock_mode: as_int(&r[2]),
            })
            .collect(),
        drawings: query(conn, "SELECT id,strokes FROM items_drawings ORDER BY id")
            .iter()
            .map(|r| Drawing {
                id: req_long(&r[0], "items_drawings.id"),
                strokes: as_blob(&r[1], "items_drawings.strokes"),
            })
            .collect(),
        notes: query(conn, "SELECT id,text,background_color,text_color,style,fixed_size FROM items_notes ORDER BY id")
            .iter()
            .map(|r| Note {
                id: req_long(&r[0], "items_notes.id"),
                text: as_string(&r[1]),
                background_color: as_string(&r[2]),
                text_color: as_string(&r[3]),
                style: as_int(&r[4]),
                fixed_size: as_string(&r[5]),
            })
            .collect(),
        metadata: query(conn, "SELECT id,thumbnail,saved,last_load_checksum,last_load_path,last_save_path,vertical_scroll,scene_rect,view_transform,application_version,horizontal_scroll FROM metadata ORDER BY id LIMIT 1")
            .first()
            .map(|r| Metadata {
                id: req_long(&r[0], "metadata.id"),
                thumbnail: as_blob(&r[1], "metadata.thumbnail"),
                saved: as_int(&r[2]),
                last_load_checksum: as_string(&r[3]),
                last_load_path: as_string(&r[4]),
                last_save_path: as_string(&r[5]),
                vertical_scroll: as_int(&r[6]),
                scene_rect: as_blob(&r[7], "metadata.scene_rect").map(|b| blobs::decode_rect_f(&b)),
                view_transform: as_blob(&r[8], "metadata.view_transform")
                    .map(|b| blobs::decode_transform(&b)),
                application_version: as_string(&r[9]),
                horizontal_scroll: as_int(&r[10]),
            }),
    }
}

// ------------------------------------------------------------------ writing

/// Inserts the full scene. [`create_schema`] must have run first.
///
/// Commit granularity mirrors the reference app's store pipeline (`storeImage` /
/// `storeItem` / `storeSceneState` each commit) — three transactions. The golden corpus
/// header counters depend on it (e.g. S1: 7 DDL + 3 pragmas + 3 store commits = 13), which
/// is what makes a from-scratch rewrite byte-identical.
pub fn write(conn: &mut Connection, scene: &Scene) {
    {
        let tx = conn.transaction().expect("begin images");
        for i in &scene.images {
            tx.execute(
                "INSERT INTO images(id,width,height,checksum,format,source,source_type,origin,data) VALUES(?,?,?,?,?,?,?,?,?)",
                rusqlite::params![i.id, i.width, i.height, i.checksum, i.format, i.source, i.source_type, i.origin, i.data],
            )
            .expect("insert image");
        }
        tx.commit().expect("commit images");
    }
    {
        let tx = conn.transaction().expect("begin items");
        for it in &scene.items {
            tx.execute(
                "INSERT INTO items(id,name,transform,sort_order,z,opacity,parent,locked,comment) VALUES(?,?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    it.id,
                    it.name,
                    it.transform.as_ref().map(|t| stored(&blobs::encode_transform(t))),
                    it.sort_order.as_ref().map(|o| stored(&blobs::encode_big_rational(o))),
                    it.z,
                    it.opacity,
                    it.parent,
                    it.locked,
                    it.comment
                ],
            )
            .expect("insert item");
        }
        for ii in &scene.items_images {
            tx.execute(
                "INSERT INTO items_images(id,image,image_transform,image_bounds,flags,playback_state,playback_frame,playback_speed) VALUES(?,?,?,?,?,?,?,?)",
                rusqlite::params![
                    ii.id,
                    ii.image,
                    ii.image_transform.as_ref().map(|t| stored(&blobs::encode_transform(t))),
                    ii.image_bounds.as_ref().map(|p| stored(&blobs::encode_path(p))),
                    ii.flags,
                    ii.playback_state,
                    ii.playback_frame,
                    ii.playback_speed
                ],
            )
            .expect("insert items_images");
        }
        for g in &scene.groups {
            tx.execute(
                "INSERT INTO items_groups(id,background_color,lock_mode) VALUES(?,?,?)",
                rusqlite::params![g.id, g.background_color, g.lock_mode],
            )
            .expect("insert items_groups");
        }
        for d in &scene.drawings {
            tx.execute(
                "INSERT INTO items_drawings(id,strokes) VALUES(?,?)",
                rusqlite::params![d.id, d.strokes],
            )
            .expect("insert items_drawings");
        }
        for n in &scene.notes {
            tx.execute(
                "INSERT INTO items_notes(id,text,background_color,text_color,style,fixed_size) VALUES(?,?,?,?,?,?)",
                rusqlite::params![n.id, n.text, n.background_color, n.text_color, n.style, n.fixed_size],
            )
            .expect("insert items_notes");
        }
        tx.commit().expect("commit items");
    }
    if let Some(m) = &scene.metadata {
        let tx = conn.transaction().expect("begin metadata");
        tx.execute(
            "INSERT INTO metadata(id,thumbnail,saved,last_load_checksum,last_load_path,last_save_path,vertical_scroll,scene_rect,view_transform,application_version,horizontal_scroll) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
            rusqlite::params![
                m.id,
                m.thumbnail,
                m.saved,
                m.last_load_checksum,
                m.last_load_path,
                m.last_save_path,
                m.vertical_scroll,
                m.scene_rect.as_ref().map(|r| stored(&blobs::encode_rect_f(r))),
                m.view_transform.as_ref().map(|t| stored(&blobs::encode_transform(t))),
                m.application_version,
                m.horizontal_scroll
            ],
        )
        .expect("insert metadata");
        tx.commit().expect("commit metadata");
    }
}

// ------------------------------------------------------------------ helpers

/// Serialized blob bytes -> TEXT value to bind (the latin-1 quirk; recon/33 §1).
fn stored(b: &[u8]) -> String {
    blobs::bytes_to_stored_text(b)
}

fn query(conn: &Connection, sql: &str) -> Vec<Vec<Value>> {
    let mut stmt = conn.prepare(sql).expect("prepare");
    let n = stmt.column_count();
    let mut rows = stmt.query([]).expect("query");
    let mut out = Vec::new();
    while let Some(row) = rows.next().expect("row") {
        out.push(
            (0..n)
                .map(|i| row.get::<usize, Value>(i).expect("column"))
                .collect(),
        );
    }
    out
}

fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::Text(t) => Some(t.clone()),
        other => panic!("cannot read {other:?} as String"),
    }
}

/// Blob column value -> serialized bytes.
///
/// PureRef writes the latin-1 TEXT form; a raw BLOB is passed through (never observed —
/// ponytail: revisit only if a real file shows the UTF-8-of-latin-1 BLOB variant).
fn as_blob(v: &Value, column: &str) -> Option<Vec<u8>> {
    match v {
        Value::Null => None,
        Value::Text(t) => {
            let b = blobs::stored_text_to_bytes(t);
            if b.is_empty() {
                None
            } else {
                Some(b)
            }
        }
        Value::Blob(b) => {
            if b.is_empty() {
                None
            } else {
                Some(b.clone())
            }
        }
        other => panic!("{column}: unexpected value type {other:?} (TEXT or BLOB expected)"),
    }
}

fn req_long(v: &Value, column: &str) -> i64 {
    as_long(v).unwrap_or_else(|| panic!("{column}: NULL id"))
}

fn as_long(v: &Value) -> Option<i64> {
    match v {
        Value::Null => None,
        Value::Integer(i) => Some(*i),
        Value::Real(f) => Some(*f as i64),
        Value::Text(t) => Some(t.parse().expect("cannot parse TEXT as Long")),
        Value::Blob(_) => panic!("cannot read BLOB as Long"),
    }
}

fn as_int(v: &Value) -> Option<i32> {
    match v {
        Value::Null => None,
        Value::Integer(i) => Some(*i as i32),
        Value::Real(f) => Some(*f as i32),
        Value::Text(t) => Some(t.parse().expect("cannot parse TEXT as Int")),
        Value::Blob(_) => panic!("cannot read BLOB as Int"),
    }
}

fn as_double(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Real(f) => Some(*f),
        Value::Integer(i) => Some(*i as f64),
        Value::Text(t) => Some(t.parse().expect("cannot parse TEXT as Double")),
        Value::Blob(_) => panic!("cannot read BLOB as Double"),
    }
}
