# Sprite Pipeline Workflow

From downloaded asset pack to game-ready sprites using a
mechanical → human → export pipeline.

Three programs, used three times. A single `sprite-packs.toml` can drive
all tools across multiple asset packs.


## Pipeline overview

```
  ┌─────────────────┐
  │                  │
  │  sprite_discover │    Step 1: CLI, mechanical
  │  (CLI)           │    directory → TOML with [images]
  │                  │
  └────────┬─────────┘
           │
           ▼
  ┌─────────────────┐
  │                  │
  │  sprite_grid     │    Step 2: interactive Bevy tool
  │  (Bevy + egui)   │    human sets grids + spans,
  │                  │    tool runs analysis + dedup
  │                  │    internally
  │                  │
  └────────┬─────────┘
           │
           ▼
  ┌─────────────────┐
  │                  │
  │  sprite_export   │    Step 3: gridded TOML →
  │  (CLI)           │    game-ready assets
  │                  │
  │  → runtime TOML  │
  │  + copied assets  │
  │                  │
  └────────┬─────────┘
           │
           ▼
  ┌─────────────────┐
  │                  │
  │  sprite_runtime  │
  │  (Bevy plugin)   │
  │  cell → sprite   │
  │                  │
  └──────────────────┘
```


## Key design: single TOML file

Every stage reads and writes the **same TOML file**. Each stage adds
its expertise:

```
sprite_discover    →  sprite_grid      →  sprite_export
(mechanical)          (human + tool)         (CLI)

Adds:                 Adds:                  Produces:
  top-level context     [sheets.*]             runtime TOML
  [images.*]            [catalog.*]            copied assets
  (per-file facts,      (sources + analysis)
   dir filtering)       (hash, bbox, dedup)
```


## Step 1: Discover — directory to mechanical facts

**Tool:** `sprite_discover`

Point at a directory of downloaded PNG assets. The tool walks the
directory, records mechanical facts about every image file, and
outputs a TOML file with pipeline context and per-image entries.

**Single-pack mode:**

```
cargo run --example sprite_discover -- D:/assets/SomePack -o pack.toml
cargo run --example sprite_discover -- D:/assets/SomePack -o pack.toml --contact contact.png
cargo run --example sprite_discover -- D:/assets/SomePack -o pack.toml --exclude "*.bak"
```

**Batch mode (recommended):** Use `sprite-packs.toml` to discover all
packs at once:

```
cargo run --example sprite_discover -- --config sprite-packs.toml
```

