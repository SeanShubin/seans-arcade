//! Render an LDtk map to PNG with swappable tilesets.
//!
//! Reads the LDtk JSON, extracts autotile placements (src rects + dest positions),
//! loads a tileset PNG (optionally different from the one LDtk references), and
//! composites the output image.
//!
//! Usage:
//!   cargo run --example ldtk_render
//!   cargo run --example ldtk_render -- --ldtk maps/reference.ldtk
//!   cargo run --example ldtk_render -- --wall "assets/external/64-bit/wall/Wall - Brick 1 64x64.png"
//!   cargo run --example ldtk_render -- --floor "assets/external/64-bit/floor/Floor - Grass 1 64x64.png"
//!   cargo run --example ldtk_render -- --out output.png

use image::{GenericImageView, RgbaImage};
use serde::Deserialize;

const DEFAULT_LDTK: &str = "maps/reference.ldtk";
const DEFAULT_OUT: &str = "generated/ldtk_render.png";

// ---------------------------------------------------------------------------
// LDtk JSON (minimal typed access)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LdtkRoot {
    #[serde(rename = "defaultLevelBgColor")]
    default_level_bg_color: String,
    levels: Vec<LdtkLevel>,
}

#[derive(Deserialize)]
struct LdtkLevel {
    #[serde(rename = "pxWid")]
    px_wid: u32,
    #[serde(rename = "pxHei")]
    px_hei: u32,
    #[serde(rename = "bgColor")]
    bg_color: Option<String>,
    #[serde(rename = "layerInstances")]
    layer_instances: Vec<LdtkLayer>,
}

#[derive(Deserialize)]
struct LdtkLayer {
    #[serde(rename = "__identifier")]
    identifier: String,
    #[serde(rename = "__cWid")]
    c_wid: u32,
    #[serde(rename = "__cHei")]
    c_hei: u32,
    #[serde(rename = "__gridSize")]
    grid_size: u32,
    #[serde(rename = "__tilesetRelPath")]
    tileset_rel_path: Option<String>,
    #[serde(rename = "autoLayerTiles")]
    auto_layer_tiles: Vec<LdtkAutoTile>,
}

