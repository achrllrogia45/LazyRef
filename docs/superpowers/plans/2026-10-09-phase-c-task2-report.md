# Phase C Task 2 Report — main.rs Board wiring

Date: 2026-10-09 (WIB). Lane scope: Task 2 only (no Task 3 toolbar/settings).

## TDD
- Appended `content_bounds_covers_board_stickers` to `crates/app/src/main.rs` mod tests.
- RED: `cargo test -p lazyref content_bounds_covers_board_stickers` failed to compile
  (no `board` field, `Rect` vs `(f32,f32,f32,f32)` tuple, no `StickerKind`) — expected red.
- GREEN after rewrite: 1 passed.

## Implementation
- Rewrote `crates/app/src/main.rs` (202 → ~200 ln) per plan Task 2 Step 3:
  `BoardApp { board, settings, settings_path, open_path, save_path, add_path,
  add_text, show_settings, status, last_viewport }`, `content_bounds()`,
  `draw_grid(painter, canvas: Mat3, viewport)`, `draw_sticker(painter, s, selected)`,
  `CentralPanel` pointer bridge (Down/Move/Up id=0, local coords, `time*1000.0`),
  F-fit, Delete-`remove_handling`, canvas paint.
- Deleted `PlaceholderSticker` struct + its 2 tests (`corners_match...`, `contains_is...`).
- `fn main` unchanged; `camera.rs`/`lib.rs` untouched.
- One deviation from plan-verbatim: `StickerKind::Text { text, .. }` instead of
  `{ text, color, .. }` in `draw_sticker` fill match — the plan's `color` binding
  was unused (warning); color is read in the text-paint arm below.
- Dead-code warning remains on Task 3 staging fields
  (`settings`, paths, `show_settings`, `status` — wired in Task 3). Kept per plan.

## Verification
- `cargo test -p lazyref content_bounds_covers_board_stickers` → 1 passed.
- `cargo test -p lazyref` → 7 lib (camera) + 1 bin (content_bounds) + 2 integration
  (app/tests/camera) + 0 doc = 10 passed, 0 failed.
- `cargo check -p lazyref` → exit 0 (1 dead-code warning, Task 3 fields).

## Diff
```
Cargo.lock                |   1 +          (Task 1 lane, uncommitted)
crates/app/Cargo.toml     |   1 +          (Task 1 lane, uncommitted)
crates/app/src/main.rs    | 282 ++++----   (this task: 157+/153- net)
crates/board/src/board.rs |  26 +++++      (Task 1 lane, uncommitted)
```

## Not committed
Per constraints (orchestrator commits, lanes never commit). Task 1 changes
(Cargo.toml dep, Cargo.lock, board.rs `remove_handling`) were pre-existing
uncommitted work in this tree, untouched by this lane.
