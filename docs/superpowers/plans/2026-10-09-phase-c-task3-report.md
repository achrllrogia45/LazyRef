# Phase C Task 3 report — toolbar/settings/dialogs (2026-10-09)

Task: plan `2026-10-09-phase-c-app-shell.md` Task 3 only (`crates/app/src/main.rs`).
Not committed, per plan.

## Test lines

- `cargo test -p lazyref toolbar_delete_uses_remove_handling` → `test tests::toolbar_delete_uses_remove_handling ... ok` (PASS first run, characterization; exit 0, also after implementation)
- `cargo test --workspace` → all green, exit 0:
  - lazyref lib: 7 passed
  - lazyref bin (app): 2 passed (`content_bounds_covers_board_stickers`, `toolbar_delete_uses_remove_handling`)
  - camera integration: 2 passed
  - lazyref-board lib: 63 passed
  - board integration: 8 passed
  - purcore selftest: 10 passed

## Gates

- `cargo check -p lazyref` → exit 0
- `cargo clippy -p lazyref --all-targets` → exit 0 (3 pre-existing warnings in purcore/board crates, none in app)

## Deviation from plan-verbatim (Step 3)

Plan's settings `Window` does not compile: `.open(&mut self.show_settings)` +
closure capturing `self` = E0500. Minimal fix, same behavior: local `open` flag
+ deferred `saved: Option<Settings>` / `save_failed` applied after `show()`
returns (first attempt assigning `open` inside closure = E0499).

## git diff --stat

```
Cargo.lock                |   1 +
crates/app/Cargo.toml     |   1 +
crates/app/src/main.rs    | 407 +++++++++++++++++++++++++++++-----------------
crates/board/src/board.rs |  26 +++
4 files changed, 286 insertions(+), 149 deletions(-)
```

Note: `Cargo.lock`, `Cargo.toml`, `board.rs` lines are pre-existing uncommitted
Task 1/2 work; Task 3 touches only `crates/app/src/main.rs` (toolbar +
settings window + 1 test). Existing Delete-key shortcut and F-fit kept.
