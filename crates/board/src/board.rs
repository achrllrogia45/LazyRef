//! Board: sticker list, selection, canvas, and the pointer state machine.
//!
//! The shell (egui harness, Android view) feeds pointer events and applies the
//! returned [`BoardAction`]s (redraw, selection changes). No rendering here.
//!
//! Pointer flow mirrors the app's board view-model: DOWN hits the topmost
//! sticker (selecting and optionally bringing it to front) or starts a canvas
//! pan; DRAG moves the handling sticker in world coordinates (screen delta
//! through the inverse canvas); a second pointer starts a sticker PINCH
//! (scale, or rotate when rotation is enabled) or a canvas zoom (scale about
//! the gesture midpoint); UP resolves tap vs double-tap via touch slop and a
//! 200 ms window, otherwise ends the drag.

use crate::math::Mat3;
use crate::sticker::Sticker;

/// Touch slop: a press that moves less than this (screen px) is a tap.
const TOUCH_SLOP: f32 = 8.0;
/// Tap-to-tap window for a double-tap, in milliseconds.
const DOUBLE_TAP_MS: f64 = 200.0;

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
#[derive(Debug, PartialEq)]
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
    /// Touch input ignored for stickers; every gesture pans/zooms the canvas.
    pub locked: bool,
    /// Touches that miss all stickers are dropped instead of panning.
    pub must_lock_to_pan: bool,
    /// Keep the handling sticker's center inside the viewport while dragging.
    pub constrained: bool,
    /// Two-finger sticker gesture rotates instead of scaling.
    pub rotation_enabled: bool,
    /// Selecting a sticker moves it to the top of the stack.
    pub bring_to_front: bool,
    mode: ActionMode,
    pointers: Vec<Pointer>,
    now_ms: f64,
    last_click_ms: f64,
    down: (f32, f32),
    canvas_anchor: Mat3,
    sticker_anchor_pos: (f32, f32),
    sticker_anchor_scale: (f32, f32),
    sticker_anchor_rot: f32,
    pinch_dist: f32,
    pinch_angle: f32,
    pinch_mid: (f32, f32),
    viewport: (f32, f32),
}

impl Board {
    pub fn new() -> Self {
        Board {
            stickers: Vec::new(),
            handling: None,
            canvas: Mat3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
            locked: false,
            must_lock_to_pan: false,
            constrained: false,
            rotation_enabled: false,
            bring_to_front: true,
            mode: ActionMode::None,
            pointers: Vec::new(),
            now_ms: 0.0,
            last_click_ms: -1_000_000.0,
            down: (0.0, 0.0),
            canvas_anchor: Mat3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
            sticker_anchor_pos: (0.0, 0.0),
            sticker_anchor_scale: (1.0, 1.0),
            sticker_anchor_rot: 0.0,
            pinch_dist: 0.0,
            pinch_angle: 0.0,
            pinch_mid: (0.0, 0.0),
            viewport: (0.0, 0.0),
        }
    }

    /// Push a sticker, select it, and report its index.
    pub fn add_sticker(&mut self, mut s: Sticker) -> usize {
        s.canvas = self.canvas;
        self.stickers.push(s);
        let idx = self.stickers.len() - 1;
        self.handling = Some(idx);
        idx
    }

    pub fn remove_all(&mut self) {
        self.stickers.clear();
        self.handling = None;
        self.mode = ActionMode::None;
        self.pointers.clear();
    }

    /// Remove the handling sticker, clear selection. None when nothing selected.
    pub fn remove_handling(&mut self) -> Option<usize> {
        let i = self.handling?;
        if i >= self.stickers.len() {
            self.handling = None;
            return None;
        }
        self.stickers.remove(i);
        self.handling = None;
        self.mode = ActionMode::None;
        self.pointers.clear();
        Some(i)
    }

    pub fn stickers(&self) -> &[Sticker] {
        &self.stickers
    }

    pub fn canvas_matrix(&self) -> Mat3 {
        self.canvas
    }

    /// Replace the canvas matrix and re-push it to every sticker.
    pub fn set_canvas_matrix(&mut self, m: Mat3) {
        self.canvas = m;
        self.push_canvas();
    }

    /// Clock for tap/double-tap detection, in milliseconds.
    pub fn set_now_ms(&mut self, ms: f64) {
        self.now_ms = ms;
    }

