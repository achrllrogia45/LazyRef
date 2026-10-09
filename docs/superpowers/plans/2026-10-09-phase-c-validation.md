# Phase C Plan Validation (2026-10-09)

**Verdict: CHANGES-NEEDED** — 5 fixes, no signature mismatches on board-side APIs. Two time-boxed items checked: (a) `screen_rect` confirmed missing; (b) `egui::Panel` export confirmed present (plan is correct, no fix).

## 1. Signature verification (all PASS unless noted)

**Board** (`crates/board/src/board.rs`): `new` :88, `add_sticker(&mut self, mut s: Sticker)->usize` :115, `remove_all` :123, `duplicate_handling()->Option<usize>` :165, `flip_handling(&mut self, horizontal: bool, vertical: bool)` :200, `reset_handling` :215, `fit(bounds:(f32,f32,f32,f32), viewport:(f32,f32), margin:f32)` :286, `handle_event(ev: BoardEvent)->Vec<BoardAction>` :156, `set_now_ms(ms:f64)` :145, `set_viewport(w:f32,h:f32)` :150, `canvas_matrix()->Mat3` :134, `set_canvas_matrix(m:Mat3)` :139, fields `stickers: Vec<Sticker>` :59, `handling: Option<usize>` :60, `canvas: Mat3` :61, `locked: bool` :63, `bring_to_front: bool` :71 — all match. `remove_handling` absent (grep empty) as expected for Task 1 new code; proposed body only touches real private fields `mode: ActionMode` :72, `pointers: Vec<Pointer>` :73, `ActionMode::None` :24 — valid. Accessor `stickers()->&[Sticker]` :130 exists, so both `board.stickers()` and `board.stickers.get_mut(i)` compile.

**BoardEvent/Pointer** (:33-45): `Pointer{id:i64,x:f32,y:f32}`, `BoardEvent::{Down,Move,Up}(Pointer)` — match.

**Sticker** (`sticker.rs`): kind enum :16-26 exact, struct fields :28-37 exact, `new(mut kind: StickerKind)->Self` :43, `bound_points()->[f32;8]` :70, `final_matrix()->Mat3` :80, `mapped_points(&[f32])->Vec<f32>` :84 — match. `Transform{pos:(f32,f32),rotation:f32,scale:(f32,f32)}` (`transform.rs`:10-14) — match.

**Mat3** (`math.rs`): `Mat3(pub [f32;9])` :8, `identity` :11, `translate(x,y)` :15, `concat(self,other: Mat3)->Mat3` :30, `invert(self)->Option<Mat3>` :41, `map_point(self,x,y)->(f32,f32)` :69 — match. `Copy` so `canvas.invert()`, `canvas.map_point`, `canvas.0[0]` all legal.

**Settings** (`settings.rs`): struct :8-13, `defaults` :17, `load(&Path)->Self` :26, `save(&Path)->std::io::Result<()>` :61, `max_options()->[u32;4]` :72 = `[1024*1024, 2048*2048, 3072*3072, 4096*4096]` = `[1048576,4194304,9437184,16777216]` — match.

**io** (`io.rs`): `FileFormat::{Ref,Pur}` :33-37, `detect(&[u8])->FileFormat` :42, `open_ref(&[u8])->Result<Board,String>` :67, `save_ref(&Board)->Vec<u8>` :128, `open_pur(&[u8],&Path)->Result<Board,String>` :199, `save_pur(&Board,&Path,&[u8])->Vec<u8>` :453 — match.

**icons** (`icons.rs`): `IconOp::{Delete,ScaleStep(f32),RotateLeft,RotateRight,FlipH,FlipV,FlipBoth}` :18-26, `apply_op(&mut Transform,&IconOp)` :31 — match.

