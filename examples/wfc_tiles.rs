//! Wave Function Collapse tile placement.
//!
//! Generates a random NxN grid of 5×5 tiles where entrances align on all edges,
//! wrapping on both axes (torus topology).
//!
//! Usage: `cargo run --example wfc_tiles [size] [seed]`
//!   size: tiles per side (default 5)
//!   seed: RNG seed (default random)
//!
//! All 256 entrance configurations are used as the tile palette.
//! Each tile has 4 sides with 2 possible entrance positions (indices 1 and 3).

use rand::{Rng, RngExt, SeedableRng};
use std::collections::VecDeque;

// Bit layout: 0=N1, 1=N3, 2=E1, 3=E3, 4=S1, 5=S3, 6=W1, 7=W3
fn north_edge(t: u8) -> u8 { t & 0b11 }
fn east_edge(t: u8) -> u8 { (t >> 2) & 0b11 }
fn south_edge(t: u8) -> u8 { (t >> 4) & 0b11 }
fn west_edge(t: u8) -> u8 { (t >> 6) & 0b11 }

// ---------------------------------------------------------------------------
// 256-bit set (one bit per tile configuration)
// ---------------------------------------------------------------------------

type TileSet = [u64; 4];

fn ts_full() -> TileSet { [u64::MAX; 4] }
fn ts_empty() -> TileSet { [0; 4] }

fn ts_has(s: &TileSet, t: u8) -> bool {
    s[t as usize / 64] & (1u64 << (t as usize % 64)) != 0
}

fn ts_add(s: &mut TileSet, t: u8) {
    s[t as usize / 64] |= 1u64 << (t as usize % 64);
}

fn ts_count(s: &TileSet) -> u32 {
    s.iter().map(|w| w.count_ones()).sum()
}

fn ts_and(a: &TileSet, b: &TileSet) -> TileSet {
    [a[0] & b[0], a[1] & b[1], a[2] & b[2], a[3] & b[3]]
}

fn ts_or_into(a: &mut TileSet, b: &TileSet) {
    for i in 0..4 { a[i] |= b[i]; }
}

fn ts_singleton(t: u8) -> TileSet {
    let mut s = ts_empty();
    ts_add(&mut s, t);
    s
}

