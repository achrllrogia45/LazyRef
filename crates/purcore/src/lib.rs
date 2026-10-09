//! Rust port of the `pur-core` `.pur` format library (PureRef 2.x SQLite format).
//!
//! Ported from the Kotlin implementation in `DroidRef-dev/purcore` (itself derived from the
//! PureRef 2.1.3 reverse-engineering dossiers in `bin-decode/recon/`). Byte-exact
//! serialization is the contract: blob codecs, the container splice and the SQLite byte
//! stream are verified against the golden corpus in `tests/`.

pub mod blobs;
pub mod checksum;
pub mod container;
pub mod legacy;
pub mod model;
pub mod purfile;
pub mod scene_db;

pub(crate) mod be;