See [sprite-packs.toml config](#pipelinetoml-config) for the file format.

**What it does:**

1.  Loads directory-filtering config from `asset_browser.toml` in the
    input directory (same skip list used by the asset browser). This
    filters out redundant directories like RPG Maker variants,
    individual frames, and individual icon directories.
2.  Walks the directory recursively, collects all `.png` files that
    are not in skipped directories.
3.  For each image, records:
    -   Dimensions (width x height)
    -   File size on disk
    -   Unique color count
    -   Transparency percentage
    -   FNV-1a hash of raw RGBA pixels
    -   Valid cell widths/heights (which of 8, 16, 24, 32, 48, 64
        divide evenly)
4.  Outputs a TOML file with:
    -   Pipeline context: `name`, `pack_root`, `exclude`,
        `contact_sheet`
    -   Per-image entries in `[images.*]`
5.  Prints next-step instructions (open `sprite_grid`).

**Flags:**

| Flag                 | Effect                                        |
|----------------------|-----------------------------------------------|
| `--name "Pack Name"` | Set the pack name in the TOML                 |
| `--exclude "*.bak"`  | Skip files matching the glob pattern          |
| `--thumb-size 64`    | Max thumbnail size in pixels (default 64)     |
| `--contact out.png`  | Generate a contact sheet with thumbnails      |

**Output:** A single TOML file with context and `[images.*]` section,
plus next-step instructions on stderr.


## Step 2: Define grids — human + interactive editor

**Tool:** `sprite_grid`

**Single-pack mode:**

```
cargo run --example sprite_grid -- pack.toml --pack-root D:/assets/SomePack
```

**Config mode (recommended):** Launches with a pack selector
dropdown, letting you switch between packs without relaunching:

```
cargo run --example sprite_grid -- --config sprite-packs.toml
```

An interactive Bevy + egui app for defining grids and spans.

**What the human does:**

-   **Switch packs** via dropdown (config mode only) — auto-saves
    dirty files, shows `[+]`/`[-]` indicators for discovered packs
-   **Navigate** through images with Left/Right arrows (or LB/RB)
-   **Cycle cell sizes** from `valid_cell_widths`/`valid_cell_heights`
    with +/- keys
-   **See occupancy** — grid overlay shows empty cells with X marks
-   **Apply grid** — creates `[sheets.*]` entry + `[catalog.*]`
    entries for all occupied cells, runs analysis + hash dedup
-   **Define spans** — right-click drag on grid to merge cells into
    multi-cell sprites (trees, buildings, etc.)
-   **Pan/zoom** with mouse wheel and click-drag
-   **Save** with Ctrl+S

**Progress indicator:** "3/13 images gridded" shown in the panel.

**What the tool does internally (no human action needed):**

-   For each occupied cell: compute hash, bounding box, pixel count,
    color count, empty detection
-   Cross-cell hash-based dedup (marks `duplicate_of`)

**Output:** The same TOML file, now with `[sheets.*]` and
`[catalog.*]` sections (including analysis fields).


## Step 3: Export — TOML to game-ready assets

**Tool:** `sprite_export`

**Single-pack mode:**

```
cargo run --example sprite_export -- pack.toml --pack-root D:/assets/SomePack -o assets/castle
```

**Batch mode:** Export all packs at once. Packs without sheets are
skipped with a warning.

```
cargo run --example sprite_export -- --config sprite-packs.toml
```

Reads the metadata TOML and produces two outputs:

1.  **Runtime TOML** (`castle.toml`) — stripped metadata with only what
    the game needs: sheets (file + grid) and catalog (non-empty,
    non-duplicate entries with sources only).

2.  **Copied assets** — sheet images and standalone files copied into the
    output directory so Bevy's asset server can load them.

**Output:** A self-contained directory with everything the game needs.


## Runtime — Bevy integration

**Module:** `sprite_runtime` (Bevy plugin)

```rust
app.add_plugins(SpriteMetadataPlugin::new("assets/castle/castle.toml"));
```

Loads the runtime TOML at startup and provides a `SpriteDatabase`
resource:

```rust
fn setup(db: Res<SpriteDatabase>) {
    // Look up by sheet + cell coordinate
    let loc = db.cell("castle-walls", 3, 5);
    // → CellLocation { image_path, rect }

    // Get sheet dimensions
    let (cols, rows) = db.sheet_dims("castle-walls").unwrap();

    // Iterate all sheets
    for sheet_id in db.sheet_ids() {
        // ...
    }
}
```

The plugin resolves the full lookup path (catalog entry → source →
sheet → file path + pixel rect) once at load time. Systems just call
`cell()` with a sheet ID and coordinates.

## Data flow summary

```
              step 1        step 2         step 3
            discover        grid           export

asset dir ► ┌──────────────────────────────────────────────────────┐
            │                    single TOML file                  │
            │                                                      │
            │  name, pack_root, exclude, contact_sheet  (context)  │
            │  [images.*]          (step 1: mechanical facts)      │
            │  [sheets.*]          (step 2: human-defined grids)   │
            │  [catalog.*]         (step 2: tool creates + analyzes│
            └─────────────────────────┬────────────────────────────┘
                                      │
                                      ▼
                             ┌─────────────────┐
                             │  sprite_export   │
                             │  → runtime TOML  │
                             │  + copied assets  │
                             └─────────────────┘

The runtime TOML is loaded by sprite_runtime (Bevy plugin) at game startup.
```


## sprite-packs.toml config

A single config file drives all three pipeline tools across multiple
asset packs. Every tool derives paths from it:

-   `{output_dir}/{name}.toml` — metadata TOML
-   `{asset_root}/{dir}` — pack root (original PNGs)
-   `{export_dir}/{name}/` — export output

```toml
asset_root = "D:/keep/assets/unzipped"
output_dir = "generated"
export_dir = "assets"

[[packs]]
name = "time-fantasy-tiles"
dir = "TimeFantasy_TILES_6.24.17"
exclude = ["guide.png"]

[[packs]]
name = "time-fantasy-characters"
dir = "timefantasy_characters"
```

| Field        | Default    | Description                                |
|--------------|------------|--------------------------------------------|
| `asset_root` | (required) | Base directory containing downloaded packs |
| `output_dir` | (required) | Where metadata TOMLs are written           |
| `export_dir` | `"assets"` | Where sprite_export writes game assets     |

Each `[[packs]]` entry has:

| Field     | Default | Description                                     |
|-----------|---------|------------------------------------------------|
| `name`    | (required) | Pack identifier, used for filenames           |
| `dir`     | (required) | Directory under `asset_root` (or absolute)    |
| `exclude` | `[]`    | Glob patterns to skip during discovery          |


## File inventory

All tools live in `examples/` and share code via `examples/shared/`.

### Pipeline tools

| File                          | Role                                                |
|-------------------------------|-----------------------------------------------------|
| `examples/sprite_discover.rs` | Step 1: directory → TOML with images + context      |
| `examples/sprite_grid.rs`     | Step 2: interactive grid definition                 |
| `examples/sprite_export.rs`   | Step 3: gridded TOML → game-ready assets            |

### Shared modules

| File                                 | Contents                                              |
|--------------------------------------|-------------------------------------------------------|
| `examples/shared/sprite_meta.rs`     | TOML types: metadata, pipeline config, manifest, verification |
| `examples/shared/sprite_analysis.rs` | Image analysis: grid detection, cell analysis, hashing |
| `examples/shared/sprite_runtime.rs`  | Bevy plugin: SpriteDatabase, CellLocation              |
| `examples/shared/scan_config.rs`     | Directory filtering config shared by browser + discover |

### Documentation

| File                                                         | Contents                        |
|--------------------------------------------------------------|---------------------------------|
| `docs/architecture/sprite-pipeline.md`                       | This file — pipeline overview   |
| `docs/architecture/sprite-metadata-format.md`                | Unified TOML format spec        |
| `sprite-packs.toml`                                              | Multi-pack pipeline config      |
