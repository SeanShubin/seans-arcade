//! Maze with keys and gates (wrapping topology, edge-based walls).
//!
//! Graph-paper style: every cell is a corridor, walls are thin 16px lines
//! between cells. DFS carves passages within each region. Gates at region
//! boundaries require keys to open. Maze wraps in all directions (torus).
//!
//! Controls:
//!   Arrow keys / WASD / D-pad / left stick: move
//!   N: new maze (random seed)
//!   R: restart current maze
//!   +/-: increase/decrease number of keys (regenerates maze)
//!   [/]: decrease/increase viewable area (odd, square)
//!
//! Run with: `cargo run --example maze`

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::{Rng, RngExt, SeedableRng};
use std::collections::{HashSet, VecDeque};
use std::io::Write;

// ---------------------------------------------------------------------------
// Maze design constraints
// ---------------------------------------------------------------------------
//
// Topology:     Torus — maze wraps in all directions, no edges.
// Structure:    Graph-paper style — every cell is a corridor, walls are thin
//               lines between cells. DFS carves passages within each region.
// Dependency:   Branching tree (max branching factor MAX_BRANCH). Each key's
//               region seeds from its parent's region. Collecting a key opens
//               gates to all of that key's children's regions.
// Key access:   Root keys are freely reachable from the start region.
//               Non-root keys require the parent key to reach.
// Key placement: Each key is in its own region (region ki+1).
// Goal:         Behind a corridor of N single-key gates (one per key, shuffled
//               order). Topology enforces that ALL keys are needed — the player
//               must pass through every gate in the corridor to reach the goal.
// Gates:        Each gate requires exactly one key. Tree gates give access to
//               key regions. Corridor gates guard the path to the goal.
//               Root keys have no tree gate — their regions connect to region 0.
// Solvability:  Guaranteed by construction. No retry loop.

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Maze geometry
const WALL_PX: usize = 16;      // wall thickness in pixels
const CORR_PX: usize = 64;      // corridor width in pixels
const CELL: usize = WALL_PX + CORR_PX; // total cell pitch (80px)

// Puzzle
const MAX_KEYS: usize = 9;      // maximum number of keys
const DEFAULT_KEYS: usize = 9;  // starting key count

// Tuning defaults
const DEFAULT_BRANCH: usize = 2;

#[derive(Clone, Copy, PartialEq, Resource)]
enum GraphShape { Linear, Branching, Hub }

impl GraphShape {
    fn cycle(self) -> Self {
        match self { Self::Linear => Self::Branching, Self::Branching => Self::Hub, Self::Hub => Self::Linear }
    }
    fn label(self) -> &'static str {
        match self { Self::Linear => "LINEAR", Self::Branching => "BRANCHING", Self::Hub => "HUB" }
    }
}

#[derive(Clone, Copy, PartialEq, Resource)]
enum RegionSize { Small, Medium, Large }

impl RegionSize {
    fn cells(self) -> usize {
        match self { Self::Small => 4, Self::Medium => 8, Self::Large => 16 }
    }
    fn cycle(self) -> Self {
        match self { Self::Small => Self::Medium, Self::Medium => Self::Large, Self::Large => Self::Small }
    }
    fn label(self) -> &'static str {
        match self { Self::Small => "SMALL", Self::Medium => "MEDIUM", Self::Large => "LARGE" }
    }
}

#[derive(Clone, Copy, PartialEq, Resource)]
enum DeadEndLevel { None, Low, High }

impl DeadEndLevel {
    fn ratio(self) -> f64 {
        match self { Self::None => 0.0, Self::Low => 0.3, Self::High => 0.6 }
    }
    fn cycle(self) -> Self {
        match self { Self::None => Self::Low, Self::Low => Self::High, Self::High => Self::None }
    }
    fn label(self) -> &'static str {
        match self { Self::None => "NONE", Self::Low => "LOW", Self::High => "HIGH" }
    }
}

// Viewport
const DEFAULT_VIEW_CELLS: usize = 5;
const MIN_VIEW_CELLS: usize = 3;
const MAX_VIEW_CELLS: usize = 15;

// Player
const MOVE_SPEED: f32 = 640.0;  // pixels per second
const STICK_DEAD: f32 = 0.3;    // gamepad dead zone

// Rendering
const KEY_SPRITE_SIZE: usize = 64;
const ITEM_SIZE: usize = CORR_PX / 2;
const INV_ROWS: usize = 3;      // max keys per inventory column
const INV_COL_W: usize = KEY_SPRITE_SIZE + WALL_PX;
const GAP_W: usize = CELL;      // gap between inventory and maze

/// Inventory panel width for a given key count.
fn inv_w(num_keys: usize) -> usize {
    let cols = (num_keys.max(1) + INV_ROWS - 1) / INV_ROWS; // ceil div
    WALL_PX + cols * INV_COL_W
}

// ---------------------------------------------------------------------------
// Key sprite sheet
// ---------------------------------------------------------------------------

const SHEET_COLS: usize = 14;
const SHEET_W: usize = SHEET_COLS * KEY_SPRITE_SIZE;
const KEYS_PER_COLOR: usize = 20;

// Per-channel median of non-transparent pixels from Fantasy Keys-sheet.png.
// Each color sampled across all 20 key designs for that category.
const KEY_COLORS: [[u8; 4]; 9] = [
    [ 73,  49,  38, 255], // 0: Bronze
    [ 61,  62,  78, 255], // 1: Silver
    [173, 169, 182, 255], // 2: White
    [ 31,  28,  37, 255], // 3: Black
    [148,  22,  34, 255], // 4: Red
    [114,  76,  42, 255], // 5: Gold
    [ 39,  96,  37, 255], // 6: Green
    [ 34,  79, 146, 255], // 7: Blue
    [ 63,  31, 100, 255], // 8: Purple
];

fn key_sheet_pos(color: usize, design: usize) -> (usize, usize) {
    let linear = color * KEYS_PER_COLOR + design;
    (linear % SHEET_COLS, linear / SHEET_COLS)
}

// Colors
const COL_BG: [u8; 4] = [0, 0, 0, 255];
const COL_GAP: [u8; 4] = [48, 48, 48, 255];
const COL_WALL: [u8; 4] = [160, 160, 160, 255];
const COL_PLAYER: [u8; 4] = [0, 220, 0, 255];
const COL_GOAL: [u8; 4] = [255, 255, 0, 255];
const COL_DIM: [u8; 4] = [50, 50, 50, 255];
const COL_WIN: [u8; 4] = [255, 255, 255, 255];
const COL_LABEL: [u8; 4] = [180, 180, 180, 255];
const COL_VALUE: [u8; 4] = [255, 255, 255, 255];

// Legend layout
const FONT_SCALE: usize = 2;       // each font pixel = 2×2 screen pixels
const GLYPH_W: usize = 4;          // raw glyph width (3 px + 1 spacing)
const GLYPH_H: usize = 6;          // raw glyph height (5 px + 1 spacing)
const CHAR_W: usize = GLYPH_W * FONT_SCALE;  // 8
const CHAR_H: usize = GLYPH_H * FONT_SCALE;  // 12
const LEGEND_PAD: usize = WALL_PX;
const LEGEND_LINE_H: usize = CHAR_H + 2;
const LEGEND_LINES: usize = 10;
const LEGEND_GAP: usize = GAP_W;  // visual gap between content and legend
const LEGEND_BODY_H: usize = LEGEND_PAD + LEGEND_LINES * LEGEND_LINE_H + LEGEND_PAD;
const LEGEND_H: usize = LEGEND_GAP + LEGEND_BODY_H;

// ---------------------------------------------------------------------------
// Tiny 4×5 pixel font (3px wide + 1 spacing, 5px tall)
// ---------------------------------------------------------------------------
// Each glyph is 5 rows of 3 bits, packed into a u16 (low 15 bits).
// Row 0 = bits 14..12, row 1 = bits 11..9, etc.
fn glyph_bits(ch: char) -> u16 {
    match ch {
        'A' => 0b_010_111_101_111_101,
        'B' => 0b_110_101_110_101_110,
        'C' => 0b_011_100_100_100_011,
        'D' => 0b_110_101_101_101_110,
        'E' => 0b_111_100_110_100_111,
        'F' => 0b_111_100_110_100_100,
        'G' => 0b_011_100_101_101_011,
        'H' => 0b_101_101_111_101_101,
        'I' => 0b_111_010_010_010_111,
        'J' => 0b_001_001_001_101_010,
        'K' => 0b_101_110_100_110_101,
        'L' => 0b_100_100_100_100_111,
        'M' => 0b_101_111_111_101_101,
        'N' => 0b_101_111_111_111_101,
        'O' => 0b_010_101_101_101_010,
        'P' => 0b_110_101_110_100_100,
        'Q' => 0b_010_101_101_110_011,
        'R' => 0b_110_101_110_101_101,
        'S' => 0b_011_100_010_001_110,
        'T' => 0b_111_010_010_010_010,
        'U' => 0b_101_101_101_101_010,
        'V' => 0b_101_101_101_010_010,
        'W' => 0b_101_101_111_111_101,
        'X' => 0b_101_101_010_101_101,
        'Y' => 0b_101_101_010_010_010,
        'Z' => 0b_111_001_010_100_111,
        '0' => 0b_010_101_101_101_010,
        '1' => 0b_010_110_010_010_111,
        '2' => 0b_110_001_010_100_111,
        '3' => 0b_110_001_010_001_110,
        '4' => 0b_101_101_111_001_001,
        '5' => 0b_111_100_110_001_110,
        '6' => 0b_011_100_111_101_011,
        '7' => 0b_111_001_010_010_010,
        '8' => 0b_010_101_010_101_010,
        '9' => 0b_110_101_111_001_110,
        ':' => 0b_000_010_000_010_000,
        '/' => 0b_001_001_010_100_100,
        '+' => 0b_000_010_111_010_000,
        '-' => 0b_000_000_111_000_000,
        '[' => 0b_011_010_010_010_011,
        ']' => 0b_110_010_010_010_110,
        '=' => 0b_000_111_000_111_000,
        '.' => 0b_000_000_000_000_010,
        'x' => 0b_000_101_010_101_000, // lowercase x for dimensions
        _ => 0,
    }
}

