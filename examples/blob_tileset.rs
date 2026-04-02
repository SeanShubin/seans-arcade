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

use image::{imageops, imageops::FilterType, Rgba, RgbaImage};

const TILE: u32 = 16;
const HALF: u32 = 8;

const N: u8 = 1;
const NE: u8 = 2;
const E: u8 = 4;
const SE: u8 = 8;
const S: u8 = 16;
const SW: u8 = 32;
const W: u8 = 64;
const NW: u8 = 128;

/// LDtk blob autotile layout: 12×5 grid (768×320 at 64px, 192×80 at 16px).
/// Manually verified by mapping each rule's 3×3 pattern in the LDtk auto-rule
/// editor to its grid position. Column 5 is always blank (visual separator).
/// See docs/research/ldtk-blob47-layout.md for details.
///
/// Each entry is (col, row, canonical_bitmask). Bitmask bits (clockwise from N):
///   NW=128 N=1 NE=2 / W=64 E=4 / SW=32 S=16 SE=8
const LDTK_BLOB_LAYOUT: [(u32, u32, u8); 47] = [
    // Left section (cols 0–4), rows 0–4: outer corners, edges, composite edges
    (0, 0,  28), // E SE S
    (1, 0, 124), // W SW S SE E
    (2, 0, 112), // W SW S
    (3, 0,   4), // E
    (4, 0,  68), // W E
    (0, 1,  31), // N NE E SE S
    (1, 1, 255), // all
    (2, 1, 241), // N W S NW SW
    (3, 1,   0), // (none) isolated
    (4, 1,  16), // S
    (0, 2,   7), // N NE E
    (1, 2, 199), // W N E NW NE
    (2, 2, 193), // W N NW
    (3, 2,  85), // N S E W
    (4, 2,  17), // N S
    (0, 3, 127), // N S E W NE SW SE
    (1, 3, 253), // N S E W NW SW SE
    (2, 3,  20), // S E
    (3, 3,  80), // W S
    (4, 3,   1), // N
    (0, 4, 223), // N S E W NW NE SE
    (1, 4, 247), // N S E W NE NW SW
    (2, 4,   5), // N E
    (3, 4,  65), // W N
    // Right section (cols 6–11), rows 0–4: inner corners, 3-cardinal combos
    (5, 0,  64), // W
    (6, 0,  92), // W S SE E
    (7, 0, 116), // W SW S E
    (8, 0,  95), // W N NE E SE S
    (9, 0, 245), // N NW W SW S E
    (10,0,  93), // W N E S SE
    (11,0, 117), // N E S W SW
    (6, 1,  71), // W N E NE
    (7, 1, 197), // W N E NW
    (8, 1, 215), // N S E W NW NE
    (9, 1, 125), // N S E W SW SE
    (10,1,  87), // N S E W NE
    (11,1, 213), // N S E W NW
    (6, 2,  29), // N E S SE
    (7, 2, 113), // N W S SW
    (8, 2,  21), // N E S
    (9, 2,  84), // W S E
    (10,2, 119), // N S E W SW NE
    (6, 3,  23), // N E S NE
    (7, 3, 209), // N W S NW
    (8, 3,  69), // W N E
    (9, 3,  81), // N S W
    (10,3, 221), // N S E W NW SE
];

/// Build the 12×5 grid: grid[row][col] = Some(mask) or None for empty cells.
fn build_layout_grid() -> [[Option<u8>; 12]; 5] {
    let mut grid = [[None; 12]; 5];
    for &(col, row, mask) in &LDTK_BLOB_LAYOUT {
        grid[row as usize][col as usize] = Some(mask);
    }
    grid
}

