//! Shared analysis functions for the sprite pipeline.
//!
//! Common image analysis logic used by discover, grid, and export tools.
//!
//! Include in examples with: `#[path = "shared/sprite_analysis.rs"] mod sprite_analysis;`

#![allow(dead_code)]

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Common cell sizes for pixel art sprite sheets.
pub const COMMON_CELL_SIZES: &[u32] = &[8, 16, 24, 32, 48, 64];

// ===========================================================================
// Image loading
// ===========================================================================

pub fn load_rgba_image(path: &Path) -> image::RgbaImage {
    let img = image::open(path).unwrap_or_else(|e| {
        eprintln!("Cannot open image {}: {e}", path.display());
        std::process::exit(1);
    });
    img.to_rgba8()
}

// ===========================================================================
// Cell operations
// ===========================================================================

pub fn crop_cell(
    img: &image::RgbaImage,
    col: u32,
    row: u32,
    cell_w: u32,
    cell_h: u32,
) -> image::RgbaImage {
    let x = col * cell_w;
    let y = row * cell_h;
    image::imageops::crop_imm(img, x, y, cell_w, cell_h).to_image()
}

pub fn is_cell_occupied(rgba: &image::RgbaImage, x0: u32, y0: u32, w: u32, h: u32) -> bool {
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            if rgba.get_pixel(x, y).0[3] > 0 {
                return true;
            }
        }
    }
    false
}

// ===========================================================================
// Hashing
// ===========================================================================

pub fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ===========================================================================
// Image-level analysis
// ===========================================================================

pub struct ImageStats {
    pub colors: u32,
    pub transparent_pct: u32,
}

pub fn analyze_image(img: &image::RgbaImage) -> ImageStats {
    let total = (img.width() * img.height()) as u64;
    let mut transparent = 0u64;
    let mut unique_colors: HashSet<[u8; 4]> = HashSet::new();

    for pixel in img.pixels() {
        if pixel.0[3] == 0 {
            transparent += 1;
        }
        unique_colors.insert(pixel.0);
    }

    ImageStats {
        colors: unique_colors.len() as u32,
        transparent_pct: if total > 0 {
            (transparent * 100 / total) as u32
        } else {
            100
        },
    }
}

// ===========================================================================
// Cell-level analysis
// ===========================================================================

pub struct CellAnalysis {
    pub empty: bool,
    pub bbox: Option<[u32; 4]>,
    pub pixels: Option<u32>,
    pub colors: Option<u32>,
    pub hash: Option<String>,
}

pub fn analyze_cell(img: &image::RgbaImage) -> CellAnalysis {
    let w = img.width();
    let h = img.height();
    let mut non_transparent = 0u32;
    let mut unique_colors: HashSet<[u8; 4]> = HashSet::new();
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;

    for (x, y, pixel) in img.enumerate_pixels() {
        if pixel.0[3] != 0 {
            non_transparent += 1;
            unique_colors.insert(pixel.0);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if non_transparent == 0 {
        return CellAnalysis {
            empty: true,
            bbox: None,
            pixels: None,
            colors: None,
            hash: None,
        };
    }

    let hash_value = fnv1a_hash(img.as_raw());

    CellAnalysis {
        empty: false,
        bbox: Some([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1]),
        pixels: Some(non_transparent),
        colors: Some(unique_colors.len() as u32),
        hash: Some(format!("{:016x}", hash_value)),
    }
}

// ===========================================================================
// Grid detection
// ===========================================================================

/// A valid grid candidate for an image.
pub struct GridCandidate {
    pub cell_w: u32,
    pub cell_h: u32,
    pub cols: u32,
    pub rows: u32,
}

/// Find all valid grid sizes from COMMON_CELL_SIZES for the given image dimensions.
pub fn find_valid_grids(width: u32, height: u32) -> Vec<GridCandidate> {
    let mut valid = Vec::new();

    // Square cells
    for &size in COMMON_CELL_SIZES {
        if width % size == 0 && height % size == 0 {
            valid.push(GridCandidate {
                cell_w: size,
                cell_h: size,
                cols: width / size,
                rows: height / size,
            });
        }
    }

    // Rectangular combinations
    for &cw in COMMON_CELL_SIZES {
        for &ch in COMMON_CELL_SIZES {
            if cw == ch {
                continue;
            }
            if width % cw == 0 && height % ch == 0 {
                valid.push(GridCandidate {
                    cell_w: cw,
                    cell_h: ch,
                    cols: width / cw,
                    rows: height / ch,
                });
            }
        }
    }

    valid
}

/// Build an occupancy grid for the given image and cell size.
/// Returns (occupied_count, total_cells).
pub fn occupancy_grid(
    rgba: &image::RgbaImage,
    cell_w: u32,
    cell_h: u32,
) -> (u32, u32) {
    let cols = rgba.width() / cell_w;
    let rows = rgba.height() / cell_h;
    let total = cols * rows;
    let mut occupied = 0u32;

    for row in 0..rows {
        for col in 0..cols {
            let x = col * cell_w;
            let y = row * cell_h;
            if is_cell_occupied(rgba, x, y, cell_w, cell_h) {
                occupied += 1;
            }
        }
    }

    (occupied, total)
}

// ===========================================================================
// Valid cell sizes
// ===========================================================================

/// All divisors of `dim` that are >= 8.
pub fn valid_cell_sizes(dim: u32) -> Vec<u32> {
    (8..=dim).filter(|&s| dim % s == 0).collect()
}

// ===========================================================================
// PNG file collection
// ===========================================================================

/// Glob `**/*.png` under `dir`, apply exclude patterns and skip-directory
/// filtering, return paths relative to `dir`.
pub fn collect_png_files(dir: &Path, exclude: &[String], skip_directories: &[String]) -> Vec<String> {
    let pattern = dir.join("**/*.png");
    let pattern_str = pattern.to_string_lossy().replace('\\', "/");

    let mut files: Vec<PathBuf> = glob::glob(&pattern_str)
        .unwrap_or_else(|e| {
            eprintln!("Invalid glob pattern: {e}");
            std::process::exit(1);
        })
        .filter_map(|r| r.ok())
        .filter(|p| p.is_file())
        .filter(|p| {
            let rel = p.strip_prefix(dir).unwrap_or(p);
            !rel.components().any(|c| {
                if let std::path::Component::Normal(name) = c {
                    if let Some(name_str) = name.to_str() {
                        let lower = name_str.to_ascii_lowercase();
                        return skip_directories.iter().any(|s| lower == *s);
                    }
                }
                false
            })
        })
        .collect();

    // Apply exclude patterns — matched against relative path from dir.
    let match_options = glob::MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };
    for exc in exclude {
        files.retain(|p| {
            let rel = p
                .strip_prefix(dir)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            !glob::Pattern::new(exc)
                .map(|pat| pat.matches_with(&rel, match_options))
                .unwrap_or(false)
        });
    }

    files.sort();
    files
        .into_iter()
        .map(|p| {
            p.strip_prefix(dir)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}
