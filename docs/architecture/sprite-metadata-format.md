# Sprite Metadata Format — Specification

A TOML format for mapping cell coordinates to their physical locations
across multiple sprite sheets. Designed for asset packs that ship
the same sprites at multiple scales and in multiple layouts.

The format uses a **single TOML file** as the coordinator through a
mechanical → human → export pipeline. Each stage reads the file,
adds its expertise, and writes it back.

Validation is done by Rust types with `#[derive(Deserialize, Serialize)]`.
If serde can deserialize the file into the defined structs, it is valid.


## Top-level structure

A valid file has pipeline context fields, a discovery section (`images`),
and enriched sections (`sheets`, `catalog`).

```toml
name = "Pack Name"
description = "Human-readable description of the asset pack."
pack_root = "D:/assets/PackName"
exclude = ["guide.png", "RPGMAKER/**"]
contact_sheet = "generated/pack-contact.png"

[images."TILESETS/castle.png"]
# ... per-image mechanical facts ...

[sheets.castle]
# ... grid definition ...

[catalog."castle.0.0"]
# ... per-cell entry ...
```

| Field           | Type     | Required | Written by      | Description                                                                                        |
| --------------- | -------- | -------- | --------------- | -------------------------------------------------------------------------------------------------- |
| `name`          | string   | no       | sprite_discover | Asset pack name                                                                                    |
| `description`   | string   | no       | human           | Human-readable description                                                                         |
| `pack_root`     | string   | no       | sprite_discover | Absolute path to the asset directory                                                               |
| `exclude`       | string[] | no       | sprite_discover | Glob patterns that were excluded (file-level; directory-level filtering uses `asset_browser.toml`) |
| `contact_sheet` | string   | no       | sprite_discover | Path to generated contact sheet PNG                                                                |
| `images`        | table    | no       | sprite_discover | Per-image mechanical facts                                                                         |
| `sheets`        | table    | no       | sprite_grid     | Sheet definitions, keyed by sheet ID                                                               |
| `catalog`       | table    | **yes**  | sprite_grid     | Physical asset inventory, keyed by ID                                                              |

A file with only `catalog` is valid — it is a physical asset inventory
with no semantic interpretation.


## Images (discovery data)

The `images` section contains per-image mechanical facts recorded by
`sprite_discover`. Image keys are relative paths from the pack root
directory, using forward slashes.

```toml
[images."TILESETS/castle.png"]
width = 544
height = 272
file_size_bytes = 48291
color_count = 37
transparent_pct = 61
hash = "a1b2c3d4e5f67890"
valid_cell_widths = [16, 32]
valid_cell_heights = [8, 16, 32]

```

| Field                | Type      | Required | Description                                           |
| -------------------- | --------- | -------- | ----------------------------------------------------- |
| `width`              | integer   | **yes**  | Image width in pixels                                 |
| `height`             | integer   | **yes**  | Image height in pixels                                |
| `file_size_bytes`    | integer   | **yes**  | File size on disk in bytes                            |
| `color_count`        | integer   | **yes**  | Number of unique RGBA colors                          |
| `transparent_pct`    | integer   | **yes**  | Percentage of fully-transparent pixels (alpha = 0)    |
| `hash`               | string    | **yes**  | FNV-1a hash of raw RGBA pixel data (16-char hex)      |
| `valid_cell_widths`  | integer[] | no       | Which of [8, 16, 24, 32, 48, 64] divide width evenly  |
| `valid_cell_heights` | integer[] | no       | Which of [8, 16, 24, 32, 48, 64] divide height evenly |

Note: redundant directories (RPG Maker variants, individual frames/icons)
are filtered out upstream by `asset_browser.toml` before discovery runs.
Only meaningful sprite sheets and standalone files appear in this section.


## Sheet definitions

Each entry in `sheets` defines a sprite sheet image and its grid layout.
Catalog entries reference sheets by their ID key.

```toml
[sheets.castle-generic]
file = "TILESETS/castle.png"
cell_w = 16
cell_h = 16
cols = 34
rows = 17
color_count = 142
transparent_pct = 38
description = "Castle structures tileset"
```