    /// Viewport size (screen px) used by [`Board::fit`] and the constrained flag.
    pub fn set_viewport(&mut self, w: f32, h: f32) {
        self.viewport = (w, h);
    }

    /// Feed one pointer event; returns shell actions. Owns the full
    /// select/drag/pinch-canvas/pinch-sticker/click state machine.
    pub fn handle_event(&mut self, ev: BoardEvent) -> Vec<BoardAction> {
        match ev {
            BoardEvent::Down(p) => self.on_down(p),
            BoardEvent::Move(p) => self.on_move(p),
            BoardEvent::Up(p) => self.on_up(p),
        }
    }

    /// Copy the handling sticker and select the copy.
    pub fn duplicate_handling(&mut self) -> Option<usize> {
        let i = self.handling?;
        if self.locked || i >= self.stickers.len() {
            return None;
        }
        let src = &self.stickers[i];
        let kind = match &src.kind {
            crate::sticker::StickerKind::Image { data, width, height } => {
                crate::sticker::StickerKind::Image { data: data.clone(), width: *width, height: *height }
            }
            crate::sticker::StickerKind::Text { text, size_px, color, background, bold } => {
                crate::sticker::StickerKind::Text {
                    text: text.clone(),
                    size_px: *size_px,
                    color: *color,
                    background: *background,
                    bold: *bold,
                }
            }
        };
        let copy = Sticker {
            transform: src.transform,
            canvas: self.canvas,
            width: src.width,
            height: src.height,
            crop: src.crop,
            kind,
        };
        self.stickers.push(copy);
        let idx = self.stickers.len() - 1;
        self.handling = Some(idx);
        Some(idx)
    }

    /// Mirror the handling sticker's scale on one or both axes.
    pub fn flip_handling(&mut self, horizontal: bool, vertical: bool) {
        if self.locked {
            return;
        }
        if let Some(s) = self.handling_sticker_mut() {
            if horizontal {
                s.transform.scale.0 = -s.transform.scale.0;
            }
            if vertical {
                s.transform.scale.1 = -s.transform.scale.1;
            }
        }
    }

    /// Reset the handling sticker's scale and rotation (keeps position/crop).
    pub fn reset_handling(&mut self) {
        if self.locked {
            return;
        }
        if let Some(s) = self.handling_sticker_mut() {
            s.transform.scale = (1.0, 1.0);
            s.transform.rotation = 0.0;
        }
    }

    /// Drag a crop edge (by handle gravity) to a screen point. Returns false
    /// when there is no handling sticker or the board is locked.
    pub fn crop_handling(&mut self, gravity: u32, x: f32, y: f32) -> bool {
        let i = match self.handling {
            Some(i) if i < self.stickers.len() && !self.locked => i,
            _ => return false,
        };
        let s = &mut self.stickers[i];
        let inv_canvas = match m_invert(s.canvas.0) {
            Some(m) => m,
            None => return false,
        };
        let (wx, wy) = m_map(inv_canvas, x, y);
        // World -> content-local: undo translate, rotate, scale about center.
        let (cx, cy) = (s.width / 2.0, s.height / 2.0);
        let (mut lx, mut ly) = (wx - s.transform.pos.0, wy - s.transform.pos.1);
        (lx, ly) = rot_about(lx, ly, cx, cy, -s.transform.rotation);
        if s.transform.scale.0.abs() < f32::EPSILON || s.transform.scale.1.abs() < f32::EPSILON {
            return false;
        }
        lx = cx + (lx - cx) / s.transform.scale.0;
        ly = cy + (ly - cy) / s.transform.scale.1;
        let g = flip_gravity_map(
            gravity,
            s.transform.scale.0 < 0.0,
            s.transform.scale.1 < 0.0,
        );
        let (mut l, mut t, mut r, mut b) = s.crop;
        match g {
            0 => {
                l = l.min(lx.min(r));
                t = t.min(ly.min(b));
            }
            1 => {
                r = r.max(lx.max(l));
                t = t.min(ly.min(b));
            }
            2 => {
                l = l.min(lx.min(r));
                b = b.max(ly.max(t));
            }
            3 => {
                r = r.max(lx.max(l));
                b = b.max(ly.max(t));
            }
            4 => l = l.min(lx.min(r)),
            5 => r = r.max(lx.max(l)),
            6 => t = t.min(ly.min(b)),
            _ => b = b.max(ly.max(t)),
        }
        // Clamp like the app: min 1px, inside real bounds.
        l = l.clamp(0.0, s.width - 1.0);
        t = t.clamp(0.0, s.height - 1.0);
        r = r.clamp(l + 1.0, s.width);
        b = b.clamp(t + 1.0, s.height);
        s.crop = (l, t, r, b);
        true
    }