fn draw_char(data: &mut [u8], img_w: usize, img_h: usize, ch: char, x0: usize, y0: usize, col: [u8; 4]) {
    let bits = glyph_bits(ch);
    if bits == 0 && ch != ' ' { return; }
    for row in 0..5 {
        for c in 0..3 {
            if bits & (1 << (14 - row * 3 - c)) != 0 {
                for sy in 0..FONT_SCALE {
                    for sx in 0..FONT_SCALE {
                        let px = x0 + c * FONT_SCALE + sx;
                        let py = y0 + row * FONT_SCALE + sy;
                        if px < img_w && py < img_h {
                            let i = (py * img_w + px) * 4;
                            data[i..i + 4].copy_from_slice(&col);
                        }
                    }
                }
            }
        }
    }
}

fn draw_text(data: &mut [u8], img_w: usize, img_h: usize, text: &str, x0: usize, y0: usize, col: [u8; 4]) {
    for (i, ch) in text.chars().enumerate() {
        draw_char(data, img_w, img_h, ch, x0 + i * CHAR_W, y0, col);
    }
}

// ---------------------------------------------------------------------------
// Layout helpers
// ---------------------------------------------------------------------------

/// Compute output image dimensions from view_cells and num_keys.
fn image_dims(view_cells: usize, num_keys: usize) -> (usize, usize) {
    let view_px = view_cells * CELL;
    let rows = num_keys.max(1).min(INV_ROWS);
    let inv_content_h = rows * KEY_SPRITE_SIZE + (rows + 1) * WALL_PX;
    let content_h = view_px.max(inv_content_h);
    let img_w = inv_w(num_keys) + GAP_W + view_px;
    let img_h = content_h + LEGEND_H;
    (img_w, img_h)
}

// ---------------------------------------------------------------------------
// Maze generation
// ---------------------------------------------------------------------------

/// Grid side length in cells.
/// Regions: 1 start + num_keys key regions + num_keys corridor regions + 1 goal.
fn grid_side(num_keys: usize, region_cells: usize) -> usize {
    let num_regions = 1 + num_keys + num_keys + 1; // start + keys + corridor + goal
    // 50% padding ensures seeding has room to grow parents without
    // stealing other regions' only cells.
    let target = num_regions * region_cells * 3 / 2;
    let mut s = 2usize;
    while s * s < target { s += 1; }
    s
}

/// A gate blocks a wall between two adjacent cells at a region boundary.
#[derive(Clone)]
struct Gate {
    cells: [(usize, usize); 2],
    key_index: usize,
}

#[derive(Resource)]
struct MazeLayout {
    seed: u64,
    num_keys: usize,
    cols: usize,
    rows: usize,
    maze_w: usize,
    maze_h: usize,
    h_walls: Vec<bool>,
    v_walls: Vec<bool>,
    #[allow(dead_code)]
    cell_regions: Vec<usize>,
    /// Dependency tree: parent[ki] = parent key index, or usize::MAX for root keys.
    /// Collecting key parent[ki] opens the gate into region ki+1.
    key_parents: Vec<usize>,
    gates: Vec<Gate>,
    start: (usize, usize),
    goal: (usize, usize),
    key_cells: Vec<(usize, usize)>,
    key_designs: Vec<usize>,
}

fn remove_wall(
    h_walls: &mut [bool], v_walls: &mut [bool],
    cols: usize, rows: usize,
    cx: usize, cy: usize, nx: usize, ny: usize,
) {
    if ny == cy {
        if (cx + 1) % cols == nx {
            h_walls[cy * cols + cx] = false;
        } else {
            h_walls[cy * cols + nx] = false;
        }
    } else {
        if (cy + 1) % rows == ny {
            v_walls[cy * cols + cx] = false;
        } else {
            v_walls[ny * cols + nx] = false;
        }
    }
}

/// DFS maze carve within a single region. Guarantees all cells of
/// the region are connected by passages (spanning tree).
fn maze_dfs_region(
    h_walls: &mut [bool], v_walls: &mut [bool],
    cols: usize, rows: usize,
    cell_regions: &[usize], region: usize,
    start: (usize, usize),
    rng: &mut impl Rng,
) {
    let mut visited = vec![false; rows * cols];
    visited[start.1 * cols + start.0] = true;
    let mut stack = vec![start];

    while let Some(&(cx, cy)) = stack.last() {
        let mut neighbors = Vec::new();
        for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
            let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
            if cell_regions[ny * cols + nx] == region && !visited[ny * cols + nx] {
                neighbors.push((nx, ny));
            }
        }
        if neighbors.is_empty() {
            stack.pop();
            continue;
        }
        let (nx, ny) = neighbors[rng.random_range(0..neighbors.len())];
        remove_wall(h_walls, v_walls, cols, rows, cx, cy, nx, ny);
        visited[ny * cols + nx] = true;
        stack.push((nx, ny));
    }
}

fn find_boundary_edges(
    cell_regions: &[usize], cols: usize, rows: usize,
    region_a: usize, region_b: usize,
) -> Vec<[(usize, usize); 2]> {
    let mut edges = Vec::new();
    for cy in 0..rows {
        for cx in 0..cols {
            let r = cell_regions[cy * cols + cx];
            let nx = (cx + 1) % cols;
            if (r == region_a && cell_regions[cy * cols + nx] == region_b)
                || (r == region_b && cell_regions[cy * cols + nx] == region_a)
            {
                edges.push([(cx, cy), (nx, cy)]);
            }
            let ny = (cy + 1) % rows;
            if (r == region_a && cell_regions[ny * cols + cx] == region_b)
                || (r == region_b && cell_regions[ny * cols + cx] == region_a)
            {
                edges.push([(cx, cy), (cx, ny)]);
            }
        }
    }
    edges
}

/// Squared toroidal distance between two cell coordinates.
fn torus_dist_sq(a: (usize, usize), b: (usize, usize), cols: usize, rows: usize) -> usize {
    let dx = {
        let d = (a.0 as i32 - b.0 as i32).unsigned_abs() as usize;
        d.min(cols - d)
    };
    let dy = {
        let d = (a.1 as i32 - b.1 as i32).unsigned_abs() as usize;
        d.min(rows - d)
    };
    dx * dx + dy * dy
}

/// Pick the boundary edge farthest from existing gates of the same color.
fn pick_spaced_gate_edge(
    boundary: &[[(usize, usize); 2]],
    gates: &[Gate],
    key_index: usize,
    cols: usize, rows: usize,
    rng: &mut impl Rng,
) -> [(usize, usize); 2] {
    let same_color: Vec<(usize, usize)> = gates.iter()
        .filter(|g| g.key_index == key_index)
        .map(|g| {
            // midpoint of the two gate cells
            let mid_x = (g.cells[0].0 + g.cells[1].0) / 2;
            let mid_y = (g.cells[0].1 + g.cells[1].1) / 2;
            (mid_x, mid_y)
        })
        .collect();
    if same_color.is_empty() {
        return boundary[rng.random_range(0..boundary.len())];
    }
    // Score each boundary edge by minimum distance to same-color gates
    let mut best_score = 0usize;
    let mut best: Vec<usize> = Vec::new();
    for (i, edge) in boundary.iter().enumerate() {
        let mid = ((edge[0].0 + edge[1].0) / 2, (edge[0].1 + edge[1].1) / 2);
        let min_dist = same_color.iter()
            .map(|&sc| torus_dist_sq(mid, sc, cols, rows))
            .min().unwrap();
        if min_dist > best_score {
            best_score = min_dist;
            best = vec![i];
        } else if min_dist == best_score {
            best.push(i);
        }
    }
    boundary[best[rng.random_range(0..best.len())]]
}

/// Open extra passages within regions to reduce dead-ends.
/// At ratio 0.0, opens all internal walls (no dead-ends).
/// At ratio 0.6, opens only 40% (most DFS dead-ends remain).
fn open_extra_passages(
    h_walls: &mut [bool], v_walls: &mut [bool],
    cols: usize, rows: usize,
    cell_regions: &[usize], num_regions: usize,
    dead_end_ratio: f64,
    rng: &mut impl Rng,
) {
    for region in 0..num_regions {
        // Collect internal walls still closed within this region
        let mut internal: Vec<(usize, usize, usize, usize)> = Vec::new();
        for cy in 0..rows {
            for cx in 0..cols {
                if cell_regions[cy * cols + cx] != region { continue; }
                let nx = (cx + 1) % cols;
                if cell_regions[cy * cols + nx] == region && h_walls[cy * cols + cx] {
                    internal.push((cx, cy, nx, cy));
                }
                let ny = (cy + 1) % rows;
                if cell_regions[ny * cols + cx] == region && v_walls[cy * cols + cx] {
                    internal.push((cx, cy, cx, ny));
                }
            }
        }
        // Shuffle
        for i in (1..internal.len()).rev() {
            let j = rng.random_range(0..=i);
            internal.swap(i, j);
        }
        // Open (1 - ratio) fraction of them
        let open_count = ((1.0 - dead_end_ratio) * internal.len() as f64) as usize;
        for &(cx, cy, nx, ny) in internal.iter().take(open_count) {
            remove_wall(h_walls, v_walls, cols, rows, cx, cy, nx, ny);
        }
    }
}

