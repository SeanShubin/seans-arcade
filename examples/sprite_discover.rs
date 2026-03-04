//! Sprite discovery tool — the front door to the sprite pipeline.
//!
//! Point at a directory of downloaded assets, get a discovery TOML with
//! mechanical facts about every image file: dimensions, colors,
//! transparency, hash, and valid cell divisors.
//!
//! Redundant directories (RPG Maker variants, individual frames/icons) are
//! filtered out using the same `asset_browser.toml` skip list used by the
//! asset browser.
//!
//! No grid detection, no cell slicing, no catalog. Interpretation
//! belongs in the editor (step 2).
//!
//! Supports batch mode via a config file listing multiple packs.
//!
//! Usage:
//!   cargo run --example sprite_discover -- D:/assets/SomePack -o discovery.toml
//!   cargo run --example sprite_discover -- D:/assets/SomePack -o discovery.toml --contact contact.png
//!   cargo run --example sprite_discover -- D:/assets/SomePack -o discovery.toml --exclude "*.bak"
//!   cargo run --example sprite_discover -- --config discover.toml

#[path = "shared/sprite_meta.rs"]
mod sprite_meta;
#[path = "shared/sprite_analysis.rs"]
mod sprite_analysis;
#[path = "shared/scan_config.rs"]
mod scan_config;

use image::{ImageBuffer, Rgba, RgbaImage};
use sprite_analysis::*;
use sprite_meta::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ===========================================================================
// Image analysis
// ===========================================================================

/// Analyze a single image file and produce an ImageEntry.
fn analyze_image_entry(path: &Path, rgba: &RgbaImage) -> ImageEntry {
    let file_size_bytes = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    let stats = analyze_image(rgba);
    let hash_value = fnv1a_hash(rgba.as_raw());

    let w = rgba.width();
    let h = rgba.height();

    let valid_cell_widths: Vec<u32> = (8..=w).filter(|&s| w % s == 0).collect();
    let valid_cell_heights: Vec<u32> = (8..=h).filter(|&s| h % s == 0).collect();

    ImageEntry {
        width: w,
        height: h,
        file_size_bytes,
        color_count: stats.colors,
        transparent_pct: stats.transparent_pct,
        hash: format!("{:016x}", hash_value),
        valid_cell_widths,
        valid_cell_heights,
    }
}

// ===========================================================================
// Contact sheet generation (thumbnails of full images)
// ===========================================================================

const CONTACT_PADDING: u32 = 4;
const LABEL_HEIGHT: u32 = 14;
const BG_COLOR: Rgba<u8> = Rgba([40, 40, 40, 255]);
const LABEL_COLOR: Rgba<u8> = Rgba([255, 255, 200, 255]);

struct ThumbEntry {
    label: String,
    image: RgbaImage,
}

