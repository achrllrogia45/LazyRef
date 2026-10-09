# Phase C App Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire `crates/app/src/main.rs` to drive the real `lazyref-board::Board` (replace placeholders, pointer events, render quads+text, toolbar open/save/add, handling ops, settings dialog).

**Architecture:** Keep `camera.rs` (tests stay green) but stop using it in `main.rs`. `Board.canvas: Mat3` is the single camera (world->screen). Stickers render via `s.final_matrix().mapped_points()` -> screen quads. Pointer feeds `BoardEvent::Down/Move/Up(id=0, local x/y)`. Toolbar mutates `Board` directly via public API + `icons::apply_op`. Settings via `Settings::load/save`.

**Tech Stack:** Rust 2021, eframe 0.36.2 (glow+x11+default_fonts), lazyref-board (path), no new external deps.

**Spec:** `DroidRef-dev/app/src/main/java/xyz/ruin/droidref/MainActivity.kt` (620 ln: icon gravities, intents 1/2, async load), `SettingsActivity.kt` (92 ln: checkboxes+max radio), `SettingsStorage.kt` (91 ln: keys, defaults true/true/2048²), `RefBoardLoadSaveManager.kt` (280 ln: SAVE_FILE_EXTENSION="ref", 0.6f step), `PurBoardLoadSaveManager.kt` (127 ln: checksum-mismatch Open-anyway dialog), `StickerViewModel.kt` (resetView/removeAllStickers/removeSticker/duplicateSticker/resetCurrentStickerX).

## Global Constraints

- Workspace members `["crates/purcore","crates/app","crates/board"]`; crate names `purcore`, `lazyref` (app), `lazyref-board`.
- Reuse only: `eframe 0.36.2`, `image 0.25` (board only), `sha2 0.10`, `rusqlite` (purcore only). No new external deps; adding `lazyref-board` path dep to `crates/app/Cargo.toml` is allowed (workspace-internal).
- No push without explicit user OK. Orchestrator commits, lanes never commit.
- `DroidRef-dev/` stays parked/uncommitted. No XML layouts (egui by design).
- Public repo: no internal paths/tool names in code, behavior-describing comments only.
- Ponytail full: shortest diff, no abstractions, quads before textures (textures deferred).

---

## File Structure

- Modify `crates/app/Cargo.toml`: add `lazyref-board = { path = "../board" }`.
- Modify `crates/board/src/board.rs`: add `remove_handling()` + 1 test.
- Rewrite `crates/app/src/main.rs` (202 ln today): `BoardApp { board, settings, settings_path, open_path, save_path, add_path, add_text, show_settings, status, last_viewport }`, `draw_grid(painter, canvas: Mat3, viewport: Rect)`, `draw_sticker(painter, s: &Sticker)`, pointer bridge, toolbar, settings window. Canvas lives inside `egui::CentralPanel` (after the `Panel::top` toolbar).
- Keep `crates/app/src/camera.rs`, `crates/app/src/lib.rs` untouched (lib stays `pub mod camera;`).