/// Build a random dependency tree for `num_keys` keys.
/// Returns `parent[ki]` = parent key index, or `usize::MAX` for root keys.
fn build_dependency_tree(num_keys: usize, shape: GraphShape, max_branch: usize, rng: &mut impl Rng) -> Vec<usize> {
    if num_keys == 0 { return vec![]; }
    let mut parent = vec![usize::MAX; num_keys];
    match shape {
        GraphShape::Linear => {
            for ki in 1..num_keys { parent[ki] = ki - 1; }
        }
        GraphShape::Hub => {
            // All keys are roots — every key region connects directly to start
        }
        GraphShape::Branching => {
            let mut children_count = vec![0usize; num_keys];
            let cap = max_branch.min(num_keys.saturating_sub(1).max(1));
            for ki in 1..num_keys {
                let eligible: Vec<usize> = (0..ki)
                    .filter(|&p| children_count[p] < cap)
                    .collect();
                // If all parents are full, fall back to chain (parent = ki-1)
                let p = if eligible.is_empty() {
                    ki - 1
                } else {
                    eligible[rng.random_range(0..eligible.len())]
                };
                parent[ki] = p;
                children_count[p] += 1;
            }
        }
    }
    parent
}

/// Generate a maze that is solvable by construction. No retry loop.
///
/// Region layout:
///   Region 0            = start (center of map)
///   Regions 1..num_keys = one per key (branching tree off region 0)
///   Regions C0..C(N-1)  = goal corridor (one gate per key, linear chain)
///   Region G            = goal (end of corridor)
///
/// The branching tree gives non-linear key discovery. The goal corridor
/// ensures ALL keys are needed: it's a linear chain of single-key gates,
/// one per key, leading to the goal. Topology enforces the constraint.
fn generate_maze(seed: u64, num_keys: usize, config: &MazeConfig) -> MazeLayout {
    let _ = std::fs::create_dir_all("logs");
    let mut log = std::fs::File::create("logs/maze_gen.log")
        .expect("failed to create logs/maze_gen.log");

    macro_rules! log {
        ($($arg:tt)*) => {{
            let _ = writeln!(log, $($arg)*);
            let _ = log.flush();
        }};
    }

    let side = grid_side(num_keys, config.region_size.cells());
    let cols = side;
    let rows = side;
    let n = cols * rows;
    let maze_w = cols * CELL;
    let maze_h = rows * CELL;

    log!("=== MAZE GENERATION ===");
    log!("seed: {seed}");
    log!("num_keys: {num_keys}");
    log!("graph_shape: {}", config.graph_shape.label());
    log!("region_size: {} ({})", config.region_size.label(), config.region_size.cells());
    log!("dead_ends: {}", config.dead_ends.label());
    log!("max_branch: {}", config.max_branch);
    log!("grid: {cols}x{rows} = {n} cells");
    log!("maze_px: {maze_w}x{maze_h}");

    // Region indices:
    //   0           = start
    //   1..=nk      = key regions (key ki is in region ki+1)
    //   nk+1..=2*nk = corridor regions (one per key)
    //   2*nk+1      = goal region
    let nk = num_keys;
    let corridor_base = nk + 1;          // first corridor region index
    let goal_region = 2 * nk + 1;        // goal region index
    let num_regions = goal_region + 1;    // total regions

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);

    log!("");
    log!("num_regions: {num_regions} (start=0, keys=1..={nk}, corridor={corridor_base}..={}, goal={goal_region})",
        corridor_base + nk.saturating_sub(1));

    // --- Step 1: Build dependency tree for key discovery ---
    let key_parents = build_dependency_tree(num_keys, config.graph_shape, config.max_branch, &mut rng);

    log!("");
    log!("--- Step 1: Dependency tree ---");
    for ki in 0..nk {
        if key_parents[ki] == usize::MAX {
            log!("  key {ki}: root (free from start)");
        } else {
            log!("  key {ki}: parent=key {}", key_parents[ki]);
        }
    }

    // --- Step 2: Assign regions at cell level ---
    let cells_per_region = n / num_regions;
    let extra = n % num_regions;
    let target_sizes: Vec<usize> = (0..num_regions)
        .map(|r| cells_per_region + if r < extra { 1 } else { 0 })
        .collect();

    let mut cell_regions = vec![usize::MAX; n];
    let center = (cols / 2, rows / 2);
    cell_regions[center.1 * cols + center.0] = 0;
    let mut region_cells_list: Vec<Vec<(usize, usize)>> = vec![Vec::new(); num_regions];
    region_cells_list[0].push(center);

    // Build a seeding schedule: (child_region, parent_region) pairs in order.
    // Key regions seed from their parent's region (BFS tree order).
    // Corridor regions form a linear chain from region 0.
    // Goal seeds from the last corridor region.
    let mut seed_schedule: Vec<(usize, usize)> = Vec::new();

    // Key regions in BFS tree order
    {
        let mut bfs_queue = VecDeque::new();
        for ki in 0..nk {
            if key_parents[ki] == usize::MAX { bfs_queue.push_back(ki); }
        }
        while let Some(ki) = bfs_queue.pop_front() {
            let parent_region = if key_parents[ki] == usize::MAX { 0 } else { key_parents[ki] + 1 };
            seed_schedule.push((ki + 1, parent_region));
            for chi in 0..nk {
                if key_parents[chi] == ki { bfs_queue.push_back(chi); }
            }
        }
    }

    // Corridor regions: chain from region 0
    seed_schedule.push((corridor_base, 0));
    for ci in 1..nk {
        seed_schedule.push((corridor_base + ci, corridor_base + ci - 1));
    }

    // Goal region from last corridor (or region 0 if no keys)
    let last_corridor = if nk > 0 { corridor_base + nk - 1 } else { 0 };
    seed_schedule.push((goal_region, last_corridor));

    log!("");
    log!("--- Step 2: Region assignment ---");
    log!("cells_per_region (target): {cells_per_region}");
    log!("target_sizes: {:?}", target_sizes);
    log!("seed_schedule ({} entries): {:?}", seed_schedule.len(), seed_schedule);

    // Seed each child region with one cell adjacent to its parent.
    // Process in dependency order so parents exist before children.
    // If the parent has no free neighbors, grow it by one cell first.
    for &(child, parent) in &seed_schedule {
        loop {
            // Collect unassigned cells adjacent to the parent region.
            let mut candidates: Vec<(usize, usize)> = Vec::new();
            for &(cx, cy) in &region_cells_list[parent] {
                for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
                    let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
                    if cell_regions[ny * cols + nx] == usize::MAX {
                        candidates.push((nx, ny));
                    }
                }
            }
            if !candidates.is_empty() {
                let pick = candidates[rng.random_range(0..candidates.len())];
                cell_regions[pick.1 * cols + pick.0] = child;
                region_cells_list[child].push(pick);
                break;
            }
            // Parent is surrounded — grow it by claiming one neighbor from
            // an adjacent region that still has unassigned neighbors itself,
            // so we don't strand that region.
            let mut grew = false;
            'outer: for &(cx, cy) in &region_cells_list[parent].clone() {
                for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
                    let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
                    let ni = ny * cols + nx;
                    let nr = cell_regions[ni];
                    if nr != usize::MAX && nr != parent && nr != child
                        && region_cells_list[nr].len() > 1
                    {
                        cell_regions[ni] = parent;
                        region_cells_list[parent].push((nx, ny));
                        region_cells_list[nr].retain(|&c| c != (nx, ny));
                        grew = true;
                        break 'outer;
                    }
                }
            }
            if !grew {
                // Last resort: grab any unassigned cell (may be disconnected,
                // mop-up phase will clean up).
                if let Some(i) = (0..n).find(|&i| cell_regions[i] == usize::MAX) {
                    let cx = i % cols;
                    let cy = i / cols;
                    cell_regions[i] = parent;
                    region_cells_list[parent].push((cx, cy));
                } else {
                    break; // grid full — shouldn't happen
                }
            }
        }
    }

    log!("seeding complete — region sizes after seeding:");
    for r in 0..num_regions {
        log!("  region {r}: {} cells", region_cells_list[r].len());
    }

    // Grow all regions round-robin
    let mut any_grew = true;
    while any_grew {
        any_grew = false;
        for region in 0..num_regions {
            if region_cells_list[region].len() >= target_sizes[region] { continue; }
            let mut frontier = Vec::new();
            for &(cx, cy) in &region_cells_list[region] {
                for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
                    let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
                    let ni = ny * cols + nx;
                    if cell_regions[ni] == usize::MAX && !frontier.contains(&(nx, ny)) {
                        frontier.push((nx, ny));
                    }
                }
            }
            if frontier.is_empty() { continue; }
            let pick = frontier[rng.random_range(0..frontier.len())];
            cell_regions[pick.1 * cols + pick.0] = region;
            region_cells_list[region].push(pick);
            any_grew = true;
        }
    }

    // Mop up remaining cells
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            if cell_regions[i] != usize::MAX { continue; }
            let cx = i % cols;
            let cy = i / cols;
            for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
                let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
                let ni = ny * cols + nx;
                if cell_regions[ni] != usize::MAX {
                    let r = cell_regions[ni];
                    cell_regions[i] = r;
                    region_cells_list[r].push((cx, cy));
                    changed = true;
                    break;
                }
            }
        }
    }

    let unassigned = cell_regions.iter().filter(|&&r| r == usize::MAX).count();
    log!("after growth+mopup: {unassigned} unassigned cells remain");
    log!("final region sizes:");
    for r in 0..num_regions {
        log!("  region {r}: {} cells (target {})", region_cells_list[r].len(), target_sizes[r]);
    }

    // --- Step 3: Ensure required boundaries exist ---
    log!("");
    log!("--- Step 3: Ensure required boundaries ---");
    // First, collect all (region_a, region_b) pairs that need a shared boundary.
    let mut required_boundaries: Vec<(usize, usize)> = Vec::new();
    for ki in 0..nk {
        let parent_region = if key_parents[ki] == usize::MAX { 0 } else { key_parents[ki] + 1 };
        required_boundaries.push((parent_region, ki + 1));
    }
    required_boundaries.push((0, corridor_base));
    for ci in 1..nk {
        required_boundaries.push((corridor_base + ci - 1, corridor_base + ci));
    }
    let last_corridor = if nk > 0 { corridor_base + nk - 1 } else { 0 };
    required_boundaries.push((last_corridor, goal_region));

    log!("required_boundaries: {:?}", required_boundaries);

    // Ensure each required pair shares at least one boundary edge.
    // If not, BFS from region_b toward region_a, converting cells along the
    // shortest path until they touch. Bounded: path length ≤ grid diameter.
    for &(ra, rb) in &required_boundaries {
        let existing = find_boundary_edges(&cell_regions, cols, rows, ra, rb);
        if !existing.is_empty() {
            log!("  boundary ({ra}, {rb}): OK ({} edges)", existing.len());
            continue;
        }
        log!("  boundary ({ra}, {rb}): MISSING — fixing via BFS (ra has {} cells, rb has {} cells)",
            region_cells_list[ra].len(), region_cells_list[rb].len());
        // BFS from all cells of rb to find the nearest cell of ra.
        let mut dist = vec![usize::MAX; n];
        let mut prev = vec![usize::MAX; n];
        let mut queue = VecDeque::new();
        for &(cx, cy) in &region_cells_list[rb] {
            let idx = cy * cols + cx;
            dist[idx] = 0;
            queue.push_back(idx);
        }
        let mut target = usize::MAX;
        while let Some(cur) = queue.pop_front() {
            let cx = cur % cols;
            let cy = cur / cols;
            if cell_regions[cur] == ra {
                target = cur;
                break;
            }
            for (ddx, ddy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let nx = (cx as i32 + ddx).rem_euclid(cols as i32) as usize;
                let ny = (cy as i32 + ddy).rem_euclid(rows as i32) as usize;
                let ni = ny * cols + nx;
                if dist[ni] == usize::MAX {
                    dist[ni] = dist[cur] + 1;
                    prev[ni] = cur;
                    queue.push_back(ni);
                }
            }
        }
        // Walk the path back from target, converting cells to rb until we
        // reach a cell already in rb. The last converted cell is adjacent to ra.
        if target != usize::MAX {
            let mut converted = 0usize;
            let mut cur = prev[target]; // skip the ra cell itself
            while cur != usize::MAX && cell_regions[cur] != rb {
                let old_region = cell_regions[cur];
                cell_regions[cur] = rb;
                let cx = cur % cols;
                let cy = cur / cols;
                region_cells_list[rb].push((cx, cy));
                if old_region != usize::MAX {
                    region_cells_list[old_region].retain(|&c| c != (cx, cy));
                }
                converted += 1;
                cur = prev[cur];
            }
            log!("    converted {converted} cells to region {rb} (BFS dist={})", dist[target]);
            let fixed = find_boundary_edges(&cell_regions, cols, rows, ra, rb);
            log!("    boundary now has {} edges", fixed.len());
        } else {
            log!("    WARNING: BFS could not find region {ra} from region {rb}!");
        }
    }

    // --- Step 4: DFS carve per region (after boundary fixes) ---
    log!("");
    log!("--- Step 4: DFS carve per region ---");
    let mut h_walls = vec![true; n];
    let mut v_walls = vec![true; n];
    for region in 0..num_regions {
        if region_cells_list[region].is_empty() {
            log!("  region {region}: EMPTY — skipped");
            continue;
        }
        let start = region_cells_list[region][0];
        log!("  region {region}: {} cells, DFS from ({}, {})", region_cells_list[region].len(), start.0, start.1);
        maze_dfs_region(&mut h_walls, &mut v_walls, cols, rows, &cell_regions, region, start, &mut rng);
    }

    // --- Step 4b: Open extra passages to reduce dead-ends ---
    let ratio = config.dead_ends.ratio();
    log!("");
    log!("--- Step 4b: Dead-end pass (ratio={ratio}) ---");
    if ratio < 1.0 {
        open_extra_passages(&mut h_walls, &mut v_walls, cols, rows, &cell_regions, num_regions, ratio, &mut rng);
        log!("  opened extra passages");
    } else {
        log!("  ratio=1.0, no extra passages");
    }

    // --- Step 5: Place gates — boundaries are guaranteed to exist ---
    log!("");
    log!("--- Step 5: Place gates ---");
    let mut gates = Vec::new();

    // Key tree gates
    for ki in 0..nk {
        let parent_region = if key_parents[ki] == usize::MAX { 0 } else { key_parents[ki] + 1 };
        let child_region = ki + 1;
        let boundary = find_boundary_edges(&cell_regions, cols, rows, parent_region, child_region);
        if boundary.is_empty() {
            log!("  key {ki}: regions {parent_region}-{child_region} NO BOUNDARY — free passage");
            continue;
        }
        if key_parents[ki] == usize::MAX {
            let pair = boundary[rng.random_range(0..boundary.len())];
            log!("  key {ki}: root, open wall between ({},{}) and ({},{})",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1);
            remove_wall(&mut h_walls, &mut v_walls, cols, rows,
                pair[0].0, pair[0].1, pair[1].0, pair[1].1);
        } else {
            let pair = pick_spaced_gate_edge(&boundary, &gates, key_parents[ki], cols, rows, &mut rng);
            log!("  key {ki}: gate (key_index={}) at ({},{})—({},{}) [{} candidates]",
                key_parents[ki], pair[0].0, pair[0].1, pair[1].0, pair[1].1, boundary.len());
            gates.push(Gate { cells: pair, key_index: key_parents[ki] });
        }
    }

    // Corridor gates: linear chain, key i guards corridor i.
    {
        let boundary = find_boundary_edges(&cell_regions, cols, rows, 0, corridor_base);
        if !boundary.is_empty() {
            let pair = pick_spaced_gate_edge(&boundary, &gates, 0, cols, rows, &mut rng);
            log!("  corridor entrance: gate (key_index=0) at ({},{})—({},{}) [{} candidates]",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1, boundary.len());
            gates.push(Gate { cells: pair, key_index: 0 });
        } else {
            log!("  corridor entrance: NO BOUNDARY — free passage");
        }
    }
    for ci in 1..nk {
        let prev_r = corridor_base + ci - 1;
        let cur_r = corridor_base + ci;
        let boundary = find_boundary_edges(&cell_regions, cols, rows, prev_r, cur_r);
        if boundary.is_empty() {
            log!("  corridor {ci}: regions {prev_r}-{cur_r} NO BOUNDARY — free passage");
            continue;
        }
        let pair = pick_spaced_gate_edge(&boundary, &gates, ci, cols, rows, &mut rng);
        log!("  corridor {ci}: gate (key_index={ci}) at ({},{})—({},{}) [{} candidates]",
            pair[0].0, pair[0].1, pair[1].0, pair[1].1, boundary.len());
        gates.push(Gate { cells: pair, key_index: ci });
    }
    // Open passage from last corridor to goal
    {
        let boundary = find_boundary_edges(&cell_regions, cols, rows, last_corridor, goal_region);
        if !boundary.is_empty() {
            let pair = boundary[rng.random_range(0..boundary.len())];
            log!("  goal passage: open wall between ({},{}) and ({},{})",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1);
            remove_wall(&mut h_walls, &mut v_walls, cols, rows,
                pair[0].0, pair[0].1, pair[1].0, pair[1].1);
        } else {
            log!("  goal passage: NO BOUNDARY — free passage");
        }
    }

    log!("total gates placed: {}", gates.len());

    // --- Step 6: Place start, keys, goal ---
    log!("");
    log!("--- Step 6: Place start, keys, goal ---");
    let start = *region_cells_list[0].iter()
        .min_by_key(|&&(cx, cy)| {
            let dx = cx as i32 - center.0 as i32;
            let dy = cy as i32 - center.1 as i32;
            dx * dx + dy * dy
        })
        .unwrap();
    log!("  start: ({}, {}) in region 0", start.0, start.1);

    let mut occupied: HashSet<(usize, usize)> = HashSet::new();
    occupied.insert(start);

    let mut key_cells = Vec::new();
    for ki in 0..nk {
        let region = ki + 1;
        let mut available: Vec<_> = region_cells_list[region].iter()
            .filter(|c| !occupied.contains(c))
            .cloned().collect();
        // Fallback: if region is empty, place in start region
        if available.is_empty() {
            log!("  key {ki}: region {region} EMPTY — falling back to region 0");
            available = region_cells_list[0].iter()
                .filter(|c| !occupied.contains(c))
                .cloned().collect();
        }
        log!("  key {ki}: region {region} has {} available cells", available.len());
        let kc = available[rng.random_range(0..available.len())];
        log!("  key {ki}: placed at ({}, {})", kc.0, kc.1);
        key_cells.push(kc);
        occupied.insert(kc);
    }

    let mut goal_available: Vec<_> = region_cells_list[goal_region].iter()
        .filter(|c| !occupied.contains(c))
        .cloned().collect();
    if goal_available.is_empty() {
        log!("  goal: region {goal_region} EMPTY — falling back to region 0");
        goal_available = region_cells_list[0].iter()
            .filter(|c| !occupied.contains(c))
            .cloned().collect();
    }
    log!("  goal: region {goal_region} has {} available cells", goal_available.len());
    let goal = goal_available[rng.random_range(0..goal_available.len())];
    log!("  goal: placed at ({}, {})", goal.0, goal.1);

    let key_designs: Vec<usize> = (0..nk).map(|_| rng.random_range(0..KEYS_PER_COLOR)).collect();

    log!("");
    log!("=== GENERATION COMPLETE ===");

    MazeLayout {
        seed, num_keys, cols, rows, maze_w, maze_h,
        h_walls, v_walls, cell_regions, key_parents, gates, start, goal,
        key_cells, key_designs,
    }
}