/// Generate a reference PNG: 12×5 grid, 64×64 per tile, 768×320 total.
/// Each tile contains a miniature 3×3 diagram showing the neighbor pattern.
fn generate_reference_png() {
    let grid = build_layout_grid();
    let cols = 12u32;
    let rows = 5u32;
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

    println!("Reference grid layout ({cols}×{rows}):");
    println!("  64×64 per tile, {img_w}×{img_h} total\n");

    for &(col, row, mask) in &LDTK_BLOB_LAYOUT {
        let tile_idx = row * cols + col;
        println!("  tile {tile_idx:2} grid({col},{row}) → mask {mask:3} (0b{mask:08b})");
    }

    for row in 0..rows {
        for col in 0..cols {
            let mask = match grid[row as usize][col as usize] {
                Some(m) => m,
                None => continue,
            };
            let tx = col;
            let ty = row;
            let tile_x = tx * tile_px;
            let tile_y = ty * tile_px;

            let cells = [
                (0, 0, mask & NW != 0),
                (1, 0, mask & N != 0),
                (2, 0, mask & NE != 0),
                (0, 1, mask & W != 0),
                (1, 1, true),
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
        }
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

/// Generate a blob tile using junction extraction: assemble a 2×2 group of source
/// tiles and extract the 16×16 at their junction point. This uses the OPPOSITE
/// quadrant from each source tile (the one facing inward), so all 4 quadrants
/// meet at tile boundaries where seams are guaranteed smooth.
fn generate_blob_tile(src: &RgbaImage, ts: &TileSetCoords, mask: u8) -> RgbaImage {
    let mut out = RgbaImage::new(TILE, TILE);
    let has = |bit: u8| mask & bit != 0;

    // TL of output ← BR quadrant of the tile selected for the NW corner
    let (tc, tr) = match (has(N), has(W), has(NW)) {
        (false, false, _) => ts.normal[IDX_TL],
        (false, true, _) => ts.normal[IDX_T],
        (true, false, _) => ts.normal[IDX_L],
        (true, true, false) => ts.inverted[IDX_BR],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 3), 0, 0); // BR quad

    // TR of output ← BL quadrant of the tile selected for the NE corner
    let (tc, tr) = match (has(N), has(E), has(NE)) {
        (false, false, _) => ts.normal[IDX_TR],
        (false, true, _) => ts.normal[IDX_T],
        (true, false, _) => ts.normal[IDX_R],
        (true, true, false) => ts.inverted[IDX_BL],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 2), HALF as i64, 0); // BL quad

    // BL of output ← TR quadrant of the tile selected for the SW corner
    let (tc, tr) = match (has(S), has(W), has(SW)) {
        (false, false, _) => ts.normal[IDX_BL],
        (false, true, _) => ts.normal[IDX_B],
        (true, false, _) => ts.normal[IDX_L],
        (true, true, false) => ts.inverted[IDX_TR],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 1), 0, HALF as i64); // TR quad

    // BR of output ← TL quadrant of the tile selected for the SE corner
    let (tc, tr) = match (has(S), has(E), has(SE)) {
        (false, false, _) => ts.normal[IDX_BR],
        (false, true, _) => ts.normal[IDX_B],
        (true, false, _) => ts.normal[IDX_R],
        (true, true, false) => ts.inverted[IDX_TL],
        (true, true, true) => ts.normal[IDX_C],
    };
    imageops::overlay(&mut out, &copy_quadrant(src, tc, tr, 0), HALF as i64, HALF as i64); // TL quad

    out
}

