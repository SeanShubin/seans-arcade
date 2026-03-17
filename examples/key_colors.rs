//! Analyze key sprite colors from Fantasy Keys-sheet.png.
//!
//! Reads the 14×13 sprite sheet (64×64 cells), groups the 180 sprites into
//! 9 color categories (20 sprites each, left-to-right then top-to-bottom),
//! and computes per-channel median and mode for non-transparent pixels.
//!
//! Run with: `cargo run --example key_colors`

use std::collections::HashMap;

const CELL: u32 = 64;
const COLS: u32 = 14;
const SPRITES_PER_COLOR: usize = 20;
const COLOR_NAMES: [&str; 9] = [
    "Bronze", "Silver", "White", "Black", "Red", "Gold", "Green", "Blue", "Purple",
];

fn median(sorted: &[u8]) -> u8 {
    if sorted.is_empty() {
        return 0;
    }
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        ((sorted[mid - 1] as u16 + sorted[mid] as u16) / 2) as u8
    } else {
        sorted[mid]
    }
}

fn mode(values: &[u8]) -> u8 {
    let mut counts: HashMap<u8, usize> = HashMap::new();
    for &v in values {
        *counts.entry(v).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(val, _)| val)
        .unwrap_or(0)
}

fn main() {
    let path = "assets/external/64-bit/keys/Fantasy Keys-sheet.png";
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("Failed to open {path}: {e}"))
        .to_rgba8();

    let (img_w, img_h) = img.dimensions();
    println!("Image size: {img_w}×{img_h}");
    println!("Grid: {COLS} cols × {} rows, {CELL}×{CELL} cells\n", img_h / CELL);

    for (ci, color_name) in COLOR_NAMES.iter().enumerate() {
        let mut reds: Vec<u8> = Vec::new();
        let mut greens: Vec<u8> = Vec::new();
        let mut blues: Vec<u8> = Vec::new();
        let mut pixel_count: u64 = 0;
        let mut transparent_count: u64 = 0;

        // Per-pixel RGB histogram for full-color mode
        let mut rgb_counts: HashMap<(u8, u8, u8), usize> = HashMap::new();

        for si in 0..SPRITES_PER_COLOR {
            let sprite_index = ci * SPRITES_PER_COLOR + si;
            let grid_x = sprite_index as u32 % COLS;
            let grid_y = sprite_index as u32 / COLS;
            let px_x = grid_x * CELL;
            let px_y = grid_y * CELL;

            for dy in 0..CELL {
                for dx in 0..CELL {
                    let x = px_x + dx;
                    let y = px_y + dy;
                    if x >= img_w || y >= img_h {
                        continue;
                    }
                    let pixel = img.get_pixel(x, y);
                    let [r, g, b, a] = pixel.0;
                    if a < 128 {
                        transparent_count += 1;
                        continue;
                    }
                    reds.push(r);
                    greens.push(g);
                    blues.push(b);
                    *rgb_counts.entry((r, g, b)).or_insert(0) += 1;
                    pixel_count += 1;
                }
            }
        }

        reds.sort();
        greens.sort();
        blues.sort();

        let med_r = median(&reds);
        let med_g = median(&greens);
        let med_b = median(&blues);

        let mode_r = mode(&reds);
        let mode_g = mode(&greens);
        let mode_b = mode(&blues);

        // Full-color mode: the single (r,g,b) triple that appears most often
        let rgb_mode = rgb_counts
            .iter()
            .max_by_key(|&(_, count)| count)
            .map(|(&rgb, _)| rgb)
            .unwrap_or((0, 0, 0));

        // Top 5 most common colors
        let mut top_colors: Vec<_> = rgb_counts.iter().collect();
        top_colors.sort_by(|a, b| b.1.cmp(a.1));

        println!("=== {color_name} ===");
        println!(
            "  Pixels: {pixel_count} opaque, {transparent_count} transparent ({:.0}% transparent)",
            transparent_count as f64 / (pixel_count + transparent_count) as f64 * 100.0
        );
        println!("  Per-channel median: rgb({med_r}, {med_g}, {med_b})");
        println!("  Per-channel mode:   rgb({mode_r}, {mode_g}, {mode_b})");
        println!(
            "  Full-color mode:    rgb({}, {}, {})  (appears {} times)",
            rgb_mode.0, rgb_mode.1, rgb_mode.2,
            rgb_counts[&rgb_mode]
        );
        println!("  Top 5 colors:");
        for (i, ((r, g, b), count)) in top_colors.iter().take(5).enumerate() {
            let pct = **count as f64 / pixel_count as f64 * 100.0;
            println!("    {}. rgb({}, {}, {})  — {} px ({pct:.1}%)", i + 1, r, g, b, count);
        }
        println!();
    }
}