// ---------------------------------------------------------------------------
// Collision & pathfinding
// ---------------------------------------------------------------------------

fn wall_between(
    layout: &MazeLayout, keys: &[bool],
    cx1: usize, cy1: usize, cx2: usize, cy2: usize,
) -> bool {
    let cols = layout.cols;
    let rows = layout.rows;

    let base_wall = if cy1 == cy2 {
        if (cx1 + 1) % cols == cx2 {
            layout.h_walls[cy1 * cols + cx1]
        } else if (cx2 + 1) % cols == cx1 {
            layout.h_walls[cy2 * cols + cx2]
        } else {
            return true;
        }
    } else if cx1 == cx2 {
        if (cy1 + 1) % rows == cy2 {
            layout.v_walls[cy1 * cols + cx1]
        } else if (cy2 + 1) % rows == cy1 {
            layout.v_walls[cy2 * cols + cx2]
        } else {
            return true;
        }
    } else {
        return true;
    };

    if !base_wall { return false; }

    for g in &layout.gates {
        if (g.cells[0] == (cx1, cy1) && g.cells[1] == (cx2, cy2))
            || (g.cells[0] == (cx2, cy2) && g.cells[1] == (cx1, cy1))
        {
            return !keys[g.key_index];
        }
    }

    true
}

