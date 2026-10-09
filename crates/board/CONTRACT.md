# Board crate — lane contract (READ THIS FIRST)

Ten lanes implement one lane each in `crates/board/src/`. This file pins the
public shapes every lane exposes and consumes. **Implement exactly these
signatures** (you may add private helpers and `#[cfg(test)]` freely; do not
rename or reshape the pinned items — sibling lanes compile against them).

Crate name: `lazyref-board`, lib path `lazyref_board::`.

## Shared vocabulary

- All angles in **degrees**, counter-clockwise positive (matches the app).
- `Mat3` is row-major `[a,b,tx, c,d,ty, 0,0,1]` = Android `Matrix.getValues()` order.
- Points are flat pairs: `&[f32]` with even length, or `[f32; 8]` for quads.
- f32 for board math, f64 for scene (`.pur`) math. Convert at the boundary.

## `math.rs` (lane MATH)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat3(pub [f32; 9]);
impl Mat3 {
    pub fn identity() -> Self;
    pub fn translate(x: f32, y: f32) -> Self;
    pub fn scale_about(sx: f32, sy: f32, px: f32, py: f32) -> Self;
    pub fn rotate_about(deg: f32, px: f32, py: f32) -> Self;
    /// self * other: `other` applies to points first (Android post/concat semantics).
    pub fn concat(self, other: Mat3) -> Mat3;
    pub fn invert(self) -> Option<Mat3>;
    pub fn map_points(self, pts: &mut [f32]);          // in-place pairs
    pub fn map_point(self, x: f32, y: f32) -> (f32, f32);
}
pub fn angle_deg(x1: f32, y1: f32, x2: f32, y2: f32) -> f32; // atan2 in degrees
pub fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32;      // hypot
pub fn trap_to_rect(pts: &[f32; 8]) -> (f32, f32, f32, f32); // bounding (l,t,r,b)
pub fn sha256_hex(bytes: &[u8]) -> String;                   // lowercase hex
```

## `transform.rs` (lane TRANSFORM)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform { pub pos: (f32, f32), pub rotation: f32, pub scale: (f32, f32) }
impl Transform {
    pub fn identity() -> Self;                       // pos 0, rot 0, scale 1
    pub fn translate(&mut self, dx: f32, dy: f32);
    pub fn rotate(&mut self, deg: f32);
    pub fn scale_by(&mut self, sx: f32, sy: f32);
    pub fn reset(&mut self);
    pub fn flipped_h(&self) -> bool;                 // scale.0 < 0
    pub fn flipped_v(&self) -> bool;
    pub fn set_flipped_h(&mut self, on: bool);
    pub fn set_flipped_v(&mut self, on: bool);
    /// Local pipeline: scale about `center`, then rotate about `center`,
    /// then translate by `pos` (matches the app's sticker matrix order).
    pub fn matrix(&self, center: (f32, f32)) -> Mat3;
}
```

## `sticker.rs` (lane STICKER)

```rust
pub enum StickerKind {
    Image { data: Vec<u8>, width: u32, height: u32 }, // original encoded bytes
    Text { text: String, size_px: f32, color: (u8,u8,u8),
           background: Option<(u8,u8,u8)>, bold: bool },
}
pub struct Sticker {
    pub transform: Transform,
    pub canvas: Mat3,          // board canvas matrix snapshot
    pub width: f32, pub height: f32,
    pub crop: (f32, f32, f32, f32), // (l,t,r,b) in content px, default = full
    pub kind: StickerKind,
}
impl Sticker {
    pub fn new(kind: StickerKind) -> Self;
    pub fn center(&self) -> (f32, f32);              // (w/2, h/2)
    /// Flip-aware corner winding in local space, matches the app's bound order.
    pub fn bound_points(&self) -> [f32; 8];
    pub fn cropped_points(&self) -> [f32; 8];
    /// canvas * transform.matrix(center).
    pub fn final_matrix(&self) -> Mat3;
    pub fn mapped_points(&self, src: &[f32]) -> Vec<f32>;
    /// Hit test: un-rotate by -rotation, trap to rect (matches the app).
    pub fn contains(&self, x: f32, y: f32) -> bool;
    /// Clamped like the app (min 1px, inside real bounds).
    pub fn set_crop(&mut self, crop: (f32, f32, f32, f32));
}
/// Decode dimensions of encoded PNG/JPEG bytes (`image` crate). None if undecodable.
pub fn decode_image(data: &[u8]) -> Option<(u32, u32)>;
```

