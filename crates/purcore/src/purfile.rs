//! Public API of pur-core: read a `.pur` file into a [`Scene`], write a [`Scene`] back as a
//! PureRef 2.x container.
//!
//! SQLite needs a real file path, so the embedded DB is materialized into a scratch file
//! (caller-supplied dir; the app passes `context.cacheDir`) for the duration of the call.
//!
//! ponytail: one scratch file per call, deleted on exit; pool them only if read/write
//! throughput ever matters (it's a user-initiated board op, not a hot loop). A panic mid-call
//! leaks one scratch file — harmless, the app's cache dir prunes.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{Connection, OpenFlags};

use crate::container::{self, Format};
use crate::model::Scene;
use crate::scene_db;

/// SQLite stamps the DB header [96:100] with the version of the library that last wrote the
/// file. The reference writer (PureRef 2.1.3) bundles SQLite 3.45.3; rusqlite links a newer
/// bundled SQLite whose own stamp would leave a 2-byte diff against the corpus. The field is
/// informational — no reader consults it — so pin it to the reference version to keep written
/// files byte-identical to app output.
const REFERENCE_SQLITE_VERSION: u32 = 3_045_003; // SQLite 3.45.3

static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

pub struct PurFile {
    scratch_dir: PathBuf,
}

impl PurFile {
    pub fn new(scratch_dir: impl Into<PathBuf>) -> Self {
        Self {
            scratch_dir: scratch_dir.into(),
        }
    }

    /// Reads a desktop `.pur`. Verifies the container checksum by default (PureRef rejects
    /// mismatches); pass `verify_checksum = false` for an explicit
    /// "open anyway" path.
    ///
    /// Legacy v1.x files are not a [`Scene`]: they predate the SQLite container and have no
    /// equivalent tables. Read them with [`crate::legacy::read`] (returns a dedicated
    /// [`crate::legacy::LegacyScene`]) instead.
    pub fn read(&self, path: &Path, verify_checksum: bool) -> Scene {
        let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        self.read_bytes(&bytes, verify_checksum)
    }

    pub fn read_bytes(&self, bytes: &[u8], verify_checksum: bool) -> Scene {
        match container::detect_format(bytes) {
            Format::V2 => {}
            Format::Legacy => {
                panic!("legacy v1.x .pur: not a v2 Scene — use purcore::legacy::read(path) instead")
            }
        }
        if verify_checksum {
            assert!(
                container::verify_checksum(bytes),
                "container checksum mismatch (md5(file[104:]))"
            );
        }
        let scratch = self.scratch_file("read");
        fs::write(&scratch, container::extract_database(bytes)).expect("write scratch db");
        let scene = {
            let db = Connection::open_with_flags(&scratch, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .expect("open scratch db");
            scene_db::read(&db)
        };
        let _ = fs::remove_file(&scratch);
        scene
    }

    /// Builds a complete v2 file: new SQLite DB in a scratch file, then the container.
    /// Reproduces app-written files byte-for-byte given identical DB content and thumbnail
    /// (verified on the golden corpus).
    pub fn write_bytes(&self, scene: &Scene, thumbnail: &[u8]) -> Vec<u8> {
        let scratch = self.scratch_file("write");
        let mut db = Connection::open(&scratch).expect("open scratch db");
        scene_db::create_schema(&db);
        scene_db::write(&mut db, scene);
        drop(db);
        let mut db_bytes = fs::read(&scratch).expect("read scratch db");
        let _ = fs::remove_file(&scratch);
        db_bytes[96..100].copy_from_slice(&REFERENCE_SQLITE_VERSION.to_be_bytes());
        container::build(&db_bytes, thumbnail)
    }

    /// Writes [`Self::write_bytes`] atomically: temp file in the target dir, then rename.
    pub fn write(&self, scene: &Scene, out_path: &Path, thumbnail: &[u8]) {
        let bytes = self.write_bytes(scene, thumbnail);
        let tmp = out_path.with_file_name(format!(
            "{}.{}.tmp",
            out_path
                .file_name()
                .expect("out path has a file name")
                .to_string_lossy(),
            std::process::id()
        ));
        fs::write(&tmp, bytes).expect("write temp file");
        if fs::rename(&tmp, out_path).is_err() {
            fs::copy(&tmp, out_path).expect("copy temp file");
            let _ = fs::remove_file(&tmp);
        }
    }

    fn scratch_file(&self, kind: &str) -> PathBuf {
        fs::create_dir_all(&self.scratch_dir).expect("create scratch dir");
        self.scratch_dir.join(format!(
            "pur-core-{kind}-{}-{}.db",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