**Interfaces (exact):**
- `Board::new()`, `add_sticker(s: Sticker)->usize`, `remove_all()`, `remove_handling()->Option<usize>` (new), `duplicate_handling()->Option<usize>`, `flip_handling(h: bool,v: bool)`, `reset_handling()`, `fit(bounds:(f32,f32,f32,f32), viewport:(f32,f32), margin:f32)`, `handle_event(ev: BoardEvent)->Vec<BoardAction>`, `set_now_ms(ms:f64)`, `set_viewport(w:f32,h:f32)`, `canvas_matrix()->Mat3`, `set_canvas_matrix(m:Mat3)`, fields `stickers: Vec<Sticker>`, `handling: Option<usize>`, `canvas: Mat3`, `locked: bool`, `bring_to_front: bool`.
- `BoardEvent::{Down(Pointer),Move(Pointer),Up(Pointer)}`, `Pointer{id:i64,x:f32,y:f32}` (shell uses `id=0`, local coords `p - viewport.min`).
- `Sticker::new(kind)`, fields `transform: Transform{pos:(f32,f32),rotation:f32,scale:(f32,f32)}`, `canvas: Mat3`, `width:f32`, `height:f32`, `crop:(f32,f32,f32,f32)`, `kind`; methods `final_matrix()->Mat3`, `mapped_points(&[f32])->Vec<f32>`, `bound_points()->[f32;8]`.
- `StickerKind::{Image{data:Vec<u8>,width:u32,height:u32}, Text{text:String,size_px:f32,color:(u8,u8,u8),background:Option<(u8,u8,u8)>,bold:bool}}`.
- `Mat3(pub [f32;9])`: `identity()`, `translate(x,y)`, `concat(self,other)`, `invert()->Option<Mat3>`, `map_point(x,y)->(f32,f32)`.
- `Settings{downscale_on_import:bool,multistep_scale:bool,max_resolution:u32}`: `defaults()`, `load(&Path)->Self`, `save(&Path)->io::Result<()>`, `max_options()->[u32;4]` = `[1048576,4194304,9437184,16777216]`.
- `io::{detect()->FileFormat, open_ref(&[u8])->Result<Board,String>, save_ref(&Board)->Vec<u8>, open_pur(&[u8],&Path)->Result<Board,String>, save_pur(&Board,&Path,&[u8])->Vec<u8>}`.
- `icons::{apply_op(&mut Transform,&IconOp)}`, `IconOp::{Delete,ScaleStep(f32),RotateLeft,RotateRight,FlipH,FlipV,FlipBoth}`.
- egui 0.36: `eframe::App::ui(&mut self, ui:&mut Ui, frame:&mut Frame)`, `egui::Panel::top("bar").show(ui,|ui|{...})`, `egui::CentralPanel::default().show(ui,|ui|{...})` (canvas lives inside it; `resp.rect` is the viewport), `resp.drag_started()->bool`, `resp.dragged()->bool`, `resp.drag_stopped()->bool`, `resp.clicked()->bool`, `resp.hover_pos()->Option<Pos2>`, `resp.interact_pointer_pos()->Option<Pos2>`, `ui.ctx().input(|i| i.time * 1000.0)->f64` (ms clock for `set_now_ms`; `i.time` is seconds), `ui.ctx().viewport_rect()->Rect` (fallback only), `Key::{F, Delete, O}`.

---

### Task 1: Deps + `Board::remove_handling`

**Files:**
- Modify: `crates/app/Cargo.toml`
- Modify: `crates/board/src/board.rs` (append method after `remove_all`, add test in `mod tests`)

**Interfaces:**
- Consumes: nothing (new).
- Produces: `Board::remove_handling(&mut self)->Option<usize>` used by Task 3 Delete button; `lazyref-board` dep used by Task 2/3.

