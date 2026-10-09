//! Board open/save: `.ref` (native board file) and `.pur` (scene interchange).
//!
//! `.ref` = the [`crate::serializer`] codec over drawable state. `.pur` open
//! maps scene rows to stickers (new scenes get fresh sort slots); save inverts
//! the mapping.

use std::path::Path;

use crate::board::Board;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FileFormat {
    Ref,
    Pur,
}

/// Sniff `.pur` (v2 container or legacy v1.x) vs `.ref`.
pub fn detect(bytes: &[u8]) -> FileFormat {
    unimplemented!()
}

pub fn open_ref(bytes: &[u8]) -> Result<Board, String> {
    unimplemented!()
}

pub fn save_ref(board: &Board) -> Vec<u8> {
    unimplemented!()
}

/// Read a `.pur` scene into a board. Needs a scratch dir for the embedded DB.
pub fn open_pur(bytes: &[u8], scratch: &Path) -> Result<Board, String> {
    unimplemented!()
}

/// Write the board as a `.pur` scene (atomic temp+rename at the target path
/// is the shell's job; this returns the file bytes).
pub fn save_pur(board: &Board, scratch: &Path, thumbnail: &[u8]) -> Vec<u8> {
    unimplemented!()
}
