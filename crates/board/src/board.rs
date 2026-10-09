//! Board: sticker list, selection, canvas, and the pointer state machine.
//!
//! The shell (egui harness, Android view) feeds pointer events and applies the
//! returned [`BoardAction`]s (redraw, selection changes). No rendering here.

use crate::icons;
use crate::math::Mat3;
use crate::sticker::Sticker;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActionMode {
    None,
    Drag,
    PinchSticker,
    Icon,
    Click,
    CanvasDrag,
    CanvasPinch,
}

#[derive(Clone, Copy, Debug)]
pub struct Pointer {
    pub id: i64,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum BoardEvent {
    Down(Pointer),
    Move(Pointer),
    Up(Pointer),
}

/// What the shell must do after events. Returned from [`Board::handle_event`].
#[derive(Debug)]
pub enum BoardAction {
    Redraw,
    StickerAdded(usize),
    StickerDeleted(usize),
    Clicked(usize),
    DoubleTapped(usize),
    StickerMoved(usize),
}

pub struct Board {
    pub stickers: Vec<Sticker>,
    pub handling: Option<usize>,
    pub canvas: Mat3,
}

impl Board {
    pub fn new() -> Self {
        unimplemented!()
    }
    /// Push a sticker, select it, and report its index.
    pub fn add_sticker(&mut self, s: Sticker) -> usize {
        unimplemented!()
    }
    pub fn remove_all(&mut self) {
        unimplemented!()
    }
    pub fn stickers(&self) -> &[Sticker] {
        unimplemented!()
    }
    pub fn canvas_matrix(&self) -> Mat3 {
        unimplemented!()
    }
    /// Replace the canvas matrix and re-push it to every sticker.
    pub fn set_canvas_matrix(&mut self, m: Mat3) {
        unimplemented!()
    }
    /// Feed one pointer event; returns shell actions. Owns the full
    /// select/drag/pinch-canvas/pinch-sticker/click state machine.
    pub fn handle_event(&mut self, ev: BoardEvent) -> Vec<BoardAction> {
        unimplemented!()
    }
}