fn generate_contact_sheet(thumbs: &[ThumbEntry], output_path: &Path) {
    if thumbs.is_empty() {
        eprintln!("No images to generate contact sheet from.");
        return;
    }

    let max_w = thumbs.iter().map(|t| t.image.width()).max().unwrap_or(16);
    let max_h = thumbs.iter().map(|t| t.image.height()).max().unwrap_or(16);

    let cell_w = max_w + CONTACT_PADDING * 2;
    let cell_h = max_h + CONTACT_PADDING * 2 + LABEL_HEIGHT;

    // Pick column count based on number of images.
    let grid_cols = if thumbs.len() <= 4 {
        2
    } else if thumbs.len() <= 16 {
        4
    } else {
        8
    }
    .min(thumbs.len() as u32);

    let grid_rows = (thumbs.len() as u32 + grid_cols - 1) / grid_cols;

    let sheet_w = grid_cols * cell_w;
    let sheet_h = grid_rows * cell_h;

    eprintln!(
        "Contact sheet: {}x{} ({} cols x {} rows, cells {}x{})",
        sheet_w, sheet_h, grid_cols, grid_rows, cell_w, cell_h
    );

    let mut contact: RgbaImage = ImageBuffer::from_pixel(sheet_w, sheet_h, BG_COLOR);

    for (idx, thumb) in thumbs.iter().enumerate() {
        let gc = idx as u32 % grid_cols;
        let gr = idx as u32 / grid_cols;

        let cx = gc * cell_w;
        let cy = gr * cell_h;

        let offset_x = cx + CONTACT_PADDING + (max_w - thumb.image.width()) / 2;
        let offset_y = cy + CONTACT_PADDING + (max_h - thumb.image.height()) / 2;

        for (px, py, pixel) in thumb.image.enumerate_pixels() {
            let dx = offset_x + px;
            let dy = offset_y + py;
            if dx < sheet_w && dy < sheet_h && pixel[3] > 0 {
                let dst = contact.get_pixel(dx, dy);
                let blended = alpha_blend(*pixel, *dst);
                contact.put_pixel(dx, dy, blended);
            }
        }

        // Label bar
        let label_y = cy + CONTACT_PADDING * 2 + max_h;
        draw_label_bar(&mut contact, cx, label_y, cell_w, LABEL_HEIGHT, &thumb.label);
    }

    contact.save(output_path).unwrap_or_else(|e| {
        eprintln!("Cannot save contact sheet {}: {e}", output_path.display());
        std::process::exit(1);
    });
    eprintln!("Saved contact sheet to {}", output_path.display());

    // Text index
    let index_path = output_path.with_extension("txt");
    let mut index = String::new();
    index.push_str("# Contact sheet index\n");
    index.push_str(&format!("# Grid: {} cols x {} rows\n", grid_cols, grid_rows));
    index.push_str("# Position (col, row) -> image path -> pixel size\n\n");

    for (idx, thumb) in thumbs.iter().enumerate() {
        let gc = idx as u32 % grid_cols;
        let gr = idx as u32 / grid_cols;
        index.push_str(&format!(
            "({}, {}) {} ({}x{})\n",
            gc, gr, thumb.label, thumb.image.width(), thumb.image.height()
        ));
    }

    std::fs::write(&index_path, &index).unwrap_or_else(|e| {
        eprintln!("Cannot write index {}: {e}", index_path.display());
    });
    eprintln!("Saved index to {}", index_path.display());
}

fn alpha_blend(src: Rgba<u8>, dst: Rgba<u8>) -> Rgba<u8> {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a == 0.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let blend = |s: u8, d: u8| -> u8 {
        ((s as f32 * sa + d as f32 * da * (1.0 - sa)) / out_a) as u8
    };
    Rgba([
        blend(src[0], dst[0]),
        blend(src[1], dst[1]),
        blend(src[2], dst[2]),
        (out_a * 255.0) as u8,
    ])
}

fn draw_label_bar(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, text: &str) {
    let bar_color = Rgba([20, 20, 20, 220]);
    for py in y..y + h {
        for px in x..x + w {
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, bar_color);
            }
        }
    }

    let mut cx = x + 2;
    let cy = y + 2;
    for ch in text.chars() {
        if cx + 4 > x + w {
            break;
        }
        draw_tiny_char(img, cx, cy, ch, LABEL_COLOR);
        cx += 4;
    }
}

fn draw_tiny_char(img: &mut RgbaImage, x: u32, y: u32, ch: char, color: Rgba<u8>) {
    let bitmap = tiny_font_bitmap(ch);
    for (row, bits) in bitmap.iter().enumerate() {
        for col in 0..3 {
            if bits & (1 << (2 - col)) != 0 {
                let px = x + col as u32;
                let py = y + row as u32;
                if px < img.width() && py < img.height() {
                    img.put_pixel(px, py, color);
                }
            }
        }
    }
}