fn build_collision(layout: &MazeLayout, keys: &[bool]) -> Vec<bool> {
    let cols = layout.cols;
    let rows = layout.rows;
    let mw = layout.maze_w;
    let mh = layout.maze_h;
    let mut map = vec![false; mw * mh];

    for cy in 0..rows {
        for cx in 0..cols {
            let px0 = cx * CELL;
            let py0 = cy * CELL;

            for py in py0..py0 + WALL_PX {
                for px in px0..px0 + WALL_PX {
                    map[py * mw + px] = true;
                }
            }

            let left_cx = (cx + cols - 1) % cols;
            if wall_between(layout, keys, left_cx, cy, cx, cy) {
                for py in py0 + WALL_PX..py0 + WALL_PX + CORR_PX {
                    for px in px0..px0 + WALL_PX {
                        map[py * mw + px] = true;
                    }
                }
            }

            let top_cy = (cy + rows - 1) % rows;
            if wall_between(layout, keys, cx, top_cy, cx, cy) {
                for py in py0..py0 + WALL_PX {
                    for px in px0 + WALL_PX..px0 + WALL_PX + CORR_PX {
                        map[py * mw + px] = true;
                    }
                }
            }
        }
    }

    map
}

fn can_move(px: i32, py: i32, col: &[bool], mw: usize, mh: usize) -> bool {
    for dy in 0..CORR_PX as i32 {
        for dx in 0..CORR_PX as i32 {
            let x = (px + dx).rem_euclid(mw as i32) as usize;
            let y = (py + dy).rem_euclid(mh as i32) as usize;
            if col[y * mw + x] { return false; }
        }
    }
    true
}

fn player_cell(px: i32, py: i32, layout: &MazeLayout) -> (usize, usize) {
    let x = (px + CORR_PX as i32 / 2).rem_euclid(layout.maze_w as i32) as usize;
    let y = (py + CORR_PX as i32 / 2).rem_euclid(layout.maze_h as i32) as usize;
    (x / CELL, y / CELL)
}

fn cell_pixel(cx: usize, cy: usize) -> (i32, i32) {
    ((cx * CELL + WALL_PX) as i32, (cy * CELL + WALL_PX) as i32)
}

fn is_cell_aligned(px: i32, mw: usize) -> bool {
    let wrapped = px.rem_euclid(mw as i32);
    (wrapped as usize % CELL) == WALL_PX
}

/// Find all currently reachable objectives (uncollected keys whose parents
/// are collected, or goal if all keys collected). Returns directions toward
/// the nearest one.
fn next_objective_dirs(
    layout: &MazeLayout, keys: &[bool], from: (usize, usize),
) -> Vec<(i32, i32)> {
    // Collect targets: uncollected keys whose parent key is already collected
    // (or root keys with parent == usize::MAX), plus goal if all keys collected.
    let mut targets = Vec::new();
    for ki in 0..layout.num_keys {
        if keys[ki] { continue; }
        let parent = layout.key_parents[ki];
        if parent == usize::MAX || keys[parent] {
            targets.push(layout.key_cells[ki]);
        }
    }
    if keys.iter().all(|&k| k) {
        targets.push(layout.goal);
    }
    if targets.is_empty() || targets.contains(&from) { return vec![]; }

    let cols = layout.cols;
    let rows = layout.rows;
    let n = rows * cols;

    // BFS from ALL targets backwards to compute distance-to-nearest-target
    let mut dist = vec![u32::MAX; n];
    let mut queue = VecDeque::new();
    for &(tx, ty) in &targets {
        let ti = ty * cols + tx;
        if dist[ti] == u32::MAX {
            dist[ti] = 0;
            queue.push_back((tx, ty));
        }
    }

    while let Some((cx, cy)) = queue.pop_front() {
        let cd = dist[cy * cols + cx];
        for &(nx, ny) in &[
            ((cx + 1) % cols, cy),
            ((cx + cols - 1) % cols, cy),
            (cx, (cy + 1) % rows),
            (cx, (cy + rows - 1) % rows),
        ] {
            let ni = ny * cols + nx;
            if dist[ni] != u32::MAX { continue; }
            if wall_between(layout, keys, cx, cy, nx, ny) { continue; }
            dist[ni] = cd + 1;
            queue.push_back((nx, ny));
        }
    }

    let si = from.1 * cols + from.0;
    if dist[si] == u32::MAX { return vec![]; }

    let my_dist = dist[si];
    let mut dirs = vec![];
    let neighbors = [
        ((from.0 + 1) % cols, from.1, 1i32, 0i32),
        ((from.0 + cols - 1) % cols, from.1, -1, 0),
        (from.0, (from.1 + 1) % rows, 0, 1),
        (from.0, (from.1 + rows - 1) % rows, 0, -1),
    ];
    for &(nx, ny, dx, dy) in &neighbors {
        if wall_between(layout, keys, from.0, from.1, nx, ny) { continue; }
        let ni = ny * cols + nx;
        if dist[ni] < my_dist {
            dirs.push((dx, dy));
        }
    }
    dirs
}

// ---------------------------------------------------------------------------
// XInput FFI
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    _thumb_rx: i16,
    _thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    _packet_number: u32,
    gamepad: XInputGamepad,
}

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