    /// Zoom the canvas so `bounds` (world rect l,t,r,b) fits `viewport`
    /// (screen px w,h) with `margin` px on each side, centered.
    pub fn fit(&mut self, bounds: (f32, f32, f32, f32), viewport: (f32, f32), margin: f32) {
        self.viewport = viewport;
        let (bw, bh) = (bounds.2 - bounds.0, bounds.3 - bounds.1);
        let (aw, ah) = (viewport.0 - 2.0 * margin, viewport.1 - 2.0 * margin);
        if bw <= 0.0 || bh <= 0.0 || aw <= 0.0 || ah <= 0.0 {
            return;
        }
        let s = (aw / bw).min(ah / bh);
        if !s.is_finite() || s <= 0.0 {
            return;
        }
        let bc = ((bounds.0 + bounds.2) / 2.0, (bounds.1 + bounds.3) / 2.0);
        let vc = (viewport.0 / 2.0, viewport.1 / 2.0);
        let m = m_concat(
            m_translate(vc.0, vc.1),
            m_concat(m_scale(s), m_translate(-bc.0, -bc.1)),
        );
        self.canvas = Mat3(m);
        self.push_canvas();
    }

    fn handling_sticker_mut(&mut self) -> Option<&mut Sticker> {
        let i = self.handling?;
        self.stickers.get_mut(i)
    }

    fn push_canvas(&mut self) {
        for s in self.stickers.iter_mut() {
            s.canvas = self.canvas;
        }
    }

    fn track(&mut self, p: Pointer) {
        if let Some(slot) = self.pointers.iter_mut().find(|q| q.id == p.id) {
            *slot = p;
        } else {
            self.pointers.push(p);
        }
    }

    fn untrack(&mut self, id: i64) {
        self.pointers.retain(|q| q.id != id);
    }