fn ts_pick(s: &TileSet, idx: u32) -> u8 {
    let mut remaining = idx;
    for t in 0..=255u8 {
        if ts_has(s, t) {
            if remaining == 0 { return t; }
            remaining -= 1;
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------
// Compatibility tables
// ---------------------------------------------------------------------------

/// For each edge value (0–3) and direction, which tiles have that edge value?
struct Compat {
    north: [TileSet; 4],
    south: [TileSet; 4],
    east: [TileSet; 4],
    west: [TileSet; 4],
}

impl Compat {
    fn new() -> Self {
        let mut c = Compat {
            north: [ts_empty(); 4],
            south: [ts_empty(); 4],
            east: [ts_empty(); 4],
            west: [ts_empty(); 4],
        };
        for t in 0..=255u8 {
            ts_add(&mut c.north[north_edge(t) as usize], t);
            ts_add(&mut c.south[south_edge(t) as usize], t);
            ts_add(&mut c.east[east_edge(t) as usize], t);
            ts_add(&mut c.west[west_edge(t) as usize], t);
        }
        c
    }

    /// Which tiles can go in a cell, given the possible tiles in its neighbor?
    /// `neighbor_tiles`: what the neighbor can be.
    /// `dir`: the direction FROM the cell TO that neighbor.
    ///   e.g. dir=North means the neighbor is to the north.
    fn allowed_by(&self, neighbor_tiles: &TileSet, dir: Dir) -> TileSet {
        // Collect edge values the neighbor can present on the shared edge
        let mut edge_vals = [false; 4];
        for t in 0..=255u8 {
            if !ts_has(neighbor_tiles, t) { continue; }
            let v = match dir {
                Dir::North => south_edge(t), // neighbor is north, its south faces us
                Dir::South => north_edge(t),
                Dir::East  => west_edge(t),
                Dir::West  => east_edge(t),
            } as usize;
            edge_vals[v] = true;
        }
        // Union all tiles that can match those edge values on our side
        let mut result = ts_empty();
        for (v, &present) in edge_vals.iter().enumerate() {
            if !present { continue; }
            let matching = match dir {
                Dir::North => &self.north[v], // our north must match
                Dir::South => &self.south[v],
                Dir::East  => &self.east[v],
                Dir::West  => &self.west[v],
            };
            ts_or_into(&mut result, matching);
        }
        result
    }
}

#[derive(Clone, Copy)]
enum Dir { North, South, East, West }

const DIRS: [Dir; 4] = [Dir::North, Dir::South, Dir::East, Dir::West];

// ---------------------------------------------------------------------------
// Grid
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Grid {
    size: usize,
    cells: Vec<TileSet>,
}

impl Grid {
    fn new(size: usize) -> Self {
        Grid { size, cells: vec![ts_full(); size * size] }
    }

    fn idx(&self, r: usize, c: usize) -> usize { r * self.size + c }

    fn neighbor(&self, r: usize, c: usize, dir: Dir) -> (usize, usize) {
        let n = self.size;
        match dir {
            Dir::North => ((r + n - 1) % n, c),
            Dir::South => ((r + 1) % n, c),
            Dir::East  => (r, (c + 1) % n),
            Dir::West  => (r, (c + n - 1) % n),
        }
    }

    fn opposite(dir: Dir) -> Dir {
        match dir {
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::East  => Dir::West,
            Dir::West  => Dir::East,
        }
    }

    /// Propagate constraints from a changed cell. Returns false on contradiction.
    fn propagate(&mut self, start_r: usize, start_c: usize, compat: &Compat) -> bool {
        let mut queue = VecDeque::new();
        queue.push_back((start_r, start_c));

        while let Some((r, c)) = queue.pop_front() {
            let current = self.cells[self.idx(r, c)];

            for dir in DIRS {
                let (nr, nc) = self.neighbor(r, c, dir);
                let ni = self.idx(nr, nc);

                // What can the neighbor be, given our possible tiles?
                let allowed = compat.allowed_by(&current, Self::opposite(dir));
                let new_set = ts_and(&self.cells[ni], &allowed);

                if new_set != self.cells[ni] {
                    if ts_count(&new_set) == 0 {
                        return false;
                    }
                    self.cells[ni] = new_set;
                    queue.push_back((nr, nc));
                }
            }
        }
        true
    }

    fn is_solved(&self) -> bool {
        self.cells.iter().all(|s| ts_count(s) == 1)
    }

    /// Find the unsolved cell with fewest possibilities. Break ties randomly.
    fn min_entropy_cell(&self, rng: &mut impl Rng) -> Option<(usize, usize)> {
        let mut best_count = u32::MAX;
        let mut candidates: Vec<(usize, usize)> = Vec::new();

        for r in 0..self.size {
            for c in 0..self.size {
                let count = ts_count(&self.cells[self.idx(r, c)]);
                if count <= 1 { continue; }
                if count < best_count {
                    best_count = count;
                    candidates.clear();
                    candidates.push((r, c));
                } else if count == best_count {
                    candidates.push((r, c));
                }
            }
        }

        if candidates.is_empty() {
            None
        } else {
            let i = rng.random_range(0..candidates.len());
            Some(candidates[i])
        }
    }

    fn resolved_tile(&self, r: usize, c: usize) -> u8 {
        ts_pick(&self.cells[self.idx(r, c)], 0)
    }
}

// ---------------------------------------------------------------------------
// Solver
// ---------------------------------------------------------------------------

fn solve(size: usize, rng: &mut impl Rng) -> Option<Grid> {
    let compat = Compat::new();
    let mut grid = Grid::new(size);

    loop {
        if grid.is_solved() { return Some(grid); }

        let (r, c) = grid.min_entropy_cell(rng)?;

        let possible = grid.cells[grid.idx(r, c)];
        let count = ts_count(&possible);
        let choice = rng.random_range(0..count);
        let tile = ts_pick(&possible, choice);

        let i = grid.idx(r, c);
        grid.cells[i] = ts_singleton(tile);
        if !grid.propagate(r, c, &compat) {
            return None; // contradiction — caller retries
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_tile(config: u8) -> [[char; 5]; 5] {
    let bit = |i: u8| (config >> i) & 1 == 1;
    let mut g = [['#'; 5]; 5];

    // Interior
    for r in 1..4 { for c in 1..4 { g[r][c] = '.'; } }

    // Entrances
    if bit(0) { g[0][1] = '.'; } // N1
    if bit(1) { g[0][3] = '.'; } // N3
    if bit(2) { g[1][4] = '.'; } // E1
    if bit(3) { g[3][4] = '.'; } // E3
    if bit(4) { g[4][1] = '.'; } // S1
    if bit(5) { g[4][3] = '.'; } // S3
    if bit(6) { g[1][0] = '.'; } // W1
    if bit(7) { g[3][0] = '.'; } // W3

    g
}

fn print_grid(grid: &Grid) {
    let n = grid.size;
    // Each tile is 5 rows tall, but adjacent tiles share wall rows.
    // Tile row spacing: 4 rows per tile (shared borders).
    let total_rows = n * 4 + 1;
    let total_cols = n * 4 + 1;

    let mut canvas = vec![vec![' '; total_cols]; total_rows];

    for tr in 0..n {
        for tc in 0..n {
            let tile = render_tile(grid.resolved_tile(tr, tc));
            let base_r = tr * 4;
            let base_c = tc * 4;
            for r in 0..5 {
                for c in 0..5 {
                    let cr = base_r + r;
                    let cc = base_c + c;
                    // For shared borders, wall wins over floor
                    if tile[r][c] == '#' || canvas[cr][cc] == ' ' {
                        canvas[cr][cc] = tile[r][c];
                    }
                }
            }
        }
    }

    for row in &canvas {
        let line: String = row.iter().collect();
        println!("{line}");
    }
}

fn verify(grid: &Grid) -> bool {
    let n = grid.size;
    for r in 0..n {
        for c in 0..n {
            let t = grid.resolved_tile(r, c);

            // Check east neighbor
            let east_t = grid.resolved_tile(r, (c + 1) % n);
            if east_edge(t) != west_edge(east_t) {
                eprintln!("Mismatch at ({r},{c}) east: {:02x} vs ({r},{}) west: {:02x}",
                    t, (c + 1) % n, east_t);
                return false;
            }

            // Check south neighbor
            let south_t = grid.resolved_tile((r + 1) % n, c);
            if south_edge(t) != north_edge(south_t) {
                eprintln!("Mismatch at ({r},{c}) south: {:02x} vs ({},{c}) north: {:02x}",
                    t, (r + 1) % n, south_t);
                return false;
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let size: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
    let seed: u64 = args.get(2).and_then(|s| s.parse().ok())
        .unwrap_or_else(|| rand::random());

    println!("Grid: {size}x{size}, seed: {seed}");

    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let start = std::time::Instant::now();

    let mut attempt = 0u32;
    let grid = loop {
        attempt += 1;
        if let Some(g) = solve(size, &mut rng) {
            break g;
        }
        if attempt % 100 == 0 {
            eprintln!("  {attempt} attempts...");
        }
    };

    let elapsed = start.elapsed();
    println!("Solved in {attempt} attempt{} ({elapsed:.2?})\n",
        if attempt == 1 { "" } else { "s" });

    let valid = verify(&grid);
    println!("Verification: {}\n", if valid { "PASS" } else { "FAIL" });

    if size <= 25 {
        print_grid(&grid);
    } else {
        println!("(grid too large for ASCII rendering, use size <= 25)");
    }

    // Stats
    let mut entrance_counts = [0u32; 9];
    for r in 0..size {
        for c in 0..size {
            let t = grid.resolved_tile(r, c);
            entrance_counts[t.count_ones() as usize] += 1;
        }
    }
    println!("\nEntrance distribution:");
    for (n, &count) in entrance_counts.iter().enumerate() {
        if count > 0 {
            println!("  {n} entrances: {count} tiles");
        }
    }
}
