# The `.pur` file format

`.pur` files store PureRef scenes. Two generations exist:

- **v2.x** — the current format: a SQLite database inside a small binary container. `purcore` reads and writes these.
- **v1.x** — the legacy binary format. `purcore` reads these (read-only); saves always write v2.

Everything below matches what `purcore` implements and what its golden-corpus tests verify against
files produced by PureRef 2.1.3.

## Container (v2.x)

```
[ 108-byte header ][ thumbnail (JPEG) ][ DB[C_len : db_size] ][ DB[0 : C_len] ]
```

The scene database is not stored contiguously. The file holds the tail of the database first, and
the database's first `C_len` bytes (`C_len = 108 + thumbnail length`) are duplicated at the very
end of the file, so both the start and the end of the file region carry valid SQLite page headers.
Reassembling the database is `file[db_size:] ++ file[C_len:db_size]`.

Header fields (all multi-byte integers big-endian; "QString" = `u32` byte length followed by
UTF-16BE bytes):

| Bytes | Field | Meaning |
|---|---|---|
| [0:4] + [4:10] | QString | file format version — `"2.1"` |
| [10:12], [12:14] | i16 | two color fields; `0` in all observed files |
| [14:22] | i64 | `db_size` — SQLite database size in bytes; also the offset where the tail head-copy begins |
| [22:26] + [26:36] | QString | version of the app that wrote the file — e.g. `"2.1.3"` |
| [36:40] | u32 | checksum field byte length — `64` |
| [40:104] | UTF-16BE | md5 hex digest of the file from byte 104 on |
| [104:108] | u32 | thumbnail byte length |
| [108:…] | bytes | thumbnail data (JPEG) |

**Checksum.** `md5(file[104:])` — the thumbnail, the database stream, and the head copy — written
as 32 hex characters in UTF-16BE across `[40:104]`. Change any byte at or after offset 104 and the
digest must be recomputed, or PureRef rejects the file on load.

## Scene database (SQLite)

A standard SQLite 3 file. Files written by PureRef 2.1.3 carry `PRAGMA user_version = 200101`
and `application_id = 940753918` (`0x3812C3FE`). Seven tables:

| Table | Holds |
|---|---|
| `images` | image payload bytes, `format`, width/height, `checksum` (md5 of the data), source path / `source_type` |
| `items` | one row per board item: `z`, `opacity`, `transform` (blob), `sort_order` (blob), `parent`, `locked`, `name`, `comment` |
| `items_images` | image-item fields: playback speed/frame/state, `image_bounds` + `image_transform` (blobs), `flags` |
| `items_groups` | group items: lock mode, background color |
| `items_drawings` | stroke items: `strokes` (a QPainterPath blob) |
| `items_notes` | note items: text, colors, style, fixed-size spec |
| `metadata` | single row: thumbnail, `scene_rect`, `view_transform`, scroll positions, `saved` flag, paths, `last_load_checksum`, app version |

Blob-typed columns don't hold SQLite BLOBs — see the storage quirk below.

### Writing files that match PureRef byte-for-byte

Small details decide whether two writers produce identical files. `purcore` reproduces
app-written files exactly, and these are the rules it follows:

- **Table creation order** is the order the pages appear in app-written files:
  `images, items_groups, items_drawings, items_notes, items_images, items, metadata`.
- **Commit structure**: a fresh save commits in **three transactions** — images; then items plus
  its subtype tables; then metadata. The SQLite change counter in the header records this.
- **Version stamp**: the SQLite header field `[96:100]` records the writing library's version.
  App-written files carry SQLite 3.45.3 (`3045003`). Any writer that wants bit-identical output
  should write that value.

None of this affects readability — any valid SQLite database is fine structurally. It only matters
for byte-level reproducibility.

## Binary codecs (blob columns)

The blob-typed columns (`transform`, `sort_order`, `image_bounds`, `image_transform`, `strokes`,
`scene_rect`, `view_transform`) share one encoding: a `QDataStream`-serialized `QVariant`,
stream version 19 (Qt 5.15 conventions). Envelope:

```
[u32 BE typeId][i8 isNull][ name field for user types ][payload]
```

The name field is present only for named user types: `u32` byte length (including the NUL) then the
name plus NUL terminator. Observed type IDs:

| typeId | Type |
|---|---|
| 80 | `QTransform` |
| 20 | `QRectF` |
| 1024 | named user type (`QMetaType::User`) — `"BigRational"`, `"QPainterPath"` |
| 65536 | the Qt6-space form of user types; accepted when reading, not written by 2.1.3 |

Payload layouts (all big-endian):

- **QTransform** — 9 × f64: `m11 m12 m13 / m21 m22 m23 / m31 m32 m33` (72 bytes).
- **QRectF** — 4 × f64: `x y w h` (32 bytes).
- **BigRational** — i32 sign; u64 limb count; limbs as u32 values, least-significant first; then
  the same pair for the denominator. Zero is `sign = 0` with a single limb `[0]`. Used for
  `items.sort_order`.
- **QPainterPath** — i32 element count; per element i32 type + f64 x + f64 y
  (types: 0 = MoveTo, 1 = LineTo, 2 = CurveTo, 3 = CurveToData); then i32 `cStart` and i32
  `fillRule`. An empty path is a bare `u32 0` with no trailing fields.

### The TEXT storage quirk

These binary payloads are not stored as SQLite BLOBs. The app serializes the value, converts the
bytes with the equivalent of `QString::fromLatin1`, and stores that string in a TEXT column —
`typeof()` reports `'text'`. To recover the original bytes, encode the string back to latin-1.
A writer that binds these columns as BLOBs will produce files PureRef reads incorrectly; bind
**TEXT**.

## Legacy v1.x (read path)

The older format is a flat big-endian binary:

- 224-byte header; the version string `"1.10"` sits UTF-16BE at bytes 4–12.
- Checksum: md5 of `bytes[108:]`, stored UTF-16BE at bytes 44–108.
- Strings are length-prefixed UTF-16BE.
- Items are tagged `34` = image, `32` = text. Images are embedded inline (original file bytes);
  duplicate images are stored as 4-byte transform-ID references.

`purcore` reads these into the same scene model; there is no writer for v1.x (saving always emits
v2, which is also the only direction PureRef itself converts).

## Testing

`crates/purcore/tests/` exercises this document against real files:

- byte-identical round-trip of a golden scene (`write_bytes` reproduces the original 38,671-byte file),
- 16 blob fixtures decoded and re-encoded byte-for-byte,
- a legacy v1.x file decoded field-for-field against a reference dump,
- SQLite storage-class checks (`typeof()` = `'text'` for all latin-1 blobs).
