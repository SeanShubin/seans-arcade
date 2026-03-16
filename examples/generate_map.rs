//! Generates a random map for grid_world using Wave Function Collapse.
//!
//! Uses LDtk-derived autotile rules (47 patterns per layer) for proper
//! wall and floor sprite selection.
//!
//! Usage: `cargo run --example generate_map [tiles_per_side] [output_path] [seed]`
//!   tiles_per_side: number of 5×5 rooms per side (default 3)
//!   output_path:    RON file to write (default maps/generated.ron)
//!   seed:           RNG seed (default random)
//!
//! Example: `cargo run --example generate_map 10 maps/big.ron 42`

use rand::{Rng, RngExt, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const ROOM_SIZE: usize = 5;

// ---------------------------------------------------------------------------
// RON map format (shared with grid_world and ldtk_convert)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct GridMap {
    cols: usize,
    rows: usize,
    start_col: usize,
    start_row: usize,
    cells: Vec<CellEntry>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
enum CellKind {
    Floor,
    Wall,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct CellEntry {
    kind: CellKind,
    src_x: u32,
    src_y: u32,
}

// ---------------------------------------------------------------------------
// Autotile — LDtk 3×3 pattern rule matching
// ---------------------------------------------------------------------------

/// LDtk autotile rules extracted from 9keys.ldtk.
/// Pattern layout: [NW, N, NE, W, center, E, SW, S, SE].
/// Values: -1 = not same type, 0 = don't care, 1 = same type.
/// First matching rule wins. Last rule is a catch-all fallback.

// Wall rules (47 rules, first match wins)
const WALL_RULES: &[([i8; 9], u32, u32)] = &[
    ([0,-1,0,-1,1,-1,0,-1,0], 384, 192),   // isolated
    ([-1,1,-1,1,1,1,-1,1,-1], 448, 0),      // all cardinals same, all corners diff
    ([0,-1,0,-1,1,1,0,1,-1], 0, 0),         // top-left outer corner
    ([0,-1,0,1,1,-1,-1,1,0], 64, 0),        // top-right outer corner
    ([-1,1,0,1,1,-1,0,-1,0], 64, 64),       // bottom-right outer corner
    ([0,1,-1,-1,1,1,0,-1,0], 0, 64),        // bottom-left outer corner
    ([-1,1,1,1,1,1,1,1,-1], 384, 256),      // NW+SE inner corners
    ([1,1,-1,1,1,1,-1,1,1], 384, 320),      // NE+SW inner corners
    ([0,1,-1,-1,1,1,0,1,1], 128, 256),
    ([0,-1,0,1,1,1,-1,1,1], 256, 256),
    ([-1,1,-1,1,1,1,-1,1,1], 0, 256),
    ([-1,1,0,1,1,-1,1,1,0], 192, 256),
    ([0,-1,0,1,1,1,1,1,-1], 320, 256),
    ([-1,1,-1,1,1,1,1,1,-1], 64, 256),
    ([1,1,-1,1,1,1,0,-1,0], 320, 320),
    ([1,1,0,1,1,-1,-1,1,0], 192, 320),
    ([1,1,-1,1,1,1,-1,1,-1], 64, 320),
    ([0,1,1,-1,1,1,0,1,-1], 128, 320),
    ([-1,1,1,1,1,1,0,-1,0], 256, 320),
    ([-1,1,1,1,1,1,-1,1,-1], 0, 320),
    ([-1,1,-1,1,1,1,0,-1,0], 256, 0),       // top T-junction
    ([0,1,-1,-1,1,1,0,1,-1], 192, 64),      // right T-junction
    ([0,-1,0,1,1,1,-1,1,-1], 192, 0),       // bottom T-junction
    ([-1,1,0,1,1,-1,-1,1,0], 256, 64),      // left T-junction
    ([-1,1,1,1,1,1,-1,1,1], 192, 192),      // NW inner corner only
    ([1,1,-1,1,1,1,1,1,-1], 128, 192),      // NE inner corner only
    ([1,1,1,1,1,1,-1,1,-1], 128, 128),      // SW inner corner only
    ([-1,1,-1,1,1,1,1,1,1], 192, 128),      // SE inner corner only
    ([0,-1,0,-1,1,0,0,-1,0], 384, 64),      // west cap
    ([0,-1,0,0,1,-1,0,-1,0], 320, 0),       // east cap
    ([0,-1,0,0,1,0,0,-1,0], 128, 64),       // vertical corridor
    ([0,-1,0,-1,1,-1,0,0,0], 320, 64),      // north cap
    ([0,0,0,-1,1,-1,0,-1,0], 384, 0),       // south cap
    ([0,0,0,-1,1,-1,0,0,0], 128, 0),        // horizontal corridor
    ([0,-1,0,-1,1,0,0,0,0], 448, 64),       // NW corner cap
    ([0,-1,0,0,1,-1,0,0,0], 448, 192),      // NE corner cap
    ([0,0,0,0,1,-1,0,-1,0], 448, 256),      // SE corner cap
    ([0,0,0,-1,1,0,0,-1,0], 448, 128),      // SW corner cap
    ([0,-1,0,0,1,0,0,0,0], 256, 192),       // north end
    ([0,0,0,0,1,-1,0,0,0], 320, 128),       // east end
    ([0,0,0,0,1,0,0,-1,0], 320, 192),       // south end
    ([0,0,0,-1,1,0,0,0,0], 256, 128),       // west end
    ([-1,1,0,1,1,0,0,0,0], 64, 192),        // NW diagonal
    ([0,1,-1,0,1,1,0,0,0], 0, 192),         // NE diagonal
    ([0,0,0,0,1,1,0,1,-1], 0, 128),         // SE diagonal
    ([0,0,0,1,1,0,-1,1,0], 64, 128),        // SW diagonal
    ([0,0,0,0,1,0,0,0,0], 384, 128),        // fallback
];

// Floor rules (47 rules, same patterns, different fallback tile)
const FLOOR_RULES: &[([i8; 9], u32, u32)] = &[
    ([0,-1,0,-1,1,-1,0,-1,0], 384, 192),
    ([-1,1,-1,1,1,1,-1,1,-1], 448, 0),
    ([0,-1,0,-1,1,1,0,1,-1], 0, 0),
    ([0,-1,0,1,1,-1,-1,1,0], 64, 0),
    ([-1,1,0,1,1,-1,0,-1,0], 64, 64),
    ([0,1,-1,-1,1,1,0,-1,0], 0, 64),
    ([-1,1,1,1,1,1,1,1,-1], 384, 256),
    ([1,1,-1,1,1,1,-1,1,1], 384, 320),
    ([0,1,-1,-1,1,1,0,1,1], 128, 256),
    ([0,-1,0,1,1,1,-1,1,1], 256, 256),
    ([-1,1,-1,1,1,1,-1,1,1], 0, 256),
    ([-1,1,0,1,1,-1,1,1,0], 192, 256),
    ([0,-1,0,1,1,1,1,1,-1], 320, 256),
    ([-1,1,-1,1,1,1,1,1,-1], 64, 256),
    ([1,1,-1,1,1,1,0,-1,0], 320, 320),
    ([1,1,0,1,1,-1,-1,1,0], 192, 320),
    ([1,1,-1,1,1,1,-1,1,-1], 64, 320),
    ([0,1,1,-1,1,1,0,1,-1], 128, 320),
    ([-1,1,1,1,1,1,0,-1,0], 256, 320),
    ([-1,1,1,1,1,1,-1,1,-1], 0, 320),
    ([-1,1,-1,1,1,1,0,-1,0], 256, 0),
    ([0,1,-1,-1,1,1,0,1,-1], 192, 64),
    ([0,-1,0,1,1,1,-1,1,-1], 192, 0),
    ([-1,1,0,1,1,-1,-1,1,0], 256, 64),
    ([-1,1,1,1,1,1,-1,1,1], 192, 192),
    ([1,1,-1,1,1,1,1,1,-1], 128, 192),
    ([1,1,1,1,1,1,-1,1,-1], 128, 128),
    ([-1,1,-1,1,1,1,1,1,1], 192, 128),
    ([0,-1,0,-1,1,0,0,-1,0], 384, 64),
    ([0,-1,0,0,1,-1,0,-1,0], 320, 0),
    ([0,-1,0,0,1,0,0,-1,0], 128, 64),
    ([0,-1,0,-1,1,-1,0,0,0], 320, 64),
    ([0,0,0,-1,1,-1,0,-1,0], 384, 0),
    ([0,0,0,-1,1,-1,0,0,0], 128, 0),
    ([0,-1,0,-1,1,0,0,0,0], 448, 64),
    ([0,-1,0,0,1,-1,0,0,0], 448, 192),
    ([0,0,0,0,1,-1,0,-1,0], 448, 256),
    ([0,0,0,-1,1,0,0,-1,0], 448, 128),
    ([0,-1,0,0,1,0,0,0,0], 256, 192),
    ([0,0,0,0,1,-1,0,0,0], 320, 128),
    ([0,0,0,0,1,0,0,-1,0], 320, 192),
    ([0,0,0,-1,1,0,0,0,0], 256, 128),
    ([-1,1,0,1,1,0,0,0,0], 64, 192),
    ([0,1,-1,0,1,1,0,0,0], 0, 192),
    ([0,0,0,0,1,1,0,1,-1], 0, 128),
    ([0,0,0,1,1,0,-1,1,0], 64, 128),
    ([0,0,0,0,1,0,0,0,0], 448, 320),        // fallback
];

/// Get the 8 neighbors as same/not-same relative to the cell at (row, col).
/// Returns [NW, N, NE, W, center(always 1), E, SW, S, SE].
fn neighbor_pattern(
    grid: &[CellKind], cols: usize, rows: usize, row: usize, col: usize,
) -> [i8; 9] {
    let target = grid[row * cols + col];
    let same = |r: usize, c: usize| -> i8 {
        if grid[r * cols + c] == target { 1 } else { -1 }
    };
    let n_r = (row + 1) % rows;        // north = visually up = higher row in Y-up grid
    let s_r = (row + rows - 1) % rows; // south = visually down = lower row
    let e_c = (col + 1) % cols;
    let w_c = (col + cols - 1) % cols;

    [
        same(n_r, w_c), same(n_r, col), same(n_r, e_c),
        same(row, w_c), 1,              same(row, e_c),
        same(s_r, w_c), same(s_r, col), same(s_r, e_c),
    ]
}

/// Match a cell's neighbor pattern against a rule table. First match wins.
fn match_rules(pattern: &[i8; 9], rules: &[([i8; 9], u32, u32)]) -> (u32, u32) {
    for &(ref rule, src_x, src_y) in rules {
        let mut matched = true;
        for i in 0..9 {
            // 0 = don't care, otherwise must match exactly
            if rule[i] != 0 && rule[i] != pattern[i] {
                matched = false;
                break;
            }
        }
        if matched {
            return (src_x, src_y);
        }
    }
    // Should never reach here since last rule is a catch-all
    (0, 0)
}

// ---------------------------------------------------------------------------
// WFC — entrance-matching tile placement (from wfc_tiles)
// ---------------------------------------------------------------------------

// Bit layout for WFC entrances: 0=N1, 1=N3, 2=E1, 3=E3, 4=S1, 5=S3, 6=W1, 7=W3
fn north_edge(t: u8) -> u8 { t & 0b11 }
fn east_edge(t: u8) -> u8 { (t >> 2) & 0b11 }
fn south_edge(t: u8) -> u8 { (t >> 4) & 0b11 }
fn west_edge(t: u8) -> u8 { (t >> 6) & 0b11 }

type TileSet = [u64; 4];
fn ts_full() -> TileSet { [u64::MAX; 4] }
fn ts_empty() -> TileSet { [0; 4] }
fn ts_has(s: &TileSet, t: u8) -> bool { s[t as usize / 64] & (1u64 << (t as usize % 64)) != 0 }
fn ts_add(s: &mut TileSet, t: u8) { s[t as usize / 64] |= 1u64 << (t as usize % 64); }
fn ts_count(s: &TileSet) -> u32 { s.iter().map(|w| w.count_ones()).sum() }
fn ts_and(a: &TileSet, b: &TileSet) -> TileSet { [a[0]&b[0], a[1]&b[1], a[2]&b[2], a[3]&b[3]] }
fn ts_or_into(a: &mut TileSet, b: &TileSet) { for i in 0..4 { a[i] |= b[i]; } }
fn ts_singleton(t: u8) -> TileSet { let mut s = ts_empty(); ts_add(&mut s, t); s }
fn ts_pick(s: &TileSet, idx: u32) -> u8 {
    let mut r = idx;
    for t in 0..=255u8 { if ts_has(s, t) { if r == 0 { return t; } r -= 1; } }
    unreachable!()
}

#[derive(Clone, Copy)]
enum Dir { North, South, East, West }
const DIRS: [Dir; 4] = [Dir::North, Dir::South, Dir::East, Dir::West];

struct Compat {
    north: [TileSet; 4], south: [TileSet; 4],
    east: [TileSet; 4],  west: [TileSet; 4],
}

impl Compat {
    fn new() -> Self {
        let mut c = Compat {
            north: [ts_empty(); 4], south: [ts_empty(); 4],
            east: [ts_empty(); 4],  west: [ts_empty(); 4],
        };
        for t in 0..=255u8 {
            ts_add(&mut c.north[north_edge(t) as usize], t);
            ts_add(&mut c.south[south_edge(t) as usize], t);
            ts_add(&mut c.east[east_edge(t) as usize], t);
            ts_add(&mut c.west[west_edge(t) as usize], t);
        }
        c
    }

    fn allowed_by(&self, neighbor_tiles: &TileSet, dir: Dir) -> TileSet {
        let mut edge_vals = [false; 4];
        for t in 0..=255u8 {
            if !ts_has(neighbor_tiles, t) { continue; }
            let v = match dir {
                Dir::North => south_edge(t),
                Dir::South => north_edge(t),
                Dir::East  => west_edge(t),
                Dir::West  => east_edge(t),
            } as usize;
            edge_vals[v] = true;
        }
        let mut result = ts_empty();
        for (v, &present) in edge_vals.iter().enumerate() {
            if !present { continue; }
            let matching = match dir {
                Dir::North => &self.north[v], Dir::South => &self.south[v],
                Dir::East  => &self.east[v],  Dir::West  => &self.west[v],
            };
            ts_or_into(&mut result, matching);
        }
        result
    }
}

fn opposite(dir: Dir) -> Dir {
    match dir {
        Dir::North => Dir::South, Dir::South => Dir::North,
        Dir::East => Dir::West, Dir::West => Dir::East,
    }
}

fn wfc_neighbor(size: usize, r: usize, c: usize, dir: Dir) -> (usize, usize) {
    let n = size;
    match dir {
        Dir::North => ((r + n - 1) % n, c), Dir::South => ((r + 1) % n, c),
        Dir::East => (r, (c + 1) % n),      Dir::West => (r, (c + n - 1) % n),
    }
}

fn wfc_solve(size: usize, rng: &mut impl Rng) -> Option<Vec<u8>> {
    let compat = Compat::new();
    let mut cells = vec![ts_full(); size * size];

    loop {
        // Check if solved
        if cells.iter().all(|s| ts_count(s) == 1) {
            return Some(cells.iter().map(|s| ts_pick(s, 0)).collect());
        }

        // Find min entropy cell (random tie-break)
        let mut best_count = u32::MAX;
        let mut candidates: Vec<usize> = Vec::new();
        for i in 0..cells.len() {
            let c = ts_count(&cells[i]);
            if c <= 1 { continue; }
            if c < best_count { best_count = c; candidates.clear(); candidates.push(i); }
            else if c == best_count { candidates.push(i); }
        }
        if candidates.is_empty() { return None; }

        let idx = candidates[rng.random_range(0..candidates.len())];
        let choice = rng.random_range(0..ts_count(&cells[idx]));
        let tile = ts_pick(&cells[idx], choice);
        cells[idx] = ts_singleton(tile);

        // Propagate
        let mut queue = VecDeque::new();
        queue.push_back(idx);
        while let Some(ci) = queue.pop_front() {
            let r = ci / size;
            let c = ci % size;
            let current = cells[ci];
            for dir in DIRS {
                let (nr, nc) = wfc_neighbor(size, r, c, dir);
                let ni = nr * size + nc;
                let allowed = compat.allowed_by(&current, opposite(dir));
                let new_set = ts_and(&cells[ni], &allowed);
                if new_set != cells[ni] {
                    if ts_count(&new_set) == 0 { return None; }
                    cells[ni] = new_set;
                    queue.push_back(ni);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tile expansion: WFC config → 5×5 cell grid
// ---------------------------------------------------------------------------

fn expand_tile(config: u8) -> [[CellKind; ROOM_SIZE]; ROOM_SIZE] {
    let bit = |i: u8| (config >> i) & 1 == 1;
    let mut g = [[CellKind::Wall; ROOM_SIZE]; ROOM_SIZE];

    // Interior is floor
    for r in 1..4 { for c in 1..4 { g[r][c] = CellKind::Floor; } }

    // Entrances — array convention: row 0 = north border, row 4 = south border.
    // Positions 1 and 3 are numbered from north along each edge.
    if bit(0) { g[0][1] = CellKind::Floor; } // N1
    if bit(1) { g[0][3] = CellKind::Floor; } // N3
    if bit(2) { g[1][4] = CellKind::Floor; } // E1
    if bit(3) { g[3][4] = CellKind::Floor; } // E3
    if bit(4) { g[4][1] = CellKind::Floor; } // S1
    if bit(5) { g[4][3] = CellKind::Floor; } // S3
    if bit(6) { g[1][0] = CellKind::Floor; } // W1
    if bit(7) { g[3][0] = CellKind::Floor; } // W3

    g
}

// ---------------------------------------------------------------------------
// Map assembly
// ---------------------------------------------------------------------------

fn build_cell_grid(wfc_tiles: &[u8], tiles_per_side: usize) -> (Vec<CellKind>, usize, usize) {
    let total = tiles_per_side * ROOM_SIZE;
    let mut grid = vec![CellKind::Wall; total * total];

    for tr in 0..tiles_per_side {
        for tc in 0..tiles_per_side {
            let config = wfc_tiles[tr * tiles_per_side + tc];
            let room = expand_tile(config);

            for lr in 0..ROOM_SIZE {
                for lc in 0..ROOM_SIZE {
                    let gr = tr * ROOM_SIZE + lr;
                    let gc = tc * ROOM_SIZE + lc;
                    grid[gr * total + gc] = room[lr][lc];
                }
            }
        }
    }

    (grid, total, total)
}

fn pick_start(grid: &[CellKind], cols: usize, rows: usize, rng: &mut impl Rng) -> (usize, usize) {
    let floors: Vec<(usize, usize)> = (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .filter(|&(r, c)| grid[r * cols + c] == CellKind::Floor)
        .collect();
    floors[rng.random_range(0..floors.len())]
}

fn assign_autotile(
    grid: &[CellKind],
    cols: usize,
    rows: usize,
) -> Vec<CellEntry> {
    (0..rows)
        .flat_map(|r| (0..cols).map(move |c| (r, c)))
        .map(|(r, c)| {
            let kind = grid[r * cols + c];
            let pattern = neighbor_pattern(grid, cols, rows, r, c);
            let rules = match kind {
                CellKind::Wall => WALL_RULES,
                CellKind::Floor => FLOOR_RULES,
            };
            let (src_x, src_y) = match_rules(&pattern, rules);
            CellEntry { kind, src_x, src_y }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let tiles_per_side: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let output_path = args.get(2).map(|s| s.as_str()).unwrap_or("maps/generated.ron");
    let seed: u64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(rand::random);

    println!("Generating {tiles_per_side}×{tiles_per_side} tile map ({}×{} cells)",
        tiles_per_side * ROOM_SIZE, tiles_per_side * ROOM_SIZE);
    println!("Output: {output_path}, seed: {seed}");

    // WFC
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let start = std::time::Instant::now();

    let mut attempt = 0u32;
    let wfc_tiles = loop {
        attempt += 1;
        if let Some(tiles) = wfc_solve(tiles_per_side, &mut rng) {
            break tiles;
        }
        if attempt % 100 == 0 {
            eprintln!("  {attempt} WFC attempts...");
        }
    };
    println!("WFC solved in {attempt} attempt{} ({:.2?})",
        if attempt == 1 { "" } else { "s" }, start.elapsed());

    // Expand to cell grid
    let (cell_kinds, cols, rows) = build_cell_grid(&wfc_tiles, tiles_per_side);
    let (start_row, start_col) = pick_start(&cell_kinds, cols, rows, &mut rng);

    // Assign autotile src coordinates
    let cells = assign_autotile(&cell_kinds, cols, rows);

    let map = GridMap { cols, rows, start_col, start_row, cells };

    // Write RON
    let ron_str = ron::ser::to_string_pretty(&map, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("Failed to serialize RON: {e}"));
    std::fs::write(output_path, &ron_str)
        .unwrap_or_else(|e| panic!("Failed to write {output_path}: {e}"));

    let floor_count = cell_kinds.iter().filter(|&&k| k == CellKind::Floor).count();
    let wall_count = cell_kinds.iter().filter(|&&k| k == CellKind::Wall).count();

    println!("\nWrote {output_path}");
    println!("  {cols}×{rows} cells, start at ({start_col}, {start_row})");
    println!("  {floor_count} floor, {wall_count} wall ({} total)", cols * rows);
}
