//! Rust implementation of the PureRef 2.x `.pur` format: container codec, blob codecs,
//! SQLite scene store, legacy v1.x reader.
//!
//! Byte-exact serialization is the contract: blob codecs, the container splice and the SQLite
//! byte stream are verified against the golden corpus in `tests/` and documented in
//! `docs/FORMAT.md`.

pub mod blobs;
pub mod checksum;
pub mod container;
pub mod legacy;
pub mod model;
pub mod purfile;
pub mod scene_db;

pub(crate) mod be;