fn load_xinput() -> Option<XInputGetStateFn> {
    use std::ffi::CString;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() { continue; }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

fn normalize_thumb(value: i16) -> f32 {
    if value >= 0 { value as f32 / 32767.0 } else { value as f32 / 32768.0 }
}

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct PadState {
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    left_stick: Vec2,
}

#[derive(Resource)]
struct MazeImage(Handle<Image>);

#[derive(Resource)]
struct KeySheetHandle(Handle<Image>);

#[derive(Resource, Default)]
struct KeySheetPixels(Option<Vec<u8>>);

#[derive(Resource)]
struct MazeConfig {
    num_keys: usize,
    view_cells: usize,
    graph_shape: GraphShape,
    region_size: RegionSize,
    dead_ends: DeadEndLevel,
    max_branch: usize,
}

#[derive(Resource)]
struct GameState {
    px: i32,
    py: i32,
    cam_x: i32,
    cam_y: i32,
    keys: Vec<bool>,
    won: bool,
    move_timer: f32,
    last_dir: (i32, i32),
    glide: (i32, i32),
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
) {
    let num_keys = DEFAULT_KEYS;
    let view_cells = DEFAULT_VIEW_CELLS;
    let config = MazeConfig {
        num_keys, view_cells,
        graph_shape: GraphShape::Branching,
        region_size: RegionSize::Medium,
        dead_ends: DeadEndLevel::High,
        max_branch: DEFAULT_BRANCH,
    };
    let seed: u64 = rand::random();
    let layout = generate_maze(seed, num_keys, &config);
    let (sx, sy) = cell_pixel(layout.start.0, layout.start.1);
    let view_px = (view_cells * CELL) as i32;
    let cam_x = sx + CORR_PX as i32 / 2 - view_px / 2;
    let cam_y = sy + CORR_PX as i32 / 2 - view_px / 2;
    commands.insert_resource(config);
    commands.insert_resource(layout);
    commands.insert_resource(GameState {
        px: sx, py: sy, cam_x, cam_y,
        keys: vec![false; num_keys], won: false, move_timer: 0.0,
        last_dir: (0, 0), glide: (0, 0),
    });

    let (img_w, img_h) = image_dims(view_cells, num_keys);
    let pixels = vec![0u8; img_w * img_h * 4];
    let image = Image::new(
        Extent3d { width: img_w as u32, height: img_h as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.insert_resource(MazeImage(handle.clone()));
    commands.spawn(Sprite { image: handle, ..default() });
    commands.spawn(Camera2d);

    let sheet: Handle<Image> = asset_server.load("external/64-bit/keys/Fantasy Keys-sheet.png");
    commands.insert_resource(KeySheetHandle(sheet));
}

fn cache_key_sheet(
    sheet_handle: Res<KeySheetHandle>,
    images: Res<Assets<Image>>,
    mut cached: ResMut<KeySheetPixels>,
) {
    if cached.0.is_some() { return; }
    if let Some(img) = images.get(&sheet_handle.0) {
        cached.0 = img.data.clone();
    }
}

fn auto_scale(
    windows: Query<&Window>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
    config: Res<MazeConfig>,
) {
    let Ok(win) = windows.single() else { return };
    let Ok(mut proj) = proj_q.single_mut() else { return };
    let Projection::Orthographic(ref mut ortho) = *proj else { return };
    let (img_w, img_h) = image_dims(config.view_cells, config.num_keys);
    let zoom = (win.width() / img_w as f32).min(win.height() / img_h as f32).floor().max(1.0);
    let new_scale = 1.0 / zoom;
    if (ortho.scale - new_scale).abs() > f32::EPSILON { ortho.scale = new_scale; }
}

fn read_gamepad(
    mut state: ResMut<PadState>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            *xinput_fn = Some(loaded);
            match loaded { Some(f) => f, None => return }
        }
    };
    let mut xs = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xs.as_mut_ptr()) };
    if result != 0 { *state = PadState::default(); return; }
    let xs = unsafe { xs.assume_init() };
    let gp = &xs.gamepad;
    let btn = |mask: u16| gp.buttons & mask != 0;
    state.dpad_up = btn(DPAD_UP);
    state.dpad_down = btn(DPAD_DOWN);
    state.dpad_left = btn(DPAD_LEFT);
    state.dpad_right = btn(DPAD_RIGHT);
    state.left_stick = Vec2::new(normalize_thumb(gp.thumb_lx), normalize_thumb(gp.thumb_ly));
}

fn movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    pad: Res<PadState>,
    layout: Res<MazeLayout>,
    config: Res<MazeConfig>,
    mut gs: ResMut<GameState>,
) {
    if gs.won { return; }

    let mw = layout.maze_w;
    let mh = layout.maze_h;

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) { dx += 1; }
    if keyboard.pressed(KeyCode::ArrowLeft)  || keyboard.pressed(KeyCode::KeyA) { dx -= 1; }
    if keyboard.pressed(KeyCode::ArrowDown)  || keyboard.pressed(KeyCode::KeyS) { dy += 1; }
    if keyboard.pressed(KeyCode::ArrowUp)    || keyboard.pressed(KeyCode::KeyW) { dy -= 1; }
    if pad.dpad_right { dx += 1; }
    if pad.dpad_left  { dx -= 1; }
    if pad.dpad_down  { dy += 1; }
    if pad.dpad_up    { dy -= 1; }
    if pad.left_stick.x >  STICK_DEAD { dx += 1; }
    if pad.left_stick.x < -STICK_DEAD { dx -= 1; }
    if pad.left_stick.y >  STICK_DEAD { dy -= 1; }
    if pad.left_stick.y < -STICK_DEAD { dy += 1; }
    dx = dx.clamp(-1, 1);
    dy = dy.clamp(-1, 1);

    if dx != 0 { gs.glide.0 = dx; }
    if dy != 0 { gs.glide.1 = dy; }

    let eff_dx = if dx != 0 { dx } else { gs.glide.0 };
    let eff_dy = if dy != 0 { dy } else { gs.glide.1 };

    if eff_dx == 0 && eff_dy == 0 {
        // No input, no glide — snap to nearest cell alignment on both axes.
        let mw_i = mw as i32;
        let mh_i = mh as i32;
        let cell_i = CELL as i32;
        let wall_i = WALL_PX as i32;
        let corr_i = CORR_PX as i32;

        let off_x = (gs.px.rem_euclid(mw_i) % cell_i) - wall_i;
        if off_x != 0 {
            if off_x > corr_i / 2 {
                gs.px = (gs.px + (cell_i - off_x)).rem_euclid(mw_i);
            } else {
                gs.px = (gs.px - off_x).rem_euclid(mw_i);
            }
        }
        let off_y = (gs.py.rem_euclid(mh_i) % cell_i) - wall_i;
        if off_y != 0 {
            if off_y > corr_i / 2 {
                gs.py = (gs.py + (cell_i - off_y)).rem_euclid(mh_i);
            } else {
                gs.py = (gs.py - off_y).rem_euclid(mh_i);
            }
        }
        gs.move_timer = 0.0;
        gs.last_dir = (0, 0);
        return;
    }

    let col = build_collision(&layout, &gs.keys);
    gs.move_timer += time.delta_secs();
    let interval = 1.0 / MOVE_SPEED;
    while gs.move_timer >= interval {
        gs.move_timer -= interval;

        let x_blocked = eff_dx != 0 && !can_move(gs.px + eff_dx, gs.py, &col, mw, mh);
        let y_blocked = eff_dy != 0 && !can_move(gs.px, gs.py + eff_dy, &col, mw, mh);

        let mut mx = if x_blocked { 0 } else { eff_dx };
        let mut my = if y_blocked { 0 } else { eff_dy };

        let nudge = (WALL_PX as i32) / 2;
        if y_blocked && eff_dx != 0 {
            let mut best = None;
            for offset in 1..=nudge {
                for &sign in &[-1i32, 1] {
                    let test_x = gs.px + sign * offset;
                    if can_move(test_x, gs.py + eff_dy, &col, mw, mh) {
                        best = Some(sign);
                        break;
                    }
                }
                if best.is_some() { break; }
            }
            if let Some(nudge_dir) = best {
                mx = nudge_dir;
                my = 0;
            }
        }
        if x_blocked && eff_dy != 0 {
            let mut best = None;
            for offset in 1..=nudge {
                for &sign in &[-1i32, 1] {
                    let test_y = gs.py + sign * offset;
                    if can_move(gs.px + eff_dx, test_y, &col, mw, mh) {
                        best = Some(sign);
                        break;
                    }
                }
                if best.is_some() { break; }
            }
            if let Some(nudge_dir) = best {
                my = nudge_dir;
                mx = 0;
            }
        }

        if x_blocked && my == 0 && gs.last_dir.1 != 0 {
            my = gs.last_dir.1;
        }
        if y_blocked && mx == 0 && gs.last_dir.0 != 0 {
            mx = gs.last_dir.0;
        }

        if mx != 0 && can_move(gs.px + mx, gs.py, &col, mw, mh) {
            gs.px = (gs.px + mx).rem_euclid(mw as i32);
            gs.last_dir = (mx, gs.last_dir.1);
        }
        if my != 0 && can_move(gs.px, gs.py + my, &col, mw, mh) {
            gs.py = (gs.py + my).rem_euclid(mh as i32);
            gs.last_dir = (gs.last_dir.0, my);
        }

        // When wall blocks glide mid-corridor, reverse to slide back to alignment
        if x_blocked {
            if !is_cell_aligned(gs.px, mw) { gs.glide.0 = -gs.glide.0; }
            else { gs.glide.0 = 0; }
        }
        if y_blocked {
            if !is_cell_aligned(gs.py, mh) { gs.glide.1 = -gs.glide.1; }
            else { gs.glide.1 = 0; }
        }

        if dx == 0 && gs.glide.0 != 0 && is_cell_aligned(gs.px, mw) {
            gs.glide.0 = 0;
        }
        if dy == 0 && gs.glide.1 != 0 && is_cell_aligned(gs.py, mh) {
            gs.glide.1 = 0;
        }
    }

    // Pickups
    let (cx, cy) = player_cell(gs.px, gs.py, &layout);
    for (ki, &(kcx, kcy)) in layout.key_cells.iter().enumerate() {
        if cx == kcx && cy == kcy && !gs.keys[ki] {
            gs.keys[ki] = true;
        }
    }
    if (cx, cy) == layout.goal {
        gs.won = true;
    }

    // Dead-zone camera
    let view_px = (config.view_cells * CELL) as i32;
    let margin = CELL as i32;
    let mw_i = mw as i32;
    let mh_i = mh as i32;
    let vx = (gs.px - gs.cam_x).rem_euclid(mw_i);
    let vy = (gs.py - gs.cam_y).rem_euclid(mh_i);
    let min = margin;
    let max_x = view_px - margin - CORR_PX as i32;
    let max_y = view_px - margin - CORR_PX as i32;
    if vx < min {
        gs.cam_x = (gs.cam_x - (min - vx)).rem_euclid(mw_i);
    } else if vx > max_x {
        gs.cam_x = (gs.cam_x + (vx - max_x)).rem_euclid(mw_i);
    }
    if vy < min {
        gs.cam_y = (gs.cam_y - (min - vy)).rem_euclid(mh_i);
    } else if vy > max_y {
        gs.cam_y = (gs.cam_y + (vy - max_y)).rem_euclid(mh_i);
    }
}

