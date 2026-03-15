//! Converts an LDtk map file into a RON map file for grid_world.
//!
//! Reads the LDtk JSON, extracts wall/floor cell data with autotile source rects,
//! and writes a compact RON file that grid_world loads at startup.
//!
//! Usage: `cargo run --example ldtk_convert`
//!
//! Input:  maps/9keys.ldtk
//! Output: maps/9keys.ron

use serde::{Deserialize, Serialize};

const LDTK_PATH: &str = "maps/9keys.ldtk";
const RON_PATH: &str = "maps/9keys.ron";

// ---------------------------------------------------------------------------
// Output format (shared with grid_world via #[path] or copy)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct GridMap {
    cols: usize,
    rows: usize,
    start_col: usize,
    start_row: usize,
    /// Row-major, row 0 = bottom (Bevy Y-up). Each entry: (kind, src_x, src_y).
    cells: Vec<CellEntry>,
}

#[derive(Serialize)]
enum CellKind {
    Floor,
    Wall,
}

#[derive(Serialize)]
struct CellEntry {
    kind: CellKind,
    src_x: u32,
    src_y: u32,
}

// ---------------------------------------------------------------------------
// LDtk JSON (minimal typed access)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LdtkRoot {
    #[serde(rename = "defaultGridSize")]
    default_grid_size: u32,
    levels: Vec<LdtkLevel>,
}

#[derive(Deserialize)]
struct LdtkLevel {
    #[serde(rename = "worldX")]
    world_x: i64,
    #[serde(rename = "worldY")]
    world_y: i64,
    #[serde(rename = "pxWid")]
    px_wid: u32,
    #[serde(rename = "layerInstances")]
    layer_instances: Vec<LdtkLayer>,
}

#[derive(Deserialize)]
struct LdtkLayer {
    #[serde(rename = "__identifier")]
    identifier: String,
    #[serde(rename = "autoLayerTiles")]
    auto_layer_tiles: Vec<LdtkAutoTile>,
    #[serde(rename = "entityInstances")]
    entity_instances: Vec<LdtkEntity>,
}

#[derive(Deserialize)]
struct LdtkAutoTile {
    px: [u32; 2],
    src: [u32; 2],
}

#[derive(Deserialize)]
struct LdtkEntity {
    #[serde(rename = "__identifier")]
    identifier: String,
    #[serde(rename = "__grid")]
    grid: [u32; 2],
}

fn main() {
    let json_str = std::fs::read_to_string(LDTK_PATH)
        .unwrap_or_else(|e| panic!("Failed to read {LDTK_PATH}: {e}"));
    let root: LdtkRoot = serde_json::from_str(&json_str)
        .unwrap_or_else(|e| panic!("Failed to parse {LDTK_PATH}: {e}"));

    let gs = root.default_grid_size;
    let levels = &root.levels;

    // Grid layout from world positions
    let min_x = levels.iter().map(|l| l.world_x).min().unwrap();
    let min_y = levels.iter().map(|l| l.world_y).min().unwrap();
    let level_px = levels[0].px_wid as i64;
    let cells_per_level = (level_px as u32 / gs) as usize;

    let max_gc = levels.iter().map(|l| ((l.world_x - min_x) / level_px) as usize).max().unwrap();
    let max_gr = levels.iter().map(|l| ((l.world_y - min_y) / level_px) as usize).max().unwrap();
    let grid_cols = max_gc + 1;
    let grid_rows = max_gr + 1;
    let total_cols = grid_cols * cells_per_level;
    let total_rows = grid_rows * cells_per_level;

    // Initialize cells
    let mut cells: Vec<Vec<CellEntry>> = (0..total_rows)
        .map(|_| {
            (0..total_cols)
                .map(|_| CellEntry { kind: CellKind::Floor, src_x: 0, src_y: 0 })
                .collect()
        })
        .collect();
    let mut start_col = 0usize;
    let mut start_row = 0usize;

    for level in levels {
        let gc = ((level.world_x - min_x) / level_px) as usize;
        let gr = ((level.world_y - min_y) / level_px) as usize;

        for layer in &level.layer_instances {
            // Start position
            if layer.identifier == "Entities" {
                for entity in &layer.entity_instances {
                    if entity.identifier == "Start_Position" {
                        let lc = entity.grid[0] as usize;
                        let lr = entity.grid[1] as usize;
                        start_col = gc * cells_per_level + lc;
                        start_row = (grid_rows - 1 - gr) * cells_per_level + (cells_per_level - 1 - lr);
                    }
                }
                continue;
            }

            let is_wall = layer.identifier == "Walls";
            let is_floor = layer.identifier == "Floor";
            if !is_wall && !is_floor { continue; }

            for tile in &layer.auto_layer_tiles {
                let local_col = tile.px[0] as usize / gs as usize;
                let local_row = tile.px[1] as usize / gs as usize;

                // LDtk Y-down → Bevy Y-up
                let bevy_col = gc * cells_per_level + local_col;
                let bevy_row = (grid_rows - 1 - gr) * cells_per_level + (cells_per_level - 1 - local_row);

                cells[bevy_row][bevy_col] = CellEntry {
                    kind: if is_wall { CellKind::Wall } else { CellKind::Floor },
                    src_x: tile.src[0],
                    src_y: tile.src[1],
                };
            }
        }
    }

    // Flatten to row-major (row 0 = bottom)
    let flat: Vec<CellEntry> = cells.into_iter().flatten().collect();

    let map = GridMap {
        cols: total_cols,
        rows: total_rows,
        start_col,
        start_row,
        cells: flat,
    };

    let ron_str = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("Failed to serialize RON: {e}"));

    std::fs::write(RON_PATH, &ron_str)
        .unwrap_or_else(|e| panic!("Failed to write {RON_PATH}: {e}"));

    println!("Wrote {RON_PATH}");
    println!("  {total_cols}×{total_rows} grid, start at ({start_col}, {start_row})");
    println!("  {} cells total", map.cells.len());
}