fn main() {
    // Always generate the reference PNG
    generate_reference_png();

    let grid = build_layout_grid();

    let src = image::open("assets/external/kingdom-explorer/KE_Ground_Tiles.png")
        .expect("Failed to open KE_Ground_Tiles.png")
        .to_rgba8();

    let out_cols = 12u32;
    let out_rows = 5u32;

    let sets = [
        ("blob47_set1", TileSetCoords::new(1, 1)),
        ("blob47_set2", TileSetCoords::new(8, 1)),
        ("blob47_set3", TileSetCoords::new(1, 4)),
        ("blob47_set4", TileSetCoords::new(8, 4)),
    ];

    for (name, ts) in &sets {
        let mut output = RgbaImage::new(out_cols * TILE, out_rows * TILE);
        for row in 0..out_rows as usize {
            for col in 0..out_cols as usize {
                if let Some(mask) = grid[row][col] {
                    let tile = generate_blob_tile(&src, ts, mask);
                    imageops::overlay(
                        &mut output,
                        &tile,
                        (col as u32 * TILE) as i64,
                        (row as u32 * TILE) as i64,
                    );
                }
            }
        }
        let path = format!("generated/{name}.png");
        output.save(&path).expect("Failed to save output");
        println!("Saved {path}");
    }

    // ── Seam check: verify source tiles have compatible quadrants at 8×8 boundaries ─
    println!("\nSeam compatibility check (set 1):");
    let ts1 = TileSetCoords::new(1, 1);
    let pairs = [
        ("T_bl == C_tl", ts1.normal[IDX_T], 2, ts1.normal[IDX_C], 0),
        ("T_br == C_tr", ts1.normal[IDX_T], 3, ts1.normal[IDX_C], 1),
        ("L_tr == C_tl", ts1.normal[IDX_L], 1, ts1.normal[IDX_C], 0),
        ("L_br == C_bl", ts1.normal[IDX_L], 3, ts1.normal[IDX_C], 2),
        ("B_tl == C_bl", ts1.normal[IDX_B], 0, ts1.normal[IDX_C], 2),
        ("B_tr == C_br", ts1.normal[IDX_B], 1, ts1.normal[IDX_C], 3),
        ("R_tl == C_tr", ts1.normal[IDX_R], 0, ts1.normal[IDX_C], 1),
        ("R_bl == C_bl", ts1.normal[IDX_R], 2, ts1.normal[IDX_C], 2),
        ("TL_br == C_tl", ts1.normal[IDX_TL], 3, ts1.normal[IDX_C], 0),
        ("TL_tr == T_tl", ts1.normal[IDX_TL], 1, ts1.normal[IDX_T], 0),
        ("TL_bl == L_tl", ts1.normal[IDX_TL], 2, ts1.normal[IDX_L], 0),
    ];
    for (label, tile_a, quad_a, tile_b, quad_b) in &pairs {
        let a = copy_quadrant(&src, tile_a.0, tile_a.1, *quad_a);
        let b = copy_quadrant(&src, tile_b.0, tile_b.1, *quad_b);
        let mut diffs = 0u32;
        for y in 0..HALF {
            for x in 0..HALF {
                if a.get_pixel(x, y) != b.get_pixel(x, y) {
                    diffs += 1;
                }
            }
        }
        let status = if diffs == 0 { "✓ match" } else { "✗ MISMATCH" };
        println!("  {label}: {status} ({diffs} pixels differ)");
    }

    // ── Verification: scale up all sets and verify quarter-tile composition ────
    for (i, name) in ["set1", "set2", "set3", "set4"].iter().enumerate() {
        let set_src = [
            TileSetCoords::new(1, 1),
            TileSetCoords::new(8, 1),
            TileSetCoords::new(1, 4),
            TileSetCoords::new(8, 4),
        ];
        verify_set(&src, &grid, &set_src[i], name);
    }
}

