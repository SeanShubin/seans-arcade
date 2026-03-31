//! Converts the 20-tile autotile sets in KE_Ground_Tiles.png into 47-tile blob
//! autotiling tilesets compatible with LDtk.
//!
//! Usage: cargo run --example blob_tileset
//!
//! Step 1: Generates generated/blob47_reference.png — a reference IntGrid pattern
//!         containing all 47 blob configurations. Recreate this in LDtk with auto-rules.
//!
//! Step 2: Once the LDtk level is set up, re-run to cross-reference and generate
//!         the 4 output tilesets (blob47_set1..4.png).

use image::{imageops, Rgba, RgbaImage};
use std::collections::{BTreeMap, BTreeSet};

const TILE: u32 = 16;
const HALF: u32 = 8;
const OUT_COLS: u32 = 8;

const N: u8 = 1;
const NE: u8 = 2;
const E: u8 = 4;
const SE: u8 = 8;
const S: u8 = 16;
const SW: u8 = 32;
const W: u8 = 64;
const NW: u8 = 128;

fn canonicalize(mask: u8) -> u8 {
    let mut m = mask;
    if m & N == 0 || m & E == 0 { m &= !NE; }
    if m & E == 0 || m & S == 0 { m &= !SE; }
    if m & S == 0 || m & W == 0 { m &= !SW; }
    if m & N == 0 || m & W == 0 { m &= !NW; }
    m
}

/// Generate a reference PNG matching the Wall_Concrete_1_64x64 layout:
/// 8×6 grid, 64×64 pixels per tile, 512×384 total.
/// Each tile contains a miniature 3×3 diagram showing the neighbor pattern.
fn generate_reference_png() {
    let masks: Vec<u8> = (0..=255u8)
        .map(canonicalize)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(masks.len(), 47);

    let cols = 8u32;
    let rows = 6u32;
    let tile_px = 64u32; // matches Wall_Concrete tile size
    let img_w = cols * tile_px; // 512
    let img_h = rows * tile_px; // 384

    let filled = Rgba([30, 30, 30, 255]); // dark filled (like LDtk intgrid)
    let empty_cell = Rgba([180, 180, 180, 255]); // light empty
    let bg = Rgba([200, 200, 200, 255]); // light background
    let grid_line = Rgba([140, 140, 140, 255]); // subtle grid
    let center_mark = Rgba([200, 60, 60, 255]); // red dot for visibility on dark

    let mut img = RgbaImage::from_pixel(img_w, img_h, bg);

    // Draw tile borders
    for ty in 0..rows {
        for tx in 0..cols {
            let px = tx * tile_px;
            let py = ty * tile_px;
            for i in 0..tile_px {
                img.put_pixel(px, py + i, grid_line);
                img.put_pixel(px + i, py, grid_line);
                img.put_pixel(px + tile_px - 1, py + i, grid_line);
                img.put_pixel(px + i, py + tile_px - 1, grid_line);
            }
        }
    }

    // Each 64×64 tile has a 3×3 mini-grid inside.
    // Cell size within tile: leave 2px padding, 3 cells of ~20px each
    let pad = 2u32;
    let cell_size = (tile_px - 2 * pad) / 3; // 20px per mini-cell
    let inner_pad = 1u32; // gap inside each mini-cell

    println!("Reference grid layout (tile index → canonical mask):");
    println!("  8×6 grid, 64×64 per tile, 512×384 total\n");

    for (i, &mask) in masks.iter().enumerate() {
        let tx = i as u32 % cols;
        let ty = i as u32 / cols;
        let tile_x = tx * tile_px;
        let tile_y = ty * tile_px;

        let cells = [
            (0, 0, mask & NW != 0),
            (1, 0, mask & N != 0),
            (2, 0, mask & NE != 0),
            (0, 1, mask & W != 0),
            (1, 1, true), // center always filled
            (2, 1, mask & E != 0),
            (0, 2, mask & SW != 0),
            (1, 2, mask & S != 0),
            (2, 2, mask & SE != 0),
        ];

        for &(cx, cy, is_filled) in &cells {
            let x0 = tile_x + pad + cx * cell_size + inner_pad;
            let y0 = tile_y + pad + cy * cell_size + inner_pad;
            let color = if is_filled { filled } else { empty_cell };
            for y in y0..y0 + cell_size - 2 * inner_pad {
                for x in x0..x0 + cell_size - 2 * inner_pad {
                    if x < img_w && y < img_h {
                        img.put_pixel(x, y, color);
                    }
                }
            }
            // Yellow dot on center cell
            if cx == 1 && cy == 1 {
                let mx = x0 + (cell_size - 2 * inner_pad) / 2;
                let my = y0 + (cell_size - 2 * inner_pad) / 2;
                for dy in 0..4u32 {
                    for dx in 0..4u32 {
                        img.put_pixel(mx - 2 + dx, my - 2 + dy, center_mark);
                    }
                }
            }
        }

        println!(
            "  tile ({tx},{ty}) idx {i:2} → mask {mask:3} (0b{mask:08b})"
        );
    }

    let path = "generated/blob47_reference.png";
    img.save(path).expect("Failed to save reference PNG");
    println!("\nSaved {path} ({img_w}×{img_h}, matching Wall_Concrete_1_64x64 layout)");
}