## `scene.rs` (lane SCENE — pure f64 `.pur`/board math)

```rust
pub struct StickerDecomp { pub pos_x: f64, pub pos_y: f64, pub rotation_deg: f64,
                           pub scale_x: f64, pub scale_y: f64 }
pub struct ItemTransforms { pub image_transform: [f64; 6], pub item_transform: [f64; 6] }
pub const RECT_PATH_TYPES: [i32; 5];                       // [0,1,1,1,1]
pub fn qt9_to_w(qt: &[f64; 9]) -> [f64; 6];
pub fn w_to_qt9(w: &[f64; 6]) -> [f64; 9];
pub fn affine_mul(m1: &[f64; 6], m2: &[f64; 6]) -> [f64; 6]; // m1 after m2
pub fn decompose_to_sticker(w: &[f64; 6], c: (f64, f64)) -> StickerDecomp;
pub fn compose_from_sticker(a: [f64; 4], t: (f64, f64), c: (f64, f64)) -> ItemTransforms;
pub fn rect_path5(cl: f64, ct: f64, cr: f64, cb: f64, cx: f64, cy: f64) -> [f64; 10];
pub fn assign_slots<T: Clone>(existing: &[T], count: usize, new_slot: impl Fn(usize) -> T) -> Vec<T>;
```

`[f64; 6]` layout is `W = [m11, m21, m31, m12, m22, m32]`
(row-vector affine: `x' = m11*x + m21*y + m31`).

## `serializer.rs` (lane SERIAL)

```rust
pub const SERIAL_VERSION: i32 = 1;
pub struct RefEntry {
    pub bounds: (i32, i32, i32, i32),  // real pixel bounds (l,t,r,b)
    pub pos: (f32, f32), pub rotation: f32, pub scale: (f32, f32),
    pub crop: (f32, f32, f32, f32), pub flip_h: bool, pub flip_v: bool,
}
pub struct RefImage { pub sha: String, pub bitmap: Vec<u8>, pub instances: Vec<RefEntry> }
/// Text-sticker sidecar: text stickers persist their editable fields here, keyed by
/// position in `RefBoardData.images`. The `bitmap` slot of a text image entry is
/// written empty (no renderer in this crate); the shell re-renders text at load.
pub struct RefText {
    pub image_index: usize, pub text: String, pub size_px: f32,
    pub color: (u8, u8, u8), pub background: Option<(u8, u8, u8)>, pub bold: bool,
}
pub struct RefBoardData { pub version: i32, pub date: i64, pub canvas: [f32; 9],
                          pub images: Vec<RefImage>, pub texts: Vec<RefText> }
pub fn encode(board: &RefBoardData) -> Vec<u8>;
pub fn decode(bytes: &[u8]) -> Result<RefBoardData, String>;
```

