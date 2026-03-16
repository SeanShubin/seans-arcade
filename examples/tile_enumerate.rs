//! Enumerates the 70 distinct 5×5 tile configurations.
//!
//! Each tile has 4 sides with 2 possible entrance positions (at indices 1 and 3).
//! Rotational symmetry is factored out; mirrors are counted as distinct.
//!
//! Run with: `cargo run --example tile_enumerate`

/// Bit layout for the 8 entrances:
///   0=N1, 1=N3, 2=E1, 3=E3, 4=S1, 5=S3, 6=W1, 7=W3
fn rotate_90(config: u8) -> u8 {
    // 90° clockwise on a 5×5 grid:
    //   N1(0)→E1(2), N3(1)→E3(3), E1(2)→S3(5), E3(3)→S1(4),
    //   S1(4)→W1(6), S3(5)→W3(7), W1(6)→N3(1), W3(7)→N1(0)
    let bit = |b: u8, i: u8| (b >> i) & 1;
    (bit(config, 7))       // old W3 → new N1 (bit 0)
    | (bit(config, 6) << 1) // old W1 → new N3 (bit 1)
    | (bit(config, 0) << 2) // old N1 → new E1 (bit 2)
    | (bit(config, 1) << 3) // old N3 → new E3 (bit 3)
    | (bit(config, 3) << 4) // old E3 → new S1 (bit 4)
    | (bit(config, 2) << 5) // old E1 → new S3 (bit 5)
    | (bit(config, 4) << 6) // old S1 → new W1 (bit 6)
    | (bit(config, 5) << 7) // old S3 → new W3 (bit 7)
}

fn canonical(config: u8) -> u8 {
    let r1 = rotate_90(config);
    let r2 = rotate_90(r1);
    let r3 = rotate_90(r2);
    config.min(r1).min(r2).min(r3)
}

fn render(config: u8) -> [String; 5] {
    let bit = |i: u8| (config >> i) & 1 == 1;
    let n1 = bit(0); let n3 = bit(1);
    let e1 = bit(2); let e3 = bit(3);
    let s1 = bit(4); let s3 = bit(5);
    let w1 = bit(6); let w3 = bit(7);

    let wall = '#';
    let open = '.';

    let mut grid = [[wall; 5]; 5];

    // Interior is all open
    for r in 1..4 {
        for c in 1..4 {
            grid[r][c] = open;
        }
    }

    // North side (row 0): positions 1 and 3
    if n1 { grid[0][1] = open; }
    if n3 { grid[0][3] = open; }

    // South side (row 4): positions 1 and 3
    if s1 { grid[4][1] = open; }
    if s3 { grid[4][3] = open; }

    // West side (col 0): positions 1 and 3
    if w1 { grid[1][0] = open; }
    if w3 { grid[3][0] = open; }

    // East side (col 4): positions 1 and 3
    if e1 { grid[1][4] = open; }
    if e3 { grid[3][4] = open; }

    grid.map(|row| row.iter().collect())
}

fn entrance_label(config: u8) -> String {
    let bit = |i: u8| (config >> i) & 1 == 1;
    let mut parts = Vec::new();
    if bit(0) { parts.push("N1"); }
    if bit(1) { parts.push("N3"); }
    if bit(2) { parts.push("E1"); }
    if bit(3) { parts.push("E3"); }
    if bit(4) { parts.push("S1"); }
    if bit(5) { parts.push("S3"); }
    if bit(6) { parts.push("W1"); }
    if bit(7) { parts.push("W3"); }
    if parts.is_empty() { "none".to_string() } else { parts.join(" ") }
}

fn main() {
    let mut seen = std::collections::BTreeSet::new();
    let mut tiles: Vec<u8> = Vec::new();

    for config in 0u8..=255 {
        let canon = canonical(config);
        if seen.insert(canon) {
            tiles.push(canon);
        }
    }

    // Group by number of entrances
    tiles.sort_by_key(|c| (c.count_ones(), *c));

    println!("{} distinct tiles (rotational symmetry factored out)\n", tiles.len());

    for (i, &config) in tiles.iter().enumerate() {
        let grid = render(config);
        let entrances = config.count_ones();
        let label = entrance_label(config);

        // Count rotational variants
        let r1 = rotate_90(config);
        let r2 = rotate_90(r1);
        let r3 = rotate_90(r2);
        let mut variants = std::collections::BTreeSet::new();
        variants.insert(config);
        variants.insert(r1);
        variants.insert(r2);
        variants.insert(r3);
        let variant_count = variants.len();

        println!(
            "Tile {:>2}  ({} entrance{}, {} rotation{})  [{}]",
            i,
            entrances,
            if entrances == 1 { "" } else { "s" },
            variant_count,
            if variant_count == 1 { "" } else { "s" },
            label,
        );
        for row in &grid {
            println!("  {row}");
        }
        println!();
    }

    // Summary
    let by_count: Vec<usize> = (0..=8)
        .map(|n| tiles.iter().filter(|c| c.count_ones() == n).count())
        .collect();
    println!("By entrance count:");
    for (n, &count) in by_count.iter().enumerate() {
        if count > 0 {
            println!("  {n} entrances: {count} tiles");
        }
    }
}