fn center_camera(gs: &mut GameState, layout: &MazeLayout, view_cells: usize) {
    let view_px = (view_cells * CELL) as i32;
    let mw = layout.maze_w as i32;
    let mh = layout.maze_h as i32;
    gs.cam_x = (gs.px + CORR_PX as i32 / 2 - view_px / 2).rem_euclid(mw);
    gs.cam_y = (gs.py + CORR_PX as i32 / 2 - view_px / 2).rem_euclid(mh);
}

fn restart_or_new(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<MazeConfig>,
    mut layout: ResMut<MazeLayout>,
    mut gs: ResMut<GameState>,
) {
    let new_maze = keyboard.just_pressed(KeyCode::KeyN);
    let restart = keyboard.just_pressed(KeyCode::KeyR);
    let more_keys = keyboard.just_pressed(KeyCode::Equal) || keyboard.just_pressed(KeyCode::NumpadAdd);
    let fewer_keys = keyboard.just_pressed(KeyCode::Minus) || keyboard.just_pressed(KeyCode::NumpadSubtract);
    let bigger_view = keyboard.just_pressed(KeyCode::BracketRight);
    let smaller_view = keyboard.just_pressed(KeyCode::BracketLeft);
    let cycle_shape = keyboard.just_pressed(KeyCode::KeyG);
    let cycle_region = keyboard.just_pressed(KeyCode::KeyF);
    let cycle_dead = keyboard.just_pressed(KeyCode::KeyV);
    let cycle_branch = keyboard.just_pressed(KeyCode::KeyB);

    // Settings that trigger regeneration
    let mut regen = false;
    if cycle_shape { config.graph_shape = config.graph_shape.cycle(); regen = true; }
    if cycle_region { config.region_size = config.region_size.cycle(); regen = true; }
    if cycle_dead { config.dead_ends = config.dead_ends.cycle(); regen = true; }
    if cycle_branch {
        config.max_branch = config.max_branch % 3 + 1; // 1->2->3->1
        regen = true;
    }
    if more_keys && config.num_keys < MAX_KEYS { config.num_keys += 1; regen = true; }
    if fewer_keys && config.num_keys > 1 { config.num_keys -= 1; regen = true; }
    if new_maze { regen = true; }

    if regen {
        let seed: u64 = rand::random();
        *layout = generate_maze(seed, config.num_keys, &config);
    }

    if regen || restart {
        let (sx, sy) = cell_pixel(layout.start.0, layout.start.1);
        gs.px = sx;
        gs.py = sy;
        gs.keys = vec![false; config.num_keys];
        gs.won = false;
        gs.move_timer = 0.0;
        gs.last_dir = (0, 0);
        gs.glide = (0, 0);
        center_camera(&mut gs, &layout, config.view_cells);
    }

    if bigger_view && config.view_cells < MAX_VIEW_CELLS {
        config.view_cells += 2;
        center_camera(&mut gs, &layout, config.view_cells);
    } else if smaller_view && config.view_cells > MIN_VIEW_CELLS {
        config.view_cells -= 2;
        center_camera(&mut gs, &layout, config.view_cells);
    }
}

fn blit_key_sprite(
    dst: &mut [u8], dst_w: usize,
    sheet: &[u8],
    color_idx: usize, design: usize,
    dest_x: usize, dest_y: usize,
) {
    let (sc, sr) = key_sheet_pos(color_idx, design);
    let sx0 = sc * KEY_SPRITE_SIZE;
    let sy0 = sr * KEY_SPRITE_SIZE;
    for y in 0..KEY_SPRITE_SIZE {
        for x in 0..KEY_SPRITE_SIZE {
            let si = ((sy0 + y) * SHEET_W + (sx0 + x)) * 4;
            if si + 3 >= sheet.len() { continue; }
            if sheet[si + 3] == 0 { continue; }
            let dx = dest_x + x;
            let dy = dest_y + y;
            if dx >= dst_w || dy * dst_w + dx >= dst.len() / 4 { continue; }
            let di = (dy * dst_w + dx) * 4;
            dst[di..di + 4].copy_from_slice(&sheet[si..si + 4]);
        }
    }
}

