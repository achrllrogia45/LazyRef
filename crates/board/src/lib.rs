//! Platform-free board model for LazyRef.
//!
//! No rendering, no window, no Android: pure data + math + persistence that any
//! shell (egui harness today, Android tomorrow) drives through the board model.
//! Behavior follows the DroidRef app's board semantics; the `.pur` mapping
//! follows `purcore` + `docs/FORMAT.md`.
//!
//! Module wiring (`pub mod …`) lands at harvest once lanes deliver.