fn tiny_font_bitmap(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '_' => [0b000, 0b000, 0b000, 0b000, 0b111],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        'a' => [0b000, 0b111, 0b001, 0b111, 0b111],
        'b' => [0b100, 0b100, 0b111, 0b101, 0b111],
        'c' => [0b000, 0b111, 0b100, 0b100, 0b111],
        'd' => [0b001, 0b001, 0b111, 0b101, 0b111],
        'e' => [0b111, 0b101, 0b111, 0b100, 0b111],
        'f' => [0b011, 0b100, 0b111, 0b100, 0b100],
        'g' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'h' => [0b100, 0b100, 0b111, 0b101, 0b101],
        'i' => [0b010, 0b000, 0b010, 0b010, 0b010],
        'j' => [0b001, 0b000, 0b001, 0b101, 0b111],
        'k' => [0b100, 0b101, 0b110, 0b101, 0b101],
        'l' => [0b110, 0b010, 0b010, 0b010, 0b111],
        'm' => [0b000, 0b111, 0b111, 0b101, 0b101],
        'n' => [0b000, 0b111, 0b101, 0b101, 0b101],
        'o' => [0b000, 0b111, 0b101, 0b101, 0b111],
        'p' => [0b000, 0b111, 0b101, 0b111, 0b100],
        'q' => [0b000, 0b111, 0b101, 0b111, 0b001],
        'r' => [0b000, 0b111, 0b100, 0b100, 0b100],
        's' => [0b000, 0b111, 0b110, 0b011, 0b111],
        't' => [0b010, 0b111, 0b010, 0b010, 0b011],
        'u' => [0b000, 0b101, 0b101, 0b101, 0b111],
        'v' => [0b000, 0b101, 0b101, 0b101, 0b010],
        'w' => [0b000, 0b101, 0b101, 0b111, 0b111],
        'x' => [0b000, 0b101, 0b010, 0b101, 0b101],
        'y' => [0b000, 0b101, 0b111, 0b001, 0b111],
        'z' => [0b000, 0b111, 0b010, 0b100, 0b111],
        _ => [0b000, 0b000, 0b000, 0b000, 0b000],
    }
}

/// Scale an image to fit within thumb_size x thumb_size, preserving aspect ratio.
/// Uses nearest-neighbor for pixel art.
fn make_thumbnail(rgba: &RgbaImage, thumb_size: u32) -> RgbaImage {
    let w = rgba.width();
    let h = rgba.height();

    if w <= thumb_size && h <= thumb_size {
        return rgba.clone();
    }

    let scale = (thumb_size as f64 / w as f64).min(thumb_size as f64 / h as f64);
    let new_w = (w as f64 * scale).round().max(1.0) as u32;
    let new_h = (h as f64 * scale).round().max(1.0) as u32;

    image::imageops::resize(rgba, new_w, new_h, image::imageops::FilterType::Nearest)
}

// ===========================================================================
// CLI
// ===========================================================================

struct CliArgs {
    /// Single-pack mode: directory to scan.
    input_dir: Option<PathBuf>,
    output_path: Option<PathBuf>,
    contact_path: Option<PathBuf>,
    pack_name: Option<String>,
    exclude_patterns: Vec<String>,
    thumb_size: u32,
    /// Batch mode: config file path.
    config_path: Option<PathBuf>,
}

fn parse_cli() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();

    let mut input_dir = None;
    let mut output_path = None;
    let mut contact_path = None;
    let mut pack_name = None;
    let mut exclude_patterns = Vec::new();
    let mut thumb_size = 64u32;
    let mut config_path = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(PathBuf::from(&args[i]));
            }
            "-o" | "--output" => {
                i += 1;
                output_path = Some(PathBuf::from(&args[i]));
            }
            "--contact" => {
                i += 1;
                contact_path = Some(PathBuf::from(&args[i]));
            }
            "--name" => {
                i += 1;
                pack_name = Some(args[i].clone());
            }
            "--exclude" => {
                i += 1;
                exclude_patterns.push(args[i].clone());
            }
            "--thumb-size" => {
                i += 1;
                thumb_size = args[i].parse().unwrap_or_else(|_| {
                    eprintln!("Invalid thumb size: {}", args[i]);
                    std::process::exit(1);
                });
            }
            other if !other.starts_with('-') && input_dir.is_none() => {
                input_dir = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if config_path.is_none() && input_dir.is_none() {
        eprintln!("Usage: sprite_discover <directory> [-o output.toml] [--exclude \"*.bak\"]");
        eprintln!("       sprite_discover --config sprite-metadata/sprite-packs.toml");
        std::process::exit(1);
    }

    CliArgs {
        input_dir,
        output_path,
        contact_path,
        pack_name,
        exclude_patterns,
        thumb_size,
        config_path,
    }
}

