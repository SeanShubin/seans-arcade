//! Tiles a source image into a grid and saves the result.
//!
//! Usage: `cargo run --example tile_image -- <source> <dest> <cols>x<rows>`
//!
//! Example: `cargo run --example tile_image -- tile.bmp tiled.bmp 5x7`

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!("Usage: {} <source> <dest> <cols>x<rows>", args[0]);
        std::process::exit(1);
    }

    let source = PathBuf::from(&args[1]);
    let dest = PathBuf::from(&args[2]);

    let (cols, rows) = parse_grid(&args[3]);

    let tile = image::open(&source)
        .unwrap_or_else(|e| panic!("Failed to open {}: {e}", source.display()));

    let tw = tile.width();
    let th = tile.height();
    let mut output = image::RgbaImage::new(tw * cols, th * rows);

    for row in 0..rows {
        for col in 0..cols {
            image::imageops::overlay(&mut output, &tile.to_rgba8(), (col * tw) as i64, (row * th) as i64);
        }
    }

    output
        .save(&dest)
        .unwrap_or_else(|e| panic!("Failed to save {}: {e}", dest.display()));

    println!(
        "{}x{} tile -> {}x{} output ({cols}x{rows}) -> {}",
        tw, th, tw * cols, th * rows, dest.display()
    );
}

fn parse_grid(s: &str) -> (u32, u32) {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        eprintln!("Grid must be <cols>x<rows>, e.g. 5x7");
        std::process::exit(1);
    }
    let cols: u32 = parts[0].parse().expect("cols must be a number");
    let rows: u32 = parts[1].parse().expect("rows must be a number");
    (cols, rows)
}
