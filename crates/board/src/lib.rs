//! Platform-free board model for LazyRef.
//!
//! No rendering, no window, no Android: pure data + math + persistence that any
//! shell (egui harness today, Android tomorrow) drives through [`board::Board`].
//! Behavior follows the DroidRef app's board semantics; the `.pur` mapping
//! follows `purcore` + `docs/FORMAT.md`.

pub mod board;
pub mod icons;
pub mod io;
pub mod math;
pub mod scene;
pub mod serializer;
pub mod settings;
pub mod sticker;
pub mod transform;