// ── Tile generation (for Step 2) ─────────────────────────────────────────

fn copy_quadrant(src: &RgbaImage, tile_col: u32, tile_row: u32, quad: usize) -> RgbaImage {
    let px = tile_col * TILE;
    let py = tile_row * TILE;
    let (qx, qy) = [(0, 0), (HALF, 0), (0, HALF), (HALF, HALF)][quad];
    imageops::crop_imm(src, px + qx, py + qy, HALF, HALF).to_image()
}

struct TileSetCoords {
    normal: [(u32, u32); 9],
    inverted: [(u32, u32); 9],
}

const IDX_TL: usize = 0;
const IDX_T: usize = 1;
const IDX_TR: usize = 2;
const IDX_L: usize = 3;
const IDX_C: usize = 4;
const IDX_R: usize = 5;
const IDX_BL: usize = 6;
const IDX_B: usize = 7;
const IDX_BR: usize = 8;

impl TileSetCoords {
    fn new(ox: u32, oy: u32) -> Self {
        let p = |dx: u32, dy: u32| (ox + dx, oy + dy);
        Self {
            normal: [
                p(0, 0), p(1, 0), p(2, 0),
                p(0, 1), p(1, 1), p(2, 1),
                p(0, 2), p(1, 2), p(2, 2),
            ],
            inverted: [
                p(3, 0), p(4, 0), p(5, 0),
                p(3, 1), p(4, 1), p(5, 1),
                p(3, 2), p(4, 2), p(5, 2),
            ],
        }
    }
}

fn generate_blob_tile(src: &RgbaImage, ts: &TileSetCoords, mask: u8) -> RgbaImage {
    let mut out = RgbaImage::new(TILE, TILE);
    let has = |bit: u8| mask & bit != 0;

    let (tc, tr) = match (has(N), has(W), has(NW)) {
        (false, false, _) => ts.normal[IDX_TL],
        (false, true, _) => ts.normal[IDX_T],
        (true, false, _) => ts.normal[IDX_L],
        (true, true, false) => ts.inverted[IDX_BR],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 0), 0, 0);

    let (tc, tr) = match (has(N), has(E), has(NE)) {
        (false, false, _) => ts.normal[IDX_TR],
        (false, true, _) => ts.normal[IDX_T],
        (true, false, _) => ts.normal[IDX_R],
        (true, true, false) => ts.inverted[IDX_BL],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 1), HALF as i64, 0);

    let (tc, tr) = match (has(S), has(W), has(SW)) {
        (false, false, _) => ts.normal[IDX_BL],
        (false, true, _) => ts.normal[IDX_B],
        (true, false, _) => ts.normal[IDX_L],
        (true, true, false) => ts.inverted[IDX_TR],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 2), 0, HALF as i64);

    let (tc, tr) = match (has(S), has(E), has(SE)) {
        (false, false, _) => ts.normal[IDX_BR],
        (false, true, _) => ts.normal[IDX_B],
        (true, false, _) => ts.normal[IDX_R],
        (true, true, false) => ts.inverted[IDX_TL],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 3), HALF as i64, HALF as i64);

    out
}