    /// Topmost (last) sticker whose area contains the screen point.
    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        for (i, s) in self.stickers.iter().enumerate().rev() {
            if sticker_contains(s, x, y) {
                return Some(i);
            }
        }
        None
    }

    fn snapshot_sticker(&mut self, i: usize) {
        let s = &self.stickers[i];
        self.sticker_anchor_pos = s.transform.pos;
        self.sticker_anchor_scale = s.transform.scale;
        self.sticker_anchor_rot = s.transform.rotation;
    }

    fn on_down(&mut self, p: Pointer) -> Vec<BoardAction> {
        self.track(p);
        if self.locked {
            self.begin_canvas_drag(p);
            return vec![BoardAction::Redraw];
        }
        match self.mode {
            ActionMode::Drag if self.pointers.len() >= 2 && self.handling.is_some() => {
                self.begin_sticker_pinch();
                vec![BoardAction::Redraw]
            }
            ActionMode::CanvasDrag if self.pointers.len() >= 2 => {
                self.begin_canvas_pinch();
                vec![BoardAction::Redraw]
            }
            ActionMode::PinchSticker | ActionMode::CanvasPinch => vec![],
            _ => {
                self.down = (p.x, p.y);
                if let Some(i) = self.hit(p.x, p.y) {
                    let idx = if self.bring_to_front && self.stickers.len() > 1 {
                        let mut s = self.stickers.remove(i);
                        s.canvas = self.canvas;
                        self.stickers.push(s);
                        self.stickers.len() - 1
                    } else {
                        i
                    };
                    self.handling = Some(idx);
                    self.snapshot_sticker(idx);
                    self.mode = ActionMode::Drag;
                    vec![BoardAction::Redraw]
                } else if self.must_lock_to_pan {
                    self.handling = None;
                    self.mode = ActionMode::None;
                    vec![]
                } else {
                    self.handling = None;
                    self.begin_canvas_drag(p);
                    vec![BoardAction::Redraw]
                }
            }
        }
    }

    fn begin_canvas_drag(&mut self, p: Pointer) {
        self.mode = ActionMode::CanvasDrag;
        self.down = (p.x, p.y);
        self.canvas_anchor = self.canvas;
    }

    fn begin_sticker_pinch(&mut self) {
        let (a, b) = (self.pointers[0], self.pointers[1]);
        let (wa, wb) = (self.to_world(a.x, a.y), self.to_world(b.x, b.y));
        self.pinch_dist = dist(wa.0, wa.1, wb.0, wb.1);
        self.pinch_angle = angle_deg(wa.0, wa.1, wb.0, wb.1);
        if let Some(i) = self.handling {
            self.snapshot_sticker(i);
        }
        self.mode = ActionMode::PinchSticker;
    }

    fn begin_canvas_pinch(&mut self) {
        let (a, b) = (self.pointers[0], self.pointers[1]);
        self.pinch_mid = ((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        self.pinch_dist = dist(a.x, a.y, b.x, b.y);
        self.canvas_anchor = self.canvas;
        self.mode = ActionMode::CanvasPinch;
    }

    fn on_move(&mut self, p: Pointer) -> Vec<BoardAction> {
        self.track(p);
        match self.mode {
            ActionMode::Drag => match self.handling {
                Some(i) if i < self.stickers.len() => {
                    let inv = match m_invert(self.canvas.0) {
                        Some(m) => m,
                        None => return vec![],
                    };
                    let (dx, dy) = m_map_vec(inv, p.x - self.down.0, p.y - self.down.1);
                    let s = &mut self.stickers[i];
                    s.transform.pos = (
                        self.sticker_anchor_pos.0 + dx,
                        self.sticker_anchor_pos.1 + dy,
                    );
                    if self.constrained {
                        constrain_to_viewport(s, self.viewport);
                    }
                    vec![BoardAction::StickerMoved(i), BoardAction::Redraw]
                }
                _ => vec![],
            },
            ActionMode::PinchSticker => match self.handling {
                Some(i) if i < self.stickers.len() && self.pointers.len() >= 2 => {
                    let (a, b) = (self.pointers[0], self.pointers[1]);
                    let (wa, wb) = (self.to_world(a.x, a.y), self.to_world(b.x, b.y));
                    let s = &mut self.stickers[i];
                    if self.rotation_enabled {
                        let rot = angle_deg(wa.0, wa.1, wb.0, wb.1);
                        s.transform.rotation = self.sticker_anchor_rot + (rot - self.pinch_angle);
                    } else {
                        let d = dist(wa.0, wa.1, wb.0, wb.1);
                        if self.pinch_dist > f32::EPSILON {
                            let f = d / self.pinch_dist;
                            s.transform.scale = (
                                self.sticker_anchor_scale.0 * f,
                                self.sticker_anchor_scale.1 * f,
                            );
                        }
                    }
                    vec![BoardAction::StickerMoved(i), BoardAction::Redraw]
                }
                _ => vec![],
            },
            ActionMode::CanvasDrag => {
                let m = m_concat(
                    self.canvas_anchor.0,
                    m_translate(p.x - self.down.0, p.y - self.down.1),
                );
                self.canvas = Mat3(m);
                self.push_canvas();
                vec![BoardAction::Redraw]
            }
            ActionMode::CanvasPinch if self.pointers.len() >= 2 => {
                let (a, b) = (self.pointers[0], self.pointers[1]);
                let d = dist(a.x, a.y, b.x, b.y);
                if self.pinch_dist > f32::EPSILON {
                    let f = d / self.pinch_dist;
                    let m = m_concat(
                        self.canvas_anchor.0,
                        m_scale_about(f, self.pinch_mid.0, self.pinch_mid.1),
                    );
                    self.canvas = Mat3(m);
                    self.push_canvas();
                }
                vec![BoardAction::Redraw]
            }
            _ => vec![],
        }
    }

    fn on_up(&mut self, p: Pointer) -> Vec<BoardAction> {
        self.untrack(p.id);
        let mut out = Vec::new();
        match self.mode {
            ActionMode::Drag => {
                if let Some(i) = self.handling {
                    if i < self.stickers.len()
                        && dist(p.x, p.y, self.down.0, self.down.1) < TOUCH_SLOP
                    {
                        out.push(BoardAction::Clicked(i));
                        if self.now_ms - self.last_click_ms < DOUBLE_TAP_MS {
                            out.push(BoardAction::DoubleTapped(i));
                        }
                    } else if i < self.stickers.len() {
                        out.push(BoardAction::StickerMoved(i));
                    }
                }
                self.last_click_ms = self.now_ms;
                self.reanchor_or_none();
            }
            ActionMode::PinchSticker => {
                if let Some(i) = self.handling {
                    if i < self.stickers.len() {
                        out.push(BoardAction::StickerMoved(i));
                    }
                }
                self.last_click_ms = self.now_ms;
                // One finger left -> fall back to a drag anchored on it.
                if self.pointers.is_empty() {
                    self.mode = ActionMode::None;
                } else {
                    self.mode = ActionMode::Drag;
                    let p = self.pointers[0];
                    self.down = (p.x, p.y);
                    if let Some(i) = self.handling {
                        if i < self.stickers.len() {
                            self.snapshot_sticker(i);
                        }
                    }
                }
            }
            ActionMode::CanvasDrag | ActionMode::CanvasPinch => {
                self.last_click_ms = self.now_ms;
                if self.pointers.is_empty() {
                    self.mode = ActionMode::None;
                } else {
                    self.mode = ActionMode::CanvasDrag;
                    self.canvas_anchor = self.canvas;
                }
            }
            _ => {}
        }
        out
    }

    /// After a pointer lifts: no pointers left -> idle; otherwise re-anchor
    /// the in-progress gesture onto the remaining pointer.
    fn reanchor_or_none(&mut self) {
        if self.pointers.is_empty() {
            self.mode = ActionMode::None;
        } else {
            let p = self.pointers[0];
            self.down = (p.x, p.y);
            self.canvas_anchor = self.canvas;
        }
    }

    fn to_world(&self, x: f32, y: f32) -> (f32, f32) {
        match m_invert(self.canvas.0) {
            Some(inv) => m_map(inv, x, y),
            None => (x, y),
        }
    }
}

// --- private 3x3 math (row-major `[a,b,tx, c,d,ty, 0,0,1]`) ---

fn m_translate(x: f32, y: f32) -> [f32; 9] {
    [1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0]
}

fn m_scale(s: f32) -> [f32; 9] {
    [s, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 1.0]
}

fn m_scale_about(s: f32, px: f32, py: f32) -> [f32; 9] {
    [s, 0.0, px * (1.0 - s), 0.0, s, py * (1.0 - s), 0.0, 0.0, 1.0]
}

/// `a * b`: `b` applies to points first.
fn m_concat(a: [f32; 9], b: [f32; 9]) -> [f32; 9] {
    let mut m = [0.0; 9];
    for r in 0..3 {
        for c in 0..3 {
            m[r * 3 + c] =
                a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c];
        }
    }
    m
}