/// Returns the expected source tile (col, row) and quadrant index for each of
/// the 4 quadrants (TL=0, TR=1, BL=2, BR=3) of a blob tile with the given mask.
fn expected_quadrant_sources(ts: &TileSetCoords, mask: u8) -> [(u32, u32, usize); 4] {
    let has = |bit: u8| mask & bit != 0;

    let tl = match (has(N), has(W), has(NW)) {
        (false, false, _) => (ts.normal[IDX_TL], 0),
        (false, true, _) => (ts.normal[IDX_T], 0),
        (true, false, _) => (ts.normal[IDX_L], 0),
        (true, true, false) => (ts.inverted[IDX_BR], 0),
        (true, true, true) => (ts.normal[IDX_C], 0),
    };
    let tr = match (has(N), has(E), has(NE)) {
        (false, false, _) => (ts.normal[IDX_TR], 1),
        (false, true, _) => (ts.normal[IDX_T], 1),
        (true, false, _) => (ts.normal[IDX_R], 1),
        (true, true, false) => (ts.inverted[IDX_BL], 1),
        (true, true, true) => (ts.normal[IDX_C], 1),
    };
    let bl = match (has(S), has(W), has(SW)) {
        (false, false, _) => (ts.normal[IDX_BL], 2),
        (false, true, _) => (ts.normal[IDX_B], 2),
        (true, false, _) => (ts.normal[IDX_L], 2),
        (true, true, false) => (ts.inverted[IDX_TR], 2),
        (true, true, true) => (ts.normal[IDX_C], 2),
    };
    let br = match (has(S), has(E), has(SE)) {
        (false, false, _) => (ts.normal[IDX_BR], 3),
        (false, true, _) => (ts.normal[IDX_B], 3),
        (true, false, _) => (ts.normal[IDX_R], 3),
        (true, true, false) => (ts.inverted[IDX_TL], 3),
        (true, true, true) => (ts.normal[IDX_C], 3),
    };

    [
        (tl.0 .0, tl.0 .1, tl.1),
        (tr.0 .0, tr.0 .1, tr.1),
        (bl.0 .0, bl.0 .1, bl.1),
        (br.0 .0, br.0 .1, br.1),
    ]
}

fn verify_set(
    src: &RgbaImage,
    grid: &[[Option<u8>; 12]; 5],
    ts: &TileSetCoords,
    name: &str,
) {
    let set_img = image::open(format!("generated/blob47_{name}.png"))
        .unwrap_or_else(|_| panic!("Failed to open blob47_{name}.png"))
        .to_rgba8();

    // Create 4x scaled version using nearest-neighbor
    let scaled = imageops::resize(
        &set_img,
        set_img.width() * 4,
        set_img.height() * 4,
        FilterType::Nearest,
    );
    let path_4x = format!("generated/blob47_{name}_4x.png");
    scaled.save(&path_4x).expect("Failed to save 4x image");
    println!("\nSaved {path_4x} ({}×{})", scaled.width(), scaled.height());

    // Verify all 47 tiles
    let mut verified = 0u32;
    let mut mismatches: Vec<String> = Vec::new();

    for row in 0..5usize {
        for col in 0..12usize {
            let mask = match grid[row][col] {
                Some(m) => m,
                None => continue,
            };
            let sources = expected_quadrant_sources(&ts, mask);

            let tile_x = col as u32 * TILE;
            let tile_y = row as u32 * TILE;

            let quad_names = ["TL", "TR", "BL", "BR"];
            let quad_offsets: [(u32, u32); 4] = [(0, 0), (HALF, 0), (0, HALF), (HALF, HALF)];

            let mut tile_ok = true;

            for q in 0..4 {
                let (src_col, src_row, src_quad) = sources[q];
                let (qx, qy) = quad_offsets[q];

                // Expected quadrant from source image
                let expected = copy_quadrant(src, src_col, src_row, src_quad);

                // Actual quadrant from generated tile
                let actual = imageops::crop_imm(&set_img, tile_x + qx, tile_y + qy, HALF, HALF)
                    .to_image();

                // Pixel-by-pixel comparison
                let mut pixel_mismatches = 0u32;
                for py in 0..HALF {
                    for px in 0..HALF {
                        if expected.get_pixel(px, py) != actual.get_pixel(px, py) {
                            pixel_mismatches += 1;
                        }
                    }
                }

                if pixel_mismatches > 0 {
                    tile_ok = false;
                    mismatches.push(format!(
                        "  Tile mask=0b{mask:08b} grid({col},{row}) quad {}: \
                         {pixel_mismatches} pixel(s) differ (expected src tile ({src_col},{src_row}) quad {})",
                        quad_names[q], quad_names[src_quad],
                    ));
                }
            }

            if tile_ok {
                verified += 1;
            }
        }
    }

    if mismatches.is_empty() {
        println!("Verified {verified}/47 tiles correct");
    } else {
        println!(
            "\nVerification: {verified}/47 tiles correct, {} with mismatches:",
            47 - verified
        );
        for m in &mismatches {
            println!("{m}");
        }
    }
}