| Field             | Type    | Required | Description                                 |
| ----------------- | ------- | -------- | ------------------------------------------- |
| `file`            | string  | **yes**  | Path to image file, relative to pack root   |
| `cell_w`          | integer | **yes**  | Cell width in pixels                        |
| `cell_h`          | integer | **yes**  | Cell height in pixels                       |
| `cols`            | integer | **yes**  | Number of columns in grid                   |
| `rows`            | integer | **yes**  | Number of rows in grid                      |
| `scale`           | number  | no       | Scale factor relative to 1x base size       |
| `color_count`     | integer | no       | Total unique RGBA colors in the sheet image |
| `transparent_pct` | integer | no       | Percentage of pixels with alpha=0           |
| `description`     | string  | no       | Human-readable note                         |

The expected image dimensions are `cell_w * cols` by `cell_h * rows`.
The `cols` and `rows` values are computed from actual image dimensions
and the chosen grid size.


## Catalog

The `catalog` section is a flat map of physical asset entries. Each
entry represents one **cell** — which might be a single grid cell,
a multi-cell span, or a standalone image file. Keys are mechanical
IDs derived from sheet name + cell position (for sheet cells) or
from filename (for individual files).

The sprite editor creates catalog entries with `sources` and runs
per-cell analysis (hash, bounding box, colors, empty detection,
dedup).

```toml
[catalog."castle.24.2"]
sources = [
  { sheet = "castle-generic", col = 24, row = 2, col_span = 9, row_span = 5 },
]

[catalog."icon449.fc.16"]
sources = [
  { file = "fullcolor/individual_16x16/icon449.png" },
  { sheet = "fullcolor-16", col = 0, row = 28 },
]
```

| Field          | Type    | Required | Written by  | Description                                                                     |
| -------------- | ------- | -------- | ----------- | ------------------------------------------------------------------------------- |
| `sources`      | array   | **yes**  | sprite_grid | One or more physical locations (see source types)                               |
| `derived_from` | object  | no       | sprite_grid | Mechanical relationship to another catalog entry                                |
| `empty`        | boolean | no       | sprite_grid | True if every pixel has alpha=0                                                 |
| `bbox`         | array   | no       | sprite_grid | Bounding box `[x, y, w, h]` of non-transparent content, relative to crop origin |
| `pixels`       | integer | no       | sprite_grid | Non-transparent pixel count                                                     |
| `colors`       | integer | no       | sprite_grid | Unique RGBA color count                                                         |
| `hash`         | string  | no       | sprite_grid | FNV-1a hex hash of raw RGBA pixel data                                          |
| `duplicate_of` | string  | no       | sprite_grid | Catalog ID of the first entry with the same hash                                |

When the same pixel content is accessible via multiple paths (standalone
file and sheet cell), all paths appear in a single catalog entry's
`sources` array.


### Per-cell analysis

The sprite grid tool computes deterministic
physical analysis for each catalog entry. These facts require no
interpretation — they are computed directly from pixel data.

- **Empty detection**: a cell is `empty = true` when every pixel has
  alpha=0. Empty cells omit all other analysis fields for compactness.
- **Bounding box**: the tightest `[x, y, w, h]` rectangle containing
  all non-transparent pixels, relative to the cell's crop origin.
- **Pixel and color counts**: count of non-transparent pixels and
  unique RGBA values within the cell region.
- **Hash**: FNV-1a hash of the raw RGBA pixel data (16-char lowercase
  hex). Used for duplicate detection across the catalog.
- **Duplicate detection**: entries with identical hashes have
  `duplicate_of` set to the first entry's ID.

```toml
[catalog."castle-generic.3.0"]
sources = [{ sheet = "castle-generic", col = 3, row = 0 }]
empty = true

[catalog."castle-generic.1.1"]
sources = [{ sheet = "castle-generic", col = 1, row = 1 }]
bbox = [0, 2, 16, 14]
pixels = 198
colors = 8
hash = "a1b2c3d4e5f67890"
```


### Derived-from relationships (catalog)

A catalog entry may declare that it is mechanically derived from another
catalog entry via a detectable transform. This is for relationships
that can be determined from pack structure (e.g. scale groups where
filenames follow a pattern).