// ===========================================================================
// Directory walking
// ===========================================================================

fn collect_png_files(dir: &Path, exclude: &[String]) -> Vec<PathBuf> {
    let pattern = dir.join("**/*.png");
    let pattern_str = pattern.to_string_lossy().replace('\\', "/");

    // Load directory-level skip list from asset_browser.toml.
    let config = scan_config::load_config(dir);
    let skip = &config.skip_directories;

    let mut files: Vec<PathBuf> = glob::glob(&pattern_str)
        .unwrap_or_else(|e| {
            eprintln!("Invalid glob pattern: {e}");
            std::process::exit(1);
        })
        .filter_map(|r| r.ok())
        .filter(|p| p.is_file())
        .filter(|p| {
            // Filter out files under skipped directories.
            let rel = p.strip_prefix(dir).unwrap_or(p);
            !rel.components().any(|c| {
                if let std::path::Component::Normal(name) = c {
                    if let Some(name_str) = name.to_str() {
                        let lower = name_str.to_ascii_lowercase();
                        return skip.iter().any(|s| lower == *s);
                    }
                }
                false
            })
        })
        .collect();

    // Apply exclude patterns — matched against relative path from input dir.
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
}

// ===========================================================================
// Core discovery logic
// ===========================================================================

struct DiscoverResult {
    output_path: Option<PathBuf>,
    pack_root: String,
    pack_name: String,
    file_count: usize,
}

/// Discover a single asset pack directory. Returns None if the directory
/// doesn't exist or has no PNGs (prints a warning instead of exiting).
fn discover_pack(
    input_dir: &Path,
    output_path: Option<&Path>,
    contact_path: Option<&Path>,
    pack_name: Option<&str>,
    exclude_patterns: &[String],
    thumb_size: u32,
) -> Option<DiscoverResult> {
    if !input_dir.is_dir() {
        eprintln!(
            "  WARN: directory does not exist: {} — skipping",
            input_dir.display()
        );
        return None;
    }

    let files = collect_png_files(input_dir, exclude_patterns);
    if files.is_empty() {
        eprintln!(
            "  WARN: no PNG files found in {} — skipping",
            input_dir.display()
        );
        return None;
    }

    eprintln!("Found {} PNG files in {}", files.len(), input_dir.display());

    let pack_name = pack_name
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            input_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

    // Load all images and analyze them.
    let mut images: BTreeMap<String, ImageEntry> = BTreeMap::new();
    let mut contact_thumbs: Vec<ThumbEntry> = Vec::new();

    for file_path in &files {
        let rel_path = file_path
            .strip_prefix(input_dir)
            .unwrap_or(file_path);
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");

        let rgba = load_rgba_image(file_path);

        eprintln!(
            "  {} ({}x{}, {} bytes)",
            rel_str,
            rgba.width(),
            rgba.height(),
            std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0),
        );

        let entry = analyze_image_entry(file_path, &rgba);
        images.insert(rel_str.clone(), entry);

        if contact_path.is_some() {
            let thumb = make_thumbnail(&rgba, thumb_size);
            let label = format!("{} {}x{}", rel_str, rgba.width(), rgba.height());
            contact_thumbs.push(ThumbEntry { label, image: thumb });
        }
    }

    // Generate contact sheet if requested.
    if let Some(contact_path) = contact_path {
        generate_contact_sheet(&contact_thumbs, contact_path);
    }

    // Build unified metadata with pipeline context.
    let abs_root = std::fs::canonicalize(input_dir)
        .unwrap_or_else(|_| input_dir.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_string();

    let meta = SpriteMetadata {
        name: Some(pack_name.clone()),
        pack_root: Some(abs_root.clone()),
        exclude: exclude_patterns.to_vec(),
        contact_sheet: contact_path.map(|p| {
            p.to_string_lossy().replace('\\', "/")
        }),
        images,
        ..Default::default()
    };

    // Serialize.
    let output = toml::to_string_pretty(&meta).unwrap_or_else(|e| {
        eprintln!("Failed to serialize TOML: {e}");
        std::process::exit(1);
    });

    if let Some(path) = output_path {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        std::fs::write(path, &output).unwrap_or_else(|e| {
            eprintln!("Failed to write {}: {e}", path.display());
            std::process::exit(1);
        });
        eprintln!("Wrote metadata to {}", path.display());
    } else {
        print!("{output}");
    }

    Some(DiscoverResult {
        output_path: output_path.map(|p| p.to_path_buf()),
        pack_root: abs_root,
        pack_name,
        file_count: files.len(),
    })
}