/// Derive layout from reference.ldtk by cross-referencing IntGrid with auto-tiles.
fn derive_layout_from_reference() -> Option<[u8; 47]> {
    let data = match std::fs::read_to_string("maps/reference.ldtk") {
        Ok(d) => d,
        Err(_) => {
            println!("maps/reference.ldtk not found — skipping tile generation.");
            return None;
        }
    };
    let json: serde_json::Value = serde_json::from_str(&data).expect("Failed to parse JSON");

    // Search all levels for the ReferenceIntGrid layer with auto-tiles
    let levels = json["levels"].as_array().unwrap();
    let layer = levels
        .iter()
        .flat_map(|level| level["layerInstances"].as_array().unwrap())
        .find(|l| {
            l["__identifier"].as_str() == Some("ReferenceIntGrid")
                && l["autoLayerTiles"]
                    .as_array()
                    .map_or(false, |a| !a.is_empty())
        })
        .expect("ReferenceIntGrid layer with auto-tiles not found in reference.ldtk");

    let grid_w = layer["__cWid"].as_u64().unwrap() as usize;
    let grid_h = layer["__cHei"].as_u64().unwrap() as usize;

    let intgrid: Vec<u8> = layer["intGridCsv"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();

    let cell_val = |col: i32, row: i32| -> u8 {
        if col < 0 || row < 0 || col >= grid_w as i32 || row >= grid_h as i32 {
            1
        } else {
            intgrid[row as usize * grid_w + col as usize].min(1)
        }
    };

    // Last entry per cell wins (highest priority)
    let mut cell_to_tile: BTreeMap<usize, u8> = BTreeMap::new();
    for tile_entry in layer["autoLayerTiles"].as_array().unwrap() {
        let t = tile_entry["t"].as_u64().unwrap() as u8;
        let d = tile_entry["d"].as_array().unwrap();
        let cell_idx = d[1].as_u64().unwrap() as usize;
        cell_to_tile.insert(cell_idx, t);
    }

    // Cross-reference: compute canonical bitmask for each filled cell
    let mut tile_to_mask: BTreeMap<u8, u8> = BTreeMap::new();
    for row in 0..grid_h {
        for col in 0..grid_w {
            let idx = row * grid_w + col;
            if intgrid[idx] == 0 {
                continue;
            }
            let c = col as i32;
            let r = row as i32;

            let mut mask: u8 = 0;
            if cell_val(c, r - 1) > 0 { mask |= N; }
            if cell_val(c + 1, r - 1) > 0 { mask |= NE; }
            if cell_val(c + 1, r) > 0 { mask |= E; }
            if cell_val(c + 1, r + 1) > 0 { mask |= SE; }
            if cell_val(c, r + 1) > 0 { mask |= S; }
            if cell_val(c - 1, r + 1) > 0 { mask |= SW; }
            if cell_val(c - 1, r) > 0 { mask |= W; }
            if cell_val(c - 1, r - 1) > 0 { mask |= NW; }

            let canonical = canonicalize(mask);

            if let Some(&tile) = cell_to_tile.get(&idx) {
                // For each tile, keep the mapping. If a tile maps to multiple
                // canonical masks, prefer the one with the most bits (most specific).
                tile_to_mask
                    .entry(tile)
                    .and_modify(|existing| {
                        if canonical.count_ones() > existing.count_ones() {
                            *existing = canonical;
                        }
                    })
                    .or_insert(canonical);
            }
        }
    }

    let mut layout = [255u8; 47];
    let mut assigned_masks: BTreeSet<u8> = BTreeSet::new();
    for (&tile, &mask) in &tile_to_mask {
        if (tile as usize) < 47 && !assigned_masks.contains(&mask) {
            layout[tile as usize] = mask;
            assigned_masks.insert(mask);
        }
    }

    let found = layout.iter().filter(|&&m| m != 255).count();
    println!("Cross-reference mapped {found}/47 tiles");

    if found < 47 {
        let all_canonical: BTreeSet<u8> = (0..=255u8).map(canonicalize).collect();
        let missing: Vec<u8> = all_canonical
            .iter()
            .filter(|m| !assigned_masks.contains(m))
            .copied()
            .collect();
        let unmapped: Vec<u8> = (0..47u8).filter(|t| layout[*t as usize] == 255).collect();
        println!("Missing masks: {missing:?}");
        println!("Unmapped tiles: {unmapped:?}");
        println!(
            "\nThe reference level needs all 47 configurations. \
             Use generated/blob47_reference.png as a guide."
        );
        return None;
    }

    Some(layout)
}

fn main() {
    // Always generate the reference PNG
    generate_reference_png();

    // Try to derive layout and generate tilesets
    let layout = match derive_layout_from_reference() {
        Some(l) => l,
        None => return,
    };

    let src = image::open("assets/external/kingdom-explorer/KE_Ground_Tiles.png")
        .expect("Failed to open KE_Ground_Tiles.png")
        .to_rgba8();

    println!("\nLDtk blob tile layout:");
    for (i, &mask) in layout.iter().enumerate() {
        let col = i as u32 % OUT_COLS;
        let row = i as u32 / OUT_COLS;
        println!("  tile {i:2} grid({col},{row}) → mask {mask:3} (0b{mask:08b})");
    }

    let sets = [
        ("blob47_set1", TileSetCoords::new(1, 1)),
        ("blob47_set2", TileSetCoords::new(8, 1)),
        ("blob47_set3", TileSetCoords::new(1, 4)),
        ("blob47_set4", TileSetCoords::new(8, 4)),
    ];

    let out_rows = (layout.len() as u32 + OUT_COLS - 1) / OUT_COLS;

    for (name, ts) in &sets {
        let mut output = RgbaImage::new(OUT_COLS * TILE, out_rows * TILE);
        for (i, &mask) in layout.iter().enumerate() {
            let tile = generate_blob_tile(&src, ts, mask);
            let col = i as u32 % OUT_COLS;
            let row = i as u32 / OUT_COLS;
            imageops::overlay(
                &mut output,
                &tile,
                (col * TILE) as i64,
                (row * TILE) as i64,
            );
        }
        let path = format!("generated/{name}.png");
        output.save(&path).expect("Failed to save output");
        println!("Saved {path}");
    }
}