```toml
[catalog."icon449.fc.32"]
sources = [
  { file = "fullcolor/individual_32x32/icon449.png" },
  { sheet = "fullcolor-32", col = 0, row = 28 },
]
derived_from = { entry = "icon449.fc.16", method = "scale", factor = 2.0 }
```

| Field     | Type   | Required | Description                                              |
| --------- | ------ | -------- | -------------------------------------------------------- |
| `entry`   | string | **yes**  | ID of the base catalog entry                             |
| `method`  | string | **yes**  | Transform type: `scale`, `palette`, `mirror`, `rotate`   |
| `factor`  | number | no       | Scale factor (required when method is `scale`)           |
| `axis`    | string | no       | `"x"` or `"y"` (required when method is `mirror`)        |
| `degrees` | number | no       | `90`, `180`, or `270` (required when method is `rotate`) |


## Source types

Each source object in a catalog entry's `sources` array describes one
physical location. The source type is determined by which fields are
present.

### File source

A standalone image file.

```toml
{ file = "frames/hero_idle_1.png" }
```

| Field  | Type   | Required | Description                               |
| ------ | ------ | -------- | ----------------------------------------- |
| `file` | string | **yes**  | Path to image file, relative to pack root |

### Sheet-cell source

A grid cell (or span of cells) in a defined sheet.

```toml
{ sheet = "hero-sheet", col = 0, row = 2 }
```

For cells that span multiple grid positions:

```toml
{ sheet = "castle-generic", col = 24, row = 2, col_span = 9, row_span = 5 }
```

| Field      | Type    | Required | Description                       |
| ---------- | ------- | -------- | --------------------------------- |
| `sheet`    | string  | **yes**  | Sheet ID (must exist in `sheets`) |
| `col`      | integer | **yes**  | Column index (0-based)            |
| `row`      | integer | **yes**  | Row index (0-based)               |
| `col_span` | integer | no       | Width in cells (default 1)        |
| `row_span` | integer | no       | Height in cells (default 1)       |

### Sheet-rect source

An arbitrary pixel rectangle in a sheet, for non-uniform layouts.

```toml
{ sheet = "atlas", rect = [128, 0, 64, 96] }
```

| Field   | Type   | Required | Description                       |
| ------- | ------ | -------- | --------------------------------- |
| `sheet` | string | **yes**  | Sheet ID (must exist in `sheets`) |
| `rect`  | array  | **yes**  | Pixel rectangle as `[x, y, w, h]` |


## Consumer lookup path

At runtime, cells are accessed by sheet ID + grid coordinates:

```rust
let loc = db.cell("castle-generic", 24, 2);
// → CellLocation { image_path: "TILESETS/castle.png", rect: [384, 32, 144, 80] }
```

The runtime resolves:

1. Find the catalog entry for the given sheet + (col, row)
2. Look up `sheets."sheet_id"` for the image file and cell size
3. Compute pixel rect: `[col * cell_w, row * cell_h, col_span * cell_w, row_span * cell_h]`


## Design principles

- **Single TOML coordinator** — one file carries everything every stage
  needs. `sprite_discover` writes context + `images`. The editor adds
  `sheets` and `catalog`. Each stage reads and writes the same file.

- **Discover → Editor → Export pipeline** — mechanical discovery,
  human grid definition, then export to game-ready assets.

- **Cell-based addressing** — cells are identified by sheet ID +
  (col, row) coordinates. No semantic naming layer needed.

- **Sheets defined once** — grid info lives in one place and is referenced
  by ID. No repetition per catalog entry.

- **Catalog is the inventory** — each catalog entry represents one visual
  cell, which might span multiple grid positions. Multi-cell entries use
  `col_span`/`row_span`.

- **Source merging** — when the same pixel content is accessible via
  multiple paths (standalone file + sheet cell), one catalog entry lists
  all sources.

- **Progressively enriched** — analysis fields are added by the editor
  during grid application. Each is optional.

- **TOML for humans** — comments supported, clean section headers make
  the sections visually distinct, natural for a Rust project.

- **Rust types as schema** — validation is done by deserializing through
  `#[derive(Deserialize, Serialize)]` structs. If serde accepts it, it
  is valid.