fn render(
    gs: Res<GameState>,
    config: Res<MazeConfig>,
    layout: Res<MazeLayout>,
    maze_img: Res<MazeImage>,
    mut images: ResMut<Assets<Image>>,
    sheet_pixels: Res<KeySheetPixels>,
) {
    let view_cells = config.view_cells;
    let view_px = view_cells * CELL;
    let nk = layout.num_keys;
    let (img_w, img_h) = image_dims(view_cells, nk);
    let mw = layout.maze_w;
    let mh = layout.maze_h;

    let Some(image) = images.get_mut(&maze_img.0) else { return };

    // Resize image if dimensions changed
    let expected = img_w * img_h * 4;
    if image.data.as_ref().map_or(true, |d| d.len() != expected) {
        *image = Image::new(
            Extent3d { width: img_w as u32, height: img_h as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            vec![0u8; expected],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
    }
    let data = image.data.as_mut().unwrap();

    // --- Helpers for output image ---
    let set = |data: &mut [u8], x: usize, y: usize, c: [u8; 4]| {
        if x < img_w && y < img_h {
            let i = (y * img_w + x) * 4;
            data[i..i + 4].copy_from_slice(&c);
        }
    };
    let fill = |data: &mut [u8], x0: usize, y0: usize, w: usize, h: usize, c: [u8; 4]| {
        for y in y0..(y0 + h).min(img_h) {
            for x in x0..(x0 + w).min(img_w) {
                let i = (y * img_w + x) * 4;
                data[i..i + 4].copy_from_slice(&c);
            }
        }
    };

    // --- Clear entire image with gap color ---
    let content_h = img_h - LEGEND_H;
    for y in 0..img_h { for x in 0..img_w { set(data, x, y, COL_GAP); } }

    // --- Build full maze buffer ---
    let mut maze = vec![0u8; mw * mh * 4];

    let mset = |buf: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| {
        let i = (y * mw + x) * 4;
        buf[i..i + 4].copy_from_slice(&c);
    };
    let mfill = |buf: &mut Vec<u8>, x0: usize, y0: usize, w: usize, h: usize, c: [u8; 4]| {
        for y in y0..(y0 + h).min(mh) { for x in x0..(x0 + w).min(mw) {
            let i = (y * mw + x) * 4;
            buf[i..i + 4].copy_from_slice(&c);
        }}
    };

    for y in 0..mh { for x in 0..mw { mset(&mut maze, x, y, COL_BG); } }

    let collision = build_collision(&layout, &gs.keys);
    for y in 0..mh { for x in 0..mw {
        if collision[y * mw + x] { mset(&mut maze, x, y, COL_WALL); }
    }}

    // Gates
    for g in &layout.gates {
        let col = KEY_COLORS[g.key_index % KEY_COLORS.len()];
        let unlocked = gs.keys[g.key_index];
        let thickness = if unlocked { WALL_PX / 2 } else { WALL_PX };
        let (ax, ay) = g.cells[0];
        let (bx, by) = g.cells[1];
        let cols = layout.cols;
        let rows = layout.rows;
        if ay == by {
            let right_x = if (ax + 1) % cols == bx { bx } else { ax };
            let wall_px = right_x * CELL;
            let wall_py = ay * CELL + WALL_PX;
            let offset = if unlocked { (WALL_PX - thickness) / 2 } else { 0 };
            mfill(&mut maze, wall_px + offset, wall_py, thickness, CORR_PX, col);
        } else {
            let bottom_y = if (ay + 1) % rows == by { by } else { ay };
            let wall_px = ax * CELL + WALL_PX;
            let wall_py = bottom_y * CELL;
            let offset = if unlocked { (WALL_PX - thickness) / 2 } else { 0 };
            mfill(&mut maze, wall_px, wall_py + offset, CORR_PX, thickness, col);
        }
    }

    // Goal
    let item_off = (CORR_PX - ITEM_SIZE) / 2;
    {
        let (gx, gy) = layout.goal;
        let x0 = gx * CELL + WALL_PX + item_off;
        let y0 = gy * CELL + WALL_PX + item_off;
        mfill(&mut maze, x0, y0, ITEM_SIZE, ITEM_SIZE, COL_GOAL);
    }

    // Keys in maze
    for (ki, &(kcx, kcy)) in layout.key_cells.iter().enumerate() {
        if gs.keys[ki] { continue; }
        let kpx = kcx * CELL + WALL_PX;
        let kpy = kcy * CELL + WALL_PX;
        let color_idx = ki % KEY_COLORS.len();
        if let Some(sheet) = sheet_pixels.0.as_ref() {
            blit_key_sprite(&mut maze, mw, sheet, color_idx, layout.key_designs[ki], kpx, kpy);
        } else {
            let off = (CORR_PX - ITEM_SIZE) / 2;
            mfill(&mut maze, kpx + off, kpy + off, ITEM_SIZE, ITEM_SIZE, KEY_COLORS[color_idx]);
        }
    }

    // Player
    let pc = if gs.won { COL_WIN } else { COL_PLAYER };
    for dy in 0..CORR_PX { for dx in 0..CORR_PX {
        let x = (gs.px as usize + dx) % mw;
        let y = (gs.py as usize + dy) % mh;
        mset(&mut maze, x, y, pc);
    }}

    // --- Inventory panel (left side) ---
    let inv_rows = nk.max(1).min(INV_ROWS);
    let inv_content_h = inv_rows * KEY_SPRITE_SIZE + (inv_rows + 1) * WALL_PX;
    let panel_w = inv_w(nk);
    let inv_y0 = if inv_content_h < content_h { (content_h - inv_content_h) / 2 } else { 0 };
    fill(data, 0, inv_y0, panel_w, inv_content_h.min(img_h), COL_BG);

    for i in 0..nk {
        let col = i / INV_ROWS;
        let row = i % INV_ROWS;
        let kx = WALL_PX + col * INV_COL_W;
        let ky = inv_y0 + WALL_PX + row * (KEY_SPRITE_SIZE + WALL_PX);
        let color_idx = i % KEY_COLORS.len();
        if gs.keys[i] {
            if let Some(sheet) = sheet_pixels.0.as_ref() {
                blit_key_sprite(data, img_w, sheet, color_idx, layout.key_designs[i], kx, ky);
            } else {
                let off = (KEY_SPRITE_SIZE - ITEM_SIZE) / 2;
                fill(data, kx + off, ky + off, ITEM_SIZE, ITEM_SIZE, KEY_COLORS[color_idx]);
            }
        } else {
            let off = (KEY_SPRITE_SIZE - ITEM_SIZE) / 2;
            fill(data, kx + off, ky + off, ITEM_SIZE, ITEM_SIZE, COL_DIM);
        }
    }

    // --- Direction indicator (below keys in inventory) ---
    let pcell = player_cell(gs.px, gs.py, &layout);
    if !gs.won {
        let progress_dirs = next_objective_dirs(&layout, &gs.keys, pcell);
        if !progress_dirs.is_empty() {
            let arrow_y = inv_y0 + inv_content_h + WALL_PX;
            let arrow_cx = panel_w / 2;
            let arrow_cy = arrow_y + CELL / 2;
            let arrow_size = WALL_PX;
            let half = arrow_size / 2;

            // Color = nearest reachable uncollected key, or goal
            let arrow_col = if gs.keys.iter().all(|&k| k) {
                COL_GOAL
            } else {
                // Find any reachable uncollected key (parent collected or root)
                let reachable_key = (0..nk).find(|&ki| {
                    !gs.keys[ki] && (layout.key_parents[ki] == usize::MAX || gs.keys[layout.key_parents[ki]])
                });
                match reachable_key {
                    Some(ki) => KEY_COLORS[ki % KEY_COLORS.len()],
                    None => COL_GOAL,
                }
            };

            // Border around the cross shape
            let b = 2usize;
            let col_border: [u8; 4] = [140, 140, 140, 255];
            // Horizontal bar border (left-center-right)
            fill(data, arrow_cx - half - arrow_size - b, arrow_cy - half - b,
                arrow_size * 3 + b * 2, arrow_size + b * 2, col_border);
            // Vertical bar border (up-center-down)
            fill(data, arrow_cx - half - b, arrow_cy - half - arrow_size - b,
                arrow_size + b * 2, arrow_size * 3 + b * 2, col_border);

            // Draw all 4 direction stubs + center as muted reference
            let col_ref: [u8; 4] = [100, 100, 100, 255];
            fill(data, arrow_cx + half, arrow_cy - half, arrow_size, arrow_size, col_ref);  // right
            fill(data, arrow_cx - half - arrow_size, arrow_cy - half, arrow_size, arrow_size, col_ref); // left
            fill(data, arrow_cx - half, arrow_cy + half, arrow_size, arrow_size, col_ref);  // down
            fill(data, arrow_cx - half, arrow_cy - half - arrow_size, arrow_size, arrow_size, col_ref); // up
            // Center dot
            fill(data, arrow_cx - half, arrow_cy - half, arrow_size, arrow_size, col_ref);
            // Overdraw progress directions in bright color
            for &(ddx, ddy) in &progress_dirs {
                if ddx > 0 {
                    fill(data, arrow_cx + half, arrow_cy - half, arrow_size, arrow_size, arrow_col);
                } else if ddx < 0 {
                    fill(data, arrow_cx - half - arrow_size, arrow_cy - half, arrow_size, arrow_size, arrow_col);
                }
                if ddy > 0 {
                    fill(data, arrow_cx - half, arrow_cy + half, arrow_size, arrow_size, arrow_col);
                } else if ddy < 0 {
                    fill(data, arrow_cx - half, arrow_cy - half - arrow_size, arrow_size, arrow_size, arrow_col);
                }
            }
        }
    }

    // --- Maze viewport (right side) ---
    let maze_x0 = panel_w + GAP_W;
    let maze_y0 = if view_px < content_h { (content_h - view_px) / 2 } else { 0 };
    let cam_x = gs.cam_x;
    let cam_y = gs.cam_y;
    for vy in 0..view_px {
        for vx in 0..view_px {
            let mx = (cam_x + vx as i32).rem_euclid(mw as i32) as usize;
            let my = (cam_y + vy as i32).rem_euclid(mh as i32) as usize;
            let si = (my * mw + mx) * 4;
            let ox = maze_x0 + vx;
            let oy = maze_y0 + vy;
            if ox < img_w && oy < img_h {
                let di = (oy * img_w + ox) * 4;
                data[di..di + 4].copy_from_slice(&maze[si..si + 4]);
            }
        }
    }

    // --- Legend (bottom strip) ---
    // COL_GAP gap is already there from the clear; paint the body below it.
    let legend_y0 = content_h + LEGEND_GAP;
    fill(data, 0, legend_y0, img_w, LEGEND_BODY_H, COL_BG);

    // Two columns: left = state, right = controls
    let lx = LEGEND_PAD;
    let rx = img_w / 2;
    let mut ly = legend_y0 + LEGEND_PAD;

    // State column
    let seed_str = format!("SEED: {}", layout.seed);
    draw_text(data, img_w, img_h, &seed_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let keys_str = format!("KEYS: {}/{}", gs.keys.iter().filter(|&&k| k).count(), config.num_keys);
    draw_text(data, img_w, img_h, &keys_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let grid_str = format!("GRID: {}x{}", layout.cols, layout.rows);
    draw_text(data, img_w, img_h, &grid_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let view_str = format!("VIEW: {}x{}", config.view_cells, config.view_cells);
    draw_text(data, img_w, img_h, &view_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let shape_str = format!("SHAPE: {}", config.graph_shape.label());
    draw_text(data, img_w, img_h, &shape_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let region_str = format!("REGION: {}/{}", config.region_size.label(), config.region_size.cells());
    draw_text(data, img_w, img_h, &region_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    let dead_str = format!("DEAD ENDS: {}", config.dead_ends.label());
    draw_text(data, img_w, img_h, &dead_str, lx, ly, COL_VALUE);
    ly += LEGEND_LINE_H;

    if config.graph_shape == GraphShape::Branching {
        let branch_str = format!("BRANCH: {}", config.max_branch);
        draw_text(data, img_w, img_h, &branch_str, lx, ly, COL_VALUE);
    } else {
        draw_text(data, img_w, img_h, "BRANCH: N/A", lx, ly, COL_DIM);
    }

    // Controls column
    let mut ry = legend_y0 + LEGEND_PAD;

    draw_text(data, img_w, img_h, "N: NEW MAZE", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "R: RESTART", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "+/-: KEYS", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "[/]: VIEW", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "G: SHAPE", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "F: REGION SIZE", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "V: DEAD ENDS", rx, ry, COL_LABEL);
    ry += LEGEND_LINE_H;

    draw_text(data, img_w, img_h, "B: BRANCH WIDTH", rx, ry, COL_LABEL);
}

fn update_title(
    mut windows: Query<&mut Window>,
    gs: Res<GameState>,
    layout: Res<MazeLayout>,
) {
    let Ok(mut win) = windows.single_mut() else { return };
    let status = if gs.won { "WIN!" } else { "9 Keys" };
    win.title = format!("{status} | seed {}", layout.seed);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .insert_resource(ClearColor(Color::srgba_u8(
            COL_GAP[0], COL_GAP[1], COL_GAP[2], COL_GAP[3],
        )))
        .init_resource::<PadState>()
        .init_resource::<KeySheetPixels>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            read_gamepad,
            restart_or_new,
            movement,
            cache_key_sheet,
            auto_scale,
            render,
            update_title,
        ).chain())
        .run();
}