fn m_invert(m: [f32; 9]) -> Option<[f32; 9]> {
    let (a, b, tx, c, d, ty) = (m[0], m[1], m[2], m[3], m[4], m[5]);
    let det = a * d - b * c;
    if det.abs() < f32::EPSILON {
        return None;
    }
    let inv = 1.0 / det;
    Some([
        d * inv,
        -b * inv,
        (b * ty - d * tx) * inv,
        -c * inv,
        a * inv,
        (c * tx - a * ty) * inv,
        0.0,
        0.0,
        1.0,
    ])
}

fn m_map(m: [f32; 9], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
}

/// Linear part only (for deltas, matching the app's mapVectors).
fn m_map_vec(m: [f32; 9], dx: f32, dy: f32) -> (f32, f32) {
    (m[0] * dx + m[1] * dy, m[3] * dx + m[4] * dy)
}

fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt()
}

fn angle_deg(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    (y2 - y1).atan2(x2 - x1).to_degrees()
}

fn rot_about(px: f32, py: f32, cx: f32, cy: f32, deg: f32) -> (f32, f32) {
    let r = deg.to_radians();
    let (dx, dy) = (px - cx, py - cy);
    let (s, c) = (r.sin(), r.cos());
    (cx + dx * c - dy * s, cy + dx * s + dy * c)
}