#[derive(Deserialize)]
struct LdtkAutoTile {
    px: [u32; 2],
    src: [u32; 2],
    f: u8,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_hex_color(hex: &str) -> [u8; 4] {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    [r, g, b, 255]
}

/// Resolve a tileset path relative to the LDtk file's directory.
fn resolve_tileset_path(ldtk_path: &str, rel_path: &str) -> String {
    let ldtk_dir = std::path::Path::new(ldtk_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let resolved = ldtk_dir.join(rel_path);
    // Canonicalize to clean up ../.. segments
    match resolved.canonicalize() {
        Ok(p) => p.to_string_lossy().into_owned(),
        Err(_) => resolved.to_string_lossy().into_owned(),
    }
}

/// Blit a tile from a tileset onto the output image, handling flips.
fn blit_tile(
    output: &mut RgbaImage,
    tileset: &image::DynamicImage,
    src_x: u32,
    src_y: u32,
    dst_x: u32,
    dst_y: u32,
    size: u32,
    flip: u8,
) {
    let ts_w = tileset.width();
    let ts_h = tileset.height();

    for dy in 0..size {
        for dx in 0..size {
            let sx = src_x + dx;
            let sy = src_y + dy;
            if sx >= ts_w || sy >= ts_h {
                continue;
            }

            // Apply flip
            let out_dx = if flip & 1 != 0 { size - 1 - dx } else { dx };
            let out_dy = if flip & 2 != 0 { size - 1 - dy } else { dy };

            let ox = dst_x + out_dx;
            let oy = dst_y + out_dy;
            if ox >= output.width() || oy >= output.height() {
                continue;
            }

            let pixel = tileset.get_pixel(sx, sy);
            let [r, g, b, a] = pixel.0;
            if a == 0 {
                continue;
            }

            // Alpha-composite onto existing pixel
            if a == 255 {
                output.put_pixel(ox, oy, image::Rgba([r, g, b, 255]));
            } else {
                let dst = output.get_pixel(ox, oy);
                let af = a as f32 / 255.0;
                let inv = 1.0 - af;
                let nr = (r as f32 * af + dst[0] as f32 * inv) as u8;
                let ng = (g as f32 * af + dst[1] as f32 * inv) as u8;
                let nb = (b as f32 * af + dst[2] as f32 * inv) as u8;
                output.put_pixel(ox, oy, image::Rgba([nr, ng, nb, 255]));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

struct Args {
    ldtk: String,
    wall_tileset: Option<String>,
    floor_tileset: Option<String>,
    floor_tile: Option<(u32, u32)>,
    out: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let mut ldtk = DEFAULT_LDTK.to_string();
    let mut wall_tileset = None;
    let mut floor_tileset = None;
    let mut floor_tile = None;
    let mut out = DEFAULT_OUT.to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--ldtk" => { i += 1; ldtk = args[i].clone(); }
            "--wall" => { i += 1; wall_tileset = Some(args[i].clone()); }
            "--floor" => { i += 1; floor_tileset = Some(args[i].clone()); }
            "--floor-tile" => {
                // --floor-tile col,row (0-indexed into the floor tileset grid)
                i += 1;
                let parts: Vec<&str> = args[i].split(',').collect();
                if parts.len() == 2 {
                    let col: u32 = parts[0].parse().expect("floor-tile col");
                    let row: u32 = parts[1].parse().expect("floor-tile row");
                    floor_tile = Some((col, row));
                }
            }
            "--out" => { i += 1; out = args[i].clone(); }
            other => { eprintln!("Unknown arg: {other}"); }
        }
        i += 1;
    }

    Args { ldtk, wall_tileset, floor_tileset, floor_tile, out }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args = parse_args();

    // Parse LDtk
    let json_str = std::fs::read_to_string(&args.ldtk)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", args.ldtk));
    let root: LdtkRoot = serde_json::from_str(&json_str)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e}", args.ldtk));

    let level = &root.levels[0];
    let bg_color = parse_hex_color(
        level.bg_color.as_deref().unwrap_or(&root.default_level_bg_color),
    );

    println!("Level: {}x{} px", level.px_wid, level.px_hei);

    // Find layers with autotile data
    let mut wall_layer = None;
    let mut floor_layer = None;

    for layer in &level.layer_instances {
        if layer.auto_layer_tiles.is_empty() {
            continue;
        }
        // Heuristic: "Walls"/"IntGrid" → wall layer, "Floor" → floor layer
        match layer.identifier.as_str() {
            "Floor" => floor_layer = Some(layer),
            _ => {
                if wall_layer.is_none() {
                    wall_layer = Some(layer);
                }
            }
        }
    }

    // Load tilesets
    let wall_tileset_img = if let Some(ref path) = args.wall_tileset {
        Some(image::open(path).unwrap_or_else(|e| panic!("Failed to open wall tileset {path}: {e}")))
    } else if let Some(layer) = wall_layer {
        layer.tileset_rel_path.as_ref().map(|rel| {
            let path = resolve_tileset_path(&args.ldtk, rel);
            println!("Wall tileset: {path}");
            image::open(&path).unwrap_or_else(|e| panic!("Failed to open {path}: {e}"))
        })
    } else {
        None
    };

    let floor_tileset_img = if let Some(ref path) = args.floor_tileset {
        Some(image::open(path).unwrap_or_else(|e| panic!("Failed to open floor tileset {path}: {e}")))
    } else if let Some(layer) = floor_layer {
        layer.tileset_rel_path.as_ref().map(|rel| {
            let path = resolve_tileset_path(&args.ldtk, rel);
            println!("Floor tileset: {path}");
            image::open(&path).unwrap_or_else(|e| panic!("Failed to open {path}: {e}"))
        })
    } else {
        None
    };

    // Create output image
    let mut output = RgbaImage::new(level.px_wid, level.px_hei);

    // Fill with background color
    for pixel in output.pixels_mut() {
        *pixel = image::Rgba(bg_color);
    }

    // Determine grid size from the wall/intgrid layer
    let grid_size = wall_layer.map(|l| l.grid_size).unwrap_or(64);

    // Render floor tiles
    if let (Some(layer), Some(tileset)) = (floor_layer, &floor_tileset_img) {
        println!(
            "Rendering {} floor tiles ({}x{} grid, {}px)",
            layer.auto_layer_tiles.len(), layer.c_wid, layer.c_hei, layer.grid_size,
        );
        for tile in &layer.auto_layer_tiles {
            blit_tile(
                &mut output, tileset,
                tile.src[0], tile.src[1],
                tile.px[0], tile.px[1],
                layer.grid_size, tile.f,
            );
        }
    } else if let Some(ref tileset) = floor_tileset_img {
        // No floor layer in LDtk, but user provided a floor tileset — tile the whole level
        let (tile_col, tile_row) = args.floor_tile.unwrap_or((0, 0));
        let src_x = tile_col * grid_size;
        let src_y = tile_row * grid_size;
        println!("Tiling floor from tileset tile ({tile_col}, {tile_row})");
        let cols = level.px_wid / grid_size;
        let rows = level.px_hei / grid_size;
        for row in 0..rows {
            for col in 0..cols {
                blit_tile(
                    &mut output, tileset,
                    src_x, src_y,
                    col * grid_size, row * grid_size,
                    grid_size, 0,
                );
            }
        }
    }

    // Render wall tiles (on top of floor)
    if let (Some(layer), Some(tileset)) = (wall_layer, &wall_tileset_img) {
        println!(
            "Rendering {} wall tiles ({}x{} grid, {}px)",
            layer.auto_layer_tiles.len(), layer.c_wid, layer.c_hei, layer.grid_size,
        );
        for tile in &layer.auto_layer_tiles {
            blit_tile(
                &mut output, tileset,
                tile.src[0], tile.src[1],
                tile.px[0], tile.px[1],
                layer.grid_size, tile.f,
            );
        }
    }

    // Ensure output directory exists
    if let Some(parent) = std::path::Path::new(&args.out).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    output.save(&args.out)
        .unwrap_or_else(|e| panic!("Failed to save {}: {e}", args.out));

    println!("Saved {}", args.out);
}
