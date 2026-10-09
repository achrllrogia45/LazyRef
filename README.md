# DroidRef (Rust edition)

Rust rewrite of **DroidRef** — a free Android image-reference board, compatible with desktop
PureRef 2.x `.pur` files. Lives beside `../DroidRef-dev` (the Kotlin/Java original) in the
DroidRef workspace.

## Provenance

- **Behavior reference:** `../DroidRef-dev` — fork of [Ruin0x11/DroidRef](https://github.com/Ruin0x11/DroidRef) (MIT), sticker canvas adapted from wuapnjie/StickerView.
- **Format spec:** `../bin-decode/recon/` — PureRef 2.1.3 reverse-engineering campaign (SQLite `.pur` format, container codec, blob codecs, save pipeline — all address-cited).
- **Port source:** Kotlin `pur-core` reference implementation in `../DroidRef-dev/purcore`.

## Layout

```
crates/purcore   .pur format core (library): container codec, checksums, blob codecs,
                 scene model + SQLite (rusqlite), legacy v1.10 reader
crates/app       board UI — desktop-first (egui/eframe); Android target later
                 (android-activity + cargo-ndk), same UI code
```

## Status

- [x] **purcore port** — 7 modules + BE helper; `cargo test -p purcore` 10/10 green: byte-identical rebuild of `S1_redbar-new.pur` (38,671 B), 16/16 blob round-trips, legacy v1.x parity, latin-1 storage classes
- [x] **desktop board prototype** — egui canvas: pan/zoom (zoom-at-cursor invariant tested), placeholder stickers; `cargo test -p droidref` 11/11 green
- [ ] **Android build** — NDK toolchain + `android-activity`, APK output
- [ ] PureRef desktop round-trip conformance for Rust-written files

## Build & test

```bash
cargo test -p purcore          # format core tests (golden corpus fixtures)
cargo test -p droidref         # camera math + board invariants
cargo run  -p droidref         # desktop board prototype (needs a display)
```

## License

MIT — carries the original Ruin0x11/DroidRef copyright; see `LICENSE`.
Rust port: achrllrogia45. Format knowledge: clean-room functional analysis (decompiled output
is documentation, not shipped code — see `../bin-decode/plans/2026-10-07-ROADMAP.md`).