/// Hit test: forward-map the cropped corners to screen, then point-in-quad.
fn sticker_contains(s: &Sticker, x: f32, y: f32) -> bool {
    let (cx, cy) = (s.width / 2.0, s.height / 2.0);
    let (l, t, r, b) = s.crop;
    let mut q = [0.0f32; 8];
    let corners = [(l, t), (r, t), (r, b), (l, b)];
    for (k, (px, py)) in corners.iter().enumerate() {
        let (mut qx, mut qy) = (
            cx + (px - cx) * s.transform.scale.0,
            cy + (py - cy) * s.transform.scale.1,
        );
        (qx, qy) = rot_about(qx, qy, cx, cy, s.transform.rotation);
        qx += s.transform.pos.0;
        qy += s.transform.pos.1;
        (qx, qy) = m_map(s.canvas.0, qx, qy);
        q[k * 2] = qx;
        q[k * 2 + 1] = qy;
    }
    // Convex quad: point must lie on the same side of every edge.
    let mut sign = 0.0f32;
    for k in 0..4 {
        let ax = q[k * 2];
        let ay = q[k * 2 + 1];
        let bx = q[((k + 1) % 4) * 2];
        let by = q[((k + 1) % 4) * 2 + 1];
        let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        if cross.abs() < f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross;
        } else if sign * cross < 0.0 {
            return false;
        }
    }
    true
}

/// Keep the sticker's mapped center inside the viewport (screen px).
fn constrain_to_viewport(s: &mut Sticker, viewport: (f32, f32)) {
    if viewport.0 <= 0.0 || viewport.1 <= 0.0 {
        return;
    }
    let (cx, cy) = (s.width / 2.0, s.height / 2.0);
    let (mx, my) = m_map(
        s.canvas.0,
        cx + s.transform.pos.0,
        cy + s.transform.pos.1,
    );
    let (mut ox, mut oy) = (0.0f32, 0.0f32);
    if mx < 0.0 {
        ox = -mx;
    } else if mx > viewport.0 {
        ox = viewport.0 - mx;
    }
    if my < 0.0 {
        oy = -my;
    } else if my > viewport.1 {
        oy = viewport.1 - my;
    }
    if ox != 0.0 || oy != 0.0 {
        if let Some(inv) = m_invert(s.canvas.0) {
            let (wx, wy) = m_map_vec(inv, ox, oy);
            s.transform.pos.0 += wx;
            s.transform.pos.1 += wy;
        }
    }
}