// ===========================================================================
// Batch config loading
// ===========================================================================

fn load_pipeline_config(path: &Path) -> PipelineConfig {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Cannot read config {}: {e}", path.display());
        std::process::exit(1);
    });
    toml::from_str::<PipelineConfig>(&text).unwrap_or_else(|e| {
        eprintln!("Failed to parse config {}: {e}", path.display());
        std::process::exit(1);
    })
}

// ===========================================================================
// Output
// ===========================================================================

fn print_next_step(toml_path: Option<&Path>, pack_root: &str) {
    let toml_display = toml_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(stdout)".into());

    eprintln!();
    eprintln!("=== Next step ===");
    eprintln!("Open the grid tool to define grids:");
    eprintln!(
        "  cargo run --example sprite_grid -- {toml_display} --pack-root {pack_root}"
    );
    eprintln!();
}

fn print_batch_summary(results: &[DiscoverResult], config_path: &Path) {
    eprintln!();
    eprintln!("=== Batch discovery complete ===");
    for r in results {
        let path_display = r
            .output_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(stdout)".into());
        eprintln!(
            "  [OK] {} — {} files -> {}",
            r.pack_name, r.file_count, path_display
        );
    }

    eprintln!();
    eprintln!("=== Next step ===");
    eprintln!(
        "  cargo run --example sprite_grid -- --config {}",
        config_path.display()
    );
    eprintln!();
}

// ===========================================================================
// Main
// ===========================================================================

fn main() {
    let cli = parse_cli();

    if let Some(ref config_path) = cli.config_path {
        // === Batch mode ===
        let config = load_pipeline_config(config_path);
        let output_dir = PathBuf::from(&config.output_dir);

        std::fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
            eprintln!(
                "Cannot create output directory {}: {e}",
                output_dir.display()
            );
            std::process::exit(1);
        });

        let mut results: Vec<DiscoverResult> = Vec::new();

        for pack in &config.packs {
            let input_dir = config.pack_root(pack);
            let output_path = config.meta_path(pack);

            eprintln!();
            eprintln!("=== Discovering: {} ===", pack.name);

            if let Some(result) = discover_pack(
                &input_dir,
                Some(&output_path),
                None,
                Some(&pack.name),
                &pack.exclude,
                cli.thumb_size,
            ) {
                results.push(result);
            }
        }

        print_batch_summary(&results, config_path);
    } else if let Some(ref input_dir) = cli.input_dir {
        // === Single mode ===
        if let Some(result) = discover_pack(
            input_dir,
            cli.output_path.as_deref(),
            cli.contact_path.as_deref(),
            cli.pack_name.as_deref(),
            &cli.exclude_patterns,
            cli.thumb_size,
        ) {
            print_next_step(result.output_path.as_deref(), &result.pack_root);
        } else {
            std::process::exit(1);
        }
    }
}