Wire format (MessagePack, hand-rolled subset — no new deps):
`int version, int64 date, 9×float32 canvas, array of (str sha, bin bitmap,
array of entries)`, each entry = `4×int bounds, 2×float pos, float rot,
2×float scale, 4×float crop, bool flipH, bool flipV`,
then an array of texts: `u16 image_index, str text, float32 size,
3×u8 color, u8 has_bg, 3×u8 bg (zeros when none), u8 bold`.
Bitmaps deduped by `sha256_hex` of bytes (caller builds the map; encode
writes what it's given).

## `icons.rs` (lane ICONS)

```rust
// Gravity constants for crop/scale handles (match the app's icon gravities).
pub const LEFT_TOP: u32 = 0; pub const RIGHT_TOP: u32 = 1;
pub const LEFT_BOTTOM: u32 = 2; pub const RIGHT_BOTTOM: u32 = 3;
pub const LEFT_CENTER: u32 = 4; pub const RIGHT_CENTER: u32 = 5;
pub const TOP_CENTER: u32 = 6; pub const BOTTOM_CENTER: u32 = 7;
pub enum IconOp { Delete, ScaleStep(f32), RotateLeft, RotateRight, FlipH, FlipV, FlipBoth }
/// Pure transform ops (no Board dependency — the board lane calls these).
pub fn apply_op(t: &mut Transform, op: &IconOp);
/// Crop-edge drag under flips: map a gravity through the sticker's flip state.
pub fn flip_gravity(gravity: u32, flip_h: bool, flip_v: bool) -> u32;
/// Touch slop for icon hit-testing (matches the app's generous icon touch area).
pub const ICON_TOUCH_RADIUS: f32 = 48.0;
```

## `board.rs` (lane BOARD)

```rust
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ActionMode { None, Drag, PinchSticker, Icon, Click, CanvasDrag, CanvasPinch }
#[derive(Clone, Copy, Debug)]
pub struct Pointer { pub id: i64, pub x: f32, pub y: f32 }
#[derive(Clone, Copy, Debug)]
pub enum BoardEvent { Down(Pointer), Move(Pointer), Up(Pointer) }
/// What the shell (egui/Android) must do after events. Returned from handle_event.
#[derive(Debug)]
pub enum BoardAction { Redraw, StickerAdded(usize), StickerDeleted(usize),
                       Clicked(usize), DoubleTapped(usize), StickerMoved(usize) }
pub struct Board {
    pub stickers: Vec<Sticker>,
    pub handling: Option<usize>,
    pub canvas: Mat3,
    // ... lane adds mode flags, icon sets, timing state as needed
}
impl Board {
    pub fn new() -> Self;
    pub fn add_sticker(&mut self, s: Sticker) -> usize;   // selects it, emits via return
    pub fn remove_all(&mut self);
    pub fn stickers(&self) -> &[Sticker];
    pub fn canvas_matrix(&self) -> Mat3;
    pub fn set_canvas_matrix(&mut self, m: Mat3);          // re-pushes to stickers
    /// Feed one pointer event; returns shell actions. Owns the full
    /// select/drag/pinch-canvas/pinch-sticker(icon or scale/rotate)/click state machine.
    pub fn handle_event(&mut self, ev: BoardEvent) -> Vec<BoardAction>;
    // Lane also adds: duplicate/flip/reset/crop ops, fit(bounds,viewport,margin).
}
```

## `io.rs` (lane IO)

```rust
pub enum FileFormat { Ref, Pur }
pub fn detect(bytes: &[u8]) -> FileFormat;                 // v2/legacy sniff
pub fn open_ref(bytes: &[u8]) -> Result<Board, String>;
pub fn save_ref(board: &Board) -> Vec<u8>;
pub fn open_pur(bytes: &[u8], scratch: &std::path::Path) -> Result<Board, String>;
pub fn save_pur(board: &Board, scratch: &std::path::Path, thumbnail: &[u8]) -> Vec<u8>;
```

`.ref` = serializer codec over drawable state (text stickers persist as a
rendered-image fallback + their text in `RefEntry` scale slots is NOT available —
lane decision, document it: simplest honest rule is persist text stickers by
their current fields in a sidecar entry; keep it minimal and tested).
`.pur` open maps scene rows to stickers via `scene::decompose_to_sticker`;
save inverts via `compose_from_sticker` (new scenes get fresh sort slots).

## `settings.rs` (lane APPRENDER)

```rust
pub struct Settings { pub downscale_on_import: bool, pub multistep_scale: bool,
                      pub max_resolution: u32 }            // pixels, e.g. 2048*2048
impl Settings {
    pub fn defaults() -> Self;                             // true, true, 2048²
    pub fn load(path: &std::path::Path) -> Self;           // missing file = defaults
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()>;
    pub fn max_options() -> [u32; 4];                      // 1024², 2048², 3072², 4096²
}
/// Downscale decoded RGBA to fit `limit` px total (multi-step halving when
/// `multistep`); returns (rgba, w, h). Pure `image`-crate ops.
pub fn fit_image(rgba: &[u8], w: u32, h: u32, limit: u32, multistep: bool) -> (Vec<u8>, u32, u32);
```

## Rules for all lanes

- One lane = the files named in its brief. Touch NOTHING else (never
  `Cargo.toml`, `Cargo.lock`, other modules, `crates/app`, `crates/purcore`).
- No `git commit`. No new dependencies (board `Cargo.toml` already has
  `purcore` + `sha2` + `image`; use only those + std).
- Comments describe behavior. No pointers to private paths, no internal
  tooling names — this ships public.
- Write the file in the FIRST THIRD of your run, then tests, then verify.
  A killed lane still leaves its contribution on disk.
- Verify with `cargo test -p lazyref-board <your_test>` (expect cargo file-lock
  waits while sibling lanes build — wait, don't kill). Paste the tail.
- Unit tests live inline (`#[cfg(test)]`); cross-cutting integration tests
  belong to lane TESTS — still write your own module tests.