**egui/eframe 0.36** (vendored): `App::ui(&mut self, &mut egui::Ui, &mut Frame)` (`eframe/epi.rs`:152 + SimpleApp impl) ✓. `Panel::top` (`panel.rs`:265) + `Panel::show(ui,|ui|)` (:422) ✓. `CentralPanel::show` (:1212, verified text at :1211-1214) ✓. `Response::{clicked :184, drag_started :393, dragged :433, drag_stopped :445, interact_pointer_pos :546, hover_pos :573}` ✓. `InputState.time: f64` (`input_state/mod.rs`:271) + `key_pressed(Key)->bool` (:746) ✓. `Key::{F (:127), Delete (:22), O (:136)}` in `data/key.rs` ✓. `Align2` via emath re-export (`lib.rs`:443-444) ✓. `FontId::default()` (`epaint/fonts.rs`:30, size 14.0/Proportional) ✓. `CornerRadius::ZERO` (`epaint/corner_radius.rs`:50) ✓. `Shape::convex_polygon` (`epaint/shapes/shape.rs`:251) ✓. `Painter::{line_segment :318, rect_filled :397, text :469}` ✓. `Ui::{ctx :452, allocate_painter :1371, label :1696, text_edit_singleline :1802, button :1848, checkbox :1866, radio :1888, horizontal :2315}` ✓. `Window::open(&mut bool)` (`window.rs`:173) + `show(&Context,…)` (:543) ✓. `Response.rect: Rect` (response.rs:37), `Rect::width/height`, `Context::input` (context.rs:991) ✓.

**Imports/paths**: `lazyref::egui` via `pub use eframe::egui` (app `lib.rs`:4) ✓; `lazyref_board::{board,math,settings,sticker,io,icons}` all `pub mod` (board `lib.rs`:8-16) ✓; `lazyref-board = { path = "../board" }` correct relative to `crates/app/` ✓.

## 2. Fail-then-pass analysis

- Task 1 test: true red (no `remove_handling` in tree), green after. Helper `sticker_at` exists (board.rs test mod :743), `handling==Some(1)` follows from `add_sticker` semantics. PASS.
- Task 2 test: true red (current `BoardApp` has `camera/stickers`, `content_bounds()->Rect`; no `board` field). Expected math `(10,20,110,70)` verified against `add_sticker` (canvas identity) + translate-only `final_matrix`. PASS.
- Task 3 test: **NOT red** — at Task 3 time both `BoardApp` and `remove_handling` already exist, so the test passes on first run. It is a characterization test, not fail-then-pass. See fix 3.

## 3. Defects (CHANGES-NEEDED)

1. **`ui.ctx().screen_rect()` does not exist.** `grep 'pub fn screen_rect' egui-0.36.2/src/` = empty. Use `ui.ctx().viewport_rect()` (context.rs:2921) — or better, the central `resp.rect` size, since `viewport_rect` includes the toolbar just added. Fix Task 3 Fit button.
2. **Time-unit bug: `set_now_ms(ui.ctx().input(|i| i.time))`.** `i.time` is seconds (f64); `Board::set_now_ms` + `DOUBLE_TAP_MS = 200.0` are milliseconds (board.rs:20,145). Feeding seconds makes the double-tap window 200 s. Fix: `i.time * 1000.0` in Task 2 pointer bridge.
3. **Task 3 Step 2 expectation is wrong.** Change `Expected: FAIL` to `Expected: PASS on first run (characterization test)` or rewrite the test to gate on actual new toolbar behavior.
4. **Test-count expectations are stale.** Actual: board 62 unit (`grep -c #[test]` per file: 6+7+6+8+7+6+7+10+5) + 8 integration (`board/tests/integration.rs`) = 70; app 7 (camera.rs) + 2 (main.rs) + 2 (`app/tests/camera.rs`) = 11. Fix "62+ prior" and "board 62+1, app 7+2+2, purcore 10" lines.
5. **CentralPanel inconsistency.** Interfaces list `CentralPanel::default().show`, but Task 2 central code calls `ui.allocate_painter` on the root `Ui` directly and never uses `CentralPanel`. Either wrap the canvas in `CentralPanel` or drop it from the Interfaces list.

## 4. Non-defects confirmed

- (b) `egui::Panel::top` is **correct**: `Panel` is exported from the egui root (`lib.rs`:654 in the containers re-export), `Panel::top` at `panel.rs`:265. There is no `TopBottomPanel` in 0.36. No fix needed.
- No Global Constraint violations: workspace members/crate names match root `Cargo.toml`; only added dep is workspace-internal `lazyref-board`; lanes-never-commit honored (`Do NOT commit` steps); no `DroidRef-dev/` changes; egui-only, no XML; no internal paths in code; quads-before-textures (`Shape::convex_polygon`, no texture API).