/// Crop-edge drag under flips (mirrors the icons lane's gravity mapping).
fn flip_gravity_map(gravity: u32, flip_h: bool, flip_v: bool) -> u32 {
    match (flip_h, flip_v) {
        (true, true) => match gravity {
            0 => 3,
            1 => 2,
            2 => 1,
            3 => 0,
            4 => 5,
            5 => 4,
            6 => 7,
            7 => 6,
            g => g,
        },
        (true, false) => match gravity {
            0 => 1,
            1 => 0,
            2 => 3,
            3 => 2,
            4 => 5,
            5 => 4,
            g => g,
        },
        (false, true) => match gravity {
            0 => 2,
            1 => 3,
            2 => 0,
            3 => 1,
            6 => 7,
            7 => 6,
            g => g,
        },
        (false, false) => gravity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sticker::StickerKind;
    use crate::transform::Transform;

    fn sticker_at(x: f32, y: f32, w: f32, h: f32) -> Sticker {
        Sticker {
            transform: Transform { pos: (x, y), rotation: 0.0, scale: (1.0, 1.0) },
            canvas: Mat3([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]),
            width: w,
            height: h,
            crop: (0.0, 0.0, w, h),
            kind: StickerKind::Text {
                text: String::from("t"),
                size_px: 12.0,
                color: (0, 0, 0),
                background: None,
                bold: false,
            },
        }
    }

    fn tap(board: &mut Board, id: i64, x: f32, y: f32) -> Vec<BoardAction> {
        let mut out = board.handle_event(BoardEvent::Down(Pointer { id, x, y }));
        out.extend(board.handle_event(BoardEvent::Up(Pointer { id, x, y })));
        out
    }

    #[test]
    fn add_selects_and_topmost_wins_hit() {
        let mut b = Board::new();
        // Stickers centered on their pos: put both so (60,60) lies inside each.
        let a = b.add_sticker(sticker_at(10.0, 10.0, 100.0, 100.0));
        assert_eq!(a, 0);
        let c = b.add_sticker(sticker_at(20.0, 20.0, 100.0, 100.0));
        assert_eq!(c, 1);
        assert_eq!(b.handling, Some(1));
        // (60,60) is inside both; topmost (last) wins.
        b.handle_event(BoardEvent::Down(Pointer { id: 1, x: 60.0, y: 60.0 }));
        assert_eq!(b.handling, Some(1));
        b.handle_event(BoardEvent::Up(Pointer { id: 1, x: 60.0, y: 60.0 }));
    }

    #[test]
    fn drag_moves_sticker_in_world_coords() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 100.0, 100.0));
        b.handle_event(BoardEvent::Down(Pointer { id: 1, x: 50.0, y: 50.0 }));
        let acts = b.handle_event(BoardEvent::Move(Pointer { id: 1, x: 70.0, y: 80.0 }));
        assert!(acts.contains(&BoardAction::StickerMoved(0)));
        let acts = b.handle_event(BoardEvent::Up(Pointer { id: 1, x: 70.0, y: 80.0 }));
        assert!(acts.contains(&BoardAction::StickerMoved(0)));
        assert_eq!(b.stickers[0].transform.pos, (20.0, 30.0));
    }

    #[test]
    fn click_single_vs_double_tap() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 100.0, 100.0));
        b.set_now_ms(1000.0);
        let acts = tap(&mut b, 1, 50.0, 50.0);
        assert!(acts.contains(&BoardAction::Clicked(0)));
        assert!(!acts.contains(&BoardAction::DoubleTapped(0)));
        // Second tap inside the window -> double.
        b.set_now_ms(1100.0);
        let acts = tap(&mut b, 1, 50.0, 50.0);
        assert!(acts.contains(&BoardAction::Clicked(0)));
        assert!(acts.contains(&BoardAction::DoubleTapped(0)));
        // After the window -> single again.
        b.set_now_ms(2000.0);
        let acts = tap(&mut b, 1, 50.0, 50.0);
        assert!(acts.contains(&BoardAction::Clicked(0)));
        assert!(!acts.contains(&BoardAction::DoubleTapped(0)));
    }

    #[test]
    fn canvas_drag_pans_and_pushes_to_stickers() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 100.0, 100.0));
        // Miss the sticker -> canvas drag.
        b.handle_event(BoardEvent::Down(Pointer { id: 1, x: 400.0, y: 400.0 }));
        assert_eq!(b.handling, None);
        b.handle_event(BoardEvent::Move(Pointer { id: 1, x: 430.0, y: 410.0 }));
        assert_eq!((b.canvas.0[2], b.canvas.0[5]), (30.0, 10.0));
        assert_eq!(b.stickers[0].canvas, b.canvas);
        b.handle_event(BoardEvent::Up(Pointer { id: 1, x: 430.0, y: 410.0 }));
        // Stickers still follow an explicit matrix set.
        let m = Mat3(m_translate(5.0, 5.0));
        b.set_canvas_matrix(m);
        assert_eq!(b.canvas_matrix(), m);
        assert_eq!(b.stickers[0].canvas, m);
    }

    #[test]
    fn remove_all_clears() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 10.0, 10.0));
        b.add_sticker(sticker_at(0.0, 0.0, 10.0, 10.0));
        b.remove_all();
        assert!(b.stickers().is_empty());
        assert_eq!(b.handling, None);
    }

    #[test]
    fn remove_handling_removes_selected() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 10.0, 10.0));
        b.add_sticker(sticker_at(50.0, 50.0, 10.0, 10.0));
        assert_eq!(b.handling, Some(1));
        assert_eq!(b.remove_handling(), Some(1));
        assert_eq!(b.stickers.len(), 1);
        assert_eq!(b.handling, None);
        assert_eq!(b.remove_handling(), None);
    }

    #[test]
    fn two_finger_pinch_scales_sticker() {
        let mut b = Board::new();
        b.add_sticker(sticker_at(0.0, 0.0, 100.0, 100.0));
        b.handle_event(BoardEvent::Down(Pointer { id: 1, x: 50.0, y: 50.0 }));
        b.handle_event(BoardEvent::Down(Pointer { id: 2, x: 90.0, y: 50.0 }));
        // Spread the fingers: world distance doubles -> scale doubles.
        let acts = b.handle_event(BoardEvent::Move(Pointer { id: 2, x: 130.0, y: 50.0 }));
        assert!(acts.contains(&BoardAction::StickerMoved(0)));
        let (sx, sy) = b.stickers[0].transform.scale;
        assert!((sx - 2.0).abs() < 1e-4 && (sy - 2.0).abs() < 1e-4);
        b.handle_event(BoardEvent::Up(Pointer { id: 2, x: 130.0, y: 50.0 }));
        b.handle_event(BoardEvent::Up(Pointer { id: 1, x: 50.0, y: 50.0 }));
    }
}