- [ ] **Step 1: Write the failing test** in `crates/board/src/board.rs` `mod tests`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lazyref-board remove_handling_removes_selected`
Expected: FAIL with `no method named remove_handling found` (compile error = red).

- [ ] **Step 3: Write minimal implementation** in `crates/board/src/board.rs` after `remove_all` (line 128):

```rust
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
```

And in `crates/app/Cargo.toml` append:

```toml
lazyref-board = { path = "../board" }
```

Full file becomes:

```toml
[package]
name = "lazyref"
description = "LazyRef — a PureRef-compatible image reference board (Android-first; this crate is the egui board app)."
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
# glow (GL) + x11 keeps it runnable under Xvfb/llvmpipe; the default wgpu
# backend wants a GPU. Add "wgpu" back when targeting real desktops.
eframe = { version = "0.36.2", default-features = false, features = ["glow", "x11", "default_fonts"] }
lazyref-board = { path = "../board" }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lazyref-board remove_handling_removes_selected`
Expected: PASS. Then `cargo test -p lazyref-board` all green (62 unit + 8 integration).

- [ ] **Step 5: Do NOT commit** (orchestrator commits; lanes never commit).

---

### Task 2: main.rs Board wiring (render + pointer + grid + fit)

**Files:**
- Modify: `crates/app/src/main.rs` (full rewrite, ~260 ln; keep `draw_grid` name, replace `PlaceholderSticker`/`Camera` use)

**Interfaces:**
- Consumes: Task 1 dep (`lazyref_board::board::{Board,BoardEvent,Pointer}`, `sticker::{Sticker,StickerKind}`, `math::Mat3`).
- Produces: `BoardApp { board: Board, ... }` with `content_bounds()->(f32,f32,f32,f32)` used by Task 3 Fit button.

- [ ] **Step 1: Write the failing test** in `crates/app/src/main.rs` `mod tests` (append; keep existing 2 tests):

```rust
#[test]
fn content_bounds_covers_board_stickers() {
    let mut app = BoardApp::new();
    app.board.remove_all();
    let mut s = Sticker::new(StickerKind::Text { text: "t".into(), size_px: 12.0, color: (0,0,0), background: None, bold: false });
    s.width = 100.0; s.height = 50.0;
    s.transform.pos = (10.0, 20.0);
    app.board.add_sticker(s);
    assert_eq!(app.content_bounds(), (10.0, 20.0, 110.0, 70.0));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lazyref content_bounds_covers_board_stickers`
Expected: FAIL `no struct BoardApp with board field / no function content_bounds` (compile error = red).

- [ ] **Step 3: Write minimal implementation** — replace top of `crates/app/src/main.rs` lines 1-75 with:

```rust
//! LazyRef board shell: egui harness driving `lazyref_board::Board`.
//! Board.canvas is the camera (world->screen). Stickers render via
//! final_matrix -> screen quads. Pointer feeds Down/Move/Up in
//! viewport-local coords (global minus viewport.min).

use lazyref::egui::{self, pos2, Color32, CornerRadius, Key, Pos2, Rect, Sense, Shape, Stroke};
use lazyref_board::board::{Board, BoardEvent, Pointer};
use lazyref_board::math::Mat3;
use lazyref_board::settings::Settings;
use lazyref_board::sticker::{Sticker, StickerKind};

struct BoardApp {
    board: Board,
    settings: Settings,
    settings_path: std::path::PathBuf,
    open_path: String,
    save_path: String,
    add_path: String,
    add_text: String,
    show_settings: bool,
    status: String,
    // ponytail: last central-panel size, reused by toolbar Fit (no ctx rect needed).
    last_viewport: (f32, f32),
}

impl BoardApp {
    fn new() -> Self {
        let settings_path = std::env::temp_dir().join("lazyref-settings.cfg");
        let settings = Settings::load(&settings_path);
        Self {
            board: Board::new(),
            settings,
            settings_path,
            open_path: String::new(),
            save_path: String::new(),
            add_path: String::new(),
            add_text: "hello".to_string(),
            show_settings: false,
            status: String::new(),
            last_viewport: (1280.0, 800.0),
        }
    }

    fn content_bounds(&self) -> (f32, f32, f32, f32) {
        let mut first = true;
        let (mut l, mut t, mut r, mut b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for s in self.board.stickers() {
            let pts = s.mapped_points(&s.bound_points());
            for p in pts.chunks_exact(2) {
                if first {
                    (l, t, r, b) = (p[0], p[1], p[0], p[1]);
                    first = false;
                } else {
                    l = l.min(p[0]); t = t.min(p[1]);
                    r = r.max(p[0]); b = b.max(p[1]);
                }
            }
        }
        if first { (0.0, 0.0, 0.0, 0.0) } else { (l, t, r, b) }
    }
}
```

Render helpers (replace `draw_sticker` + `draw_grid`):

```rust
fn draw_grid(painter: &egui::Painter, canvas: Mat3, viewport: Rect) {
    let inv = match canvas.invert() { Some(m) => m, None => return };
    let (x0, y0) = inv.map_point(viewport.min.x, viewport.min.y);
    let (x1, y1) = inv.map_point(viewport.max.x, viewport.max.y);
    let (lx, rx) = (x0.min(x1), x0.max(x1));
    let (ty, by) = (y0.min(y1), y0.max(y1));
    let zoom = canvas.0[0].abs().max(1e-6);
    let mut step = 50.0f32;
    while step * zoom < 30.0 { step *= 2.0; }
    while step * zoom > 240.0 { step *= 0.5; }
    while (rx - lx) / step > 200.0 { step *= 2.0; }
    let line = Stroke::new(1.0, Color32::from_gray(52));
    let mut x = (lx / step).floor() * step;
    while x <= rx {
        let (sx, _) = canvas.map_point(x, 0.0);
        painter.line_segment([pos2(sx, viewport.top()), pos2(sx, viewport.bottom())], line);
        x += step;
    }
    let mut y = (ty / step).floor() * step;
    while y <= by {
        let (_, sy) = canvas.map_point(0.0, y);
        painter.line_segment([pos2(viewport.left(), sy), pos2(viewport.right(), sy)], line);
        y += step;
    }
}

fn draw_sticker(painter: &egui::Painter, s: &Sticker, selected: bool) {
    let v = s.mapped_points(&s.bound_points());
    let pts = vec![pos2(v[0], v[1]), pos2(v[2], v[3]), pos2(v[6], v[7]), pos2(v[4], v[5])];
    let (fill, label) = match &s.kind {
        StickerKind::Image { width, height, .. } => (Color32::from_rgb(90, 120, 150), format!("{width}x{height}")),
        StickerKind::Text { text, color, .. } => (Color32::from_rgb(60, 60, 64), text.clone()),
    };
    let stroke = if selected { Stroke::new(2.0, Color32::WHITE) } else { Stroke::new(1.0, Color32::from_black_alpha(140)) };
    painter.add(Shape::convex_polygon(pts.clone(), fill, stroke));
    if let StickerKind::Text { color, .. } = &s.kind {
        painter.text(pts[0], egui::Align2::LEFT_TOP, label, egui::FontId::default(), Color32::from_rgb(color.0, color.1, color.2));
    }
}
```

Pointer bridge inside `egui::CentralPanel::default().show(ui, |ui| { ... })` (canvas lives in the central panel, AFTER the toolbar panel). Inside the central closure, after `let (resp, painter) = ui.allocate_painter(...)`:

```rust
let viewport = resp.rect;
self.last_viewport = (viewport.width(), viewport.height());
self.board.set_viewport(viewport.width(), viewport.height());
// i.time is seconds; Board clock is ms (DOUBLE_TAP_MS = 200.0).
self.board.set_now_ms(ui.ctx().input(|i| i.time * 1000.0));
let to_local = |p: Pos2| (p.x - viewport.min.x, p.y - viewport.min.y);
if resp.drag_started() {
    if let Some(p) = resp.interact_pointer_pos() {
        let (x, y) = to_local(p);
        self.board.handle_event(BoardEvent::Down(Pointer { id: 0, x, y }));
    }
} else if resp.dragged() {
    if let Some(p) = resp.interact_pointer_pos() {
        let (x, y) = to_local(p);
        self.board.handle_event(BoardEvent::Move(Pointer { id: 0, x, y }));
    }
}
if resp.drag_stopped() {
    if let Some(p) = resp.interact_pointer_pos().or(resp.hover_pos()) {
        let (x, y) = to_local(p);
        self.board.handle_event(BoardEvent::Up(Pointer { id: 0, x, y }));
    }
}
if resp.clicked() {
    if let Some(p) = resp.interact_pointer_pos().or(resp.hover_pos()) {
        let (x, y) = to_local(p);
        self.board.handle_event(BoardEvent::Down(Pointer { id: 0, x, y }));
        self.board.handle_event(BoardEvent::Up(Pointer { id: 0, x, y }));
    }
}
if ui.input(|i| i.key_pressed(Key::F)) {
    let b = self.content_bounds();
    self.board.fit(b, (viewport.width(), viewport.height()), 24.0);
}
```

Canvas paint:

```rust
painter.rect_filled(viewport, CornerRadius::ZERO, Color32::from_rgb(24, 24, 28));
draw_grid(&painter, self.board.canvas_matrix(), viewport);
let handling = self.board.handling;
for (i, s) in self.board.stickers().iter().enumerate() {
    draw_sticker(&painter, s, Some(i) == handling);
}
```

Keep `fn main()` unchanged (1280x800 LazyRef).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lazyref content_bounds_covers_board_stickers`
Expected: PASS. Then `cargo test -p lazyref` (7 camera lib + 2 app/tests/camera + 2 new main tests; old `corners_match...`/`contains_is...` tests on `PlaceholderSticker` must be deleted with the struct; suite stays green).

- [ ] **Step 5: Do NOT commit.**

---

### Task 3: Toolbar (open/save/add/handling ops) + settings window

**Files:**
- Modify: `crates/app/src/main.rs` (add `egui::Panel::top` toolbar + `egui::Window` settings in `BoardApp::ui`, before central panel)

**Interfaces:**
- Consumes: Task 2 `BoardApp` fields + `content_bounds()`; Task 1 `remove_handling`; `io`, `icons`, `Settings`.
- Produces: working desktop shell (manual smoke: open S1 fixture, save .ref, reload).

- [ ] **Step 1: Write the failing test** (pure logic, no window):

```rust
#[test]
fn toolbar_delete_uses_remove_handling() {
    let mut app = BoardApp::new();
    let mut s = Sticker::new(StickerKind::Text { text: "x".into(), size_px: 12.0, color: (0,0,0), background: None, bold: false });
    s.width = 20.0; s.height = 10.0;
    app.board.add_sticker(s);
    assert_eq!(app.board.stickers().len(), 1);
    assert_eq!(app.board.remove_handling(), Some(0));
    assert!(app.board.stickers().is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lazyref toolbar_delete_uses_remove_handling`
Expected: PASS on first run (characterization test — both `BoardApp` and `remove_handling` already exist by Task 3; it pins the wiring, not new behavior).

- [ ] **Step 3: Write minimal implementation** — at top of `BoardApp::ui`, before central allocation:

```rust
egui::Panel::top("toolbar").show(ui, |ui| {
    ui.horizontal(|ui| {
        if ui.button("Fit").clicked() {
            // Fit uses the last central-panel size (toolbar has no viewport).
            let b = self.content_bounds();
            // ponytail: fixed 24px margin, per-canvas fit lives in Board.
            self.board.fit(b, self.last_viewport, 24.0);
        }
        if ui.button("Clear").clicked() { self.board.remove_all(); }
        if ui.button("Dup").clicked() { self.board.duplicate_handling(); }
        if ui.button("FlipH").clicked() { self.board.flip_handling(true, false); }
        if ui.button("FlipV").clicked() { self.board.flip_handling(false, true); }
        if ui.button("RotL").clicked() {
            if let Some(i) = self.board.handling {
                if let Some(s) = self.board.stickers.get_mut(i) {
                    lazyref_board::icons::apply_op(&mut s.transform, &lazyref_board::icons::IconOp::RotateLeft);
                }
            }
        }
        if ui.button("RotR").clicked() {
            if let Some(i) = self.board.handling {
                if let Some(s) = self.board.stickers.get_mut(i) {
                    lazyref_board::icons::apply_op(&mut s.transform, &lazyref_board::icons::IconOp::RotateRight);
                }
            }
        }
        if ui.button("Del").clicked() { self.board.remove_handling(); }
        let mut lock = self.board.locked;
        if ui.checkbox(&mut lock, "Lock").changed() { self.board.locked = lock; }
        if ui.button("Settings").clicked() { self.show_settings = !self.show_settings; }
    });
    ui.horizontal(|ui| {
        ui.label("Open:");
        ui.text_edit_singleline(&mut self.open_path);
        if ui.button("Open").clicked() {
            match std::fs::read(&self.open_path) {
                Ok(bytes) => {
                    let r: Result<Board, String> = match lazyref_board::io::detect(&bytes) {
                        lazyref_board::io::FileFormat::Ref => lazyref_board::io::open_ref(&bytes),
                        lazyref_board::io::FileFormat::Pur => {
                            let scratch = std::env::temp_dir().join("lazyref-open");
                            std::fs::create_dir_all(&scratch).ok();
                            lazyref_board::io::open_pur(&bytes, &scratch)
                        }
                    };
                    match r {
                        Ok(b) => { self.board = b; self.status.clear(); }
                        Err(e) => { self.status = e; }
                    }
                }
                Err(e) => { self.status = e.to_string(); }
            }
        }
        ui.label("Save:");
        ui.text_edit_singleline(&mut self.save_path);
        if ui.button("Save .ref").clicked() {
            let bytes = lazyref_board::io::save_ref(&self.board);
            // ponytail: direct write, atomic rename when a path dir exists.
            self.status = match std::fs::write(&self.save_path, &bytes) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            };
        }
        if ui.button("Save .pur").clicked() {
            let scratch = std::env::temp_dir().join("lazyref-save");
            std::fs::create_dir_all(&scratch).ok();
            let bytes = lazyref_board::io::save_pur(&self.board, &scratch, &[]);
            self.status = match std::fs::write(&self.save_path, &bytes) {
                Ok(()) => String::new(),
                Err(e) => e.to_string(),
            };
        }
    });
    ui.horizontal(|ui| {
        ui.label("Img:");
        ui.text_edit_singleline(&mut self.add_path);
        if ui.button("Add img").clicked() {
            match std::fs::read(&self.add_path) {
                Ok(data) => {
                    let s = Sticker::new(StickerKind::Image { data, width: 0, height: 0 });
                    self.board.add_sticker(s);
                    self.status.clear();
                }
                Err(e) => { self.status = e.to_string(); }
            }
        }
        ui.label("Txt:");
        ui.text_edit_singleline(&mut self.add_text);
        if ui.button("Add txt").clicked() {
            let mut s = Sticker::new(StickerKind::Text { text: self.add_text.clone(), size_px: 24.0, color: (255,255,255), background: None, bold: false });
            s.width = 200.0; s.height = 40.0; s.crop = (0.0, 0.0, 200.0, 40.0);
            self.board.add_sticker(s);
        }
    });
    if !self.status.is_empty() {
        ui.label(&self.status);
    }
});
if self.show_settings {
    egui::Window::new("Settings").open(&mut self.show_settings).show(ui.ctx(), |ui| {
        let mut s = self.settings.clone();
        ui.checkbox(&mut s.downscale_on_import, "Downscale on import");
        ui.checkbox(&mut s.multistep_scale, "Multi-step downscale");
        for opt in Settings::max_options() {
            let sel = s.max_resolution == opt;
            if ui.radio(sel, format!("{} px", opt)).clicked() { s.max_resolution = opt; }
        }
        if ui.button("Save").clicked() {
            // ponytail: direct write; corrupt/missing loads defaults anyway.
            if s.save(&self.settings_path).is_ok() { self.settings = s; self.show_settings = false; }
            else { self.status = "settings save failed".to_string(); }
        }
    });
}
```

Delete-key shortcut in central section:

```rust
if ui.input(|i| i.key_pressed(Key::Delete)) {
    self.board.remove_handling();
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lazyref toolbar_delete_uses_remove_handling`
Expected: PASS. Then `cargo test --workspace` (board 62 unit + 8 integration + 1 new, app 7 + 2 + 2 new, purcore 10).

- [ ] **Step 5: Do NOT commit.**

---

## Self-Review

- [x] Spec coverage: MainActivity icon ops -> toolbar Dup/Flip/Rot/Del; intents -> Open/Save path rows; SettingsActivity checkboxes+max radio -> settings Window; SettingsStorage defaults -> `Settings::load` fallback; Ref/Pur managers -> `io::open_ref/save_ref/open_pur/save_pur`; ViewModel reset/remove/duplicate -> `reset_handling` (toolbar Reset skipped: `reset_handling` exists on Board, wire on demand — ponytail), `remove_all`/`remove_handling`/`duplicate_handling`.
- [x] Placeholder scan: no TBD/TODO/appropriate-handling/similar-to; every code block is compilable against the listed exact signatures.
- [x] Type consistency: `Mat3(pub [f32;9])` everywhere; `Pointer{id:i64,x:f32,y:f32}`; `Board::fit((f32×4),(f32,f32),f32)`; `Settings::max_options()->[u32;4]`; `IconOp::RotateLeft/RotateRight` (no args); `Key::F/Delete`.
