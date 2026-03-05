//! Sprite export tool — the back door of the sprite pipeline.
//!
//! Reads a metadata TOML file and produces game-ready output:
//! 1. Runtime TOML — stripped version with only what the game needs
//! 2. Copied assets — sheet images in the output directory
//!
//! Usage:
//!   cargo run --example sprite_export -- castle.toml --pack-root D:/assets/SomePack -o assets/castle
//!   cargo run --example sprite_export -- --config sprite-metadata/sprite-packs.toml

#[path = "shared/sprite_meta.rs"]
mod sprite_meta;
#[path = "shared/sprite_analysis.rs"]
mod sprite_analysis;

use sprite_analysis::*;
use sprite_meta::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

// ===========================================================================
// Runtime TOML types (stripped version of SpriteMetadata)
// ===========================================================================

/// Runtime sprite metadata — only what the game needs.
#[derive(Debug, serde::Serialize)]
struct RuntimeMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    walk_defaults: Option<WalkDefaults>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    sheets: BTreeMap<String, RuntimeSheet>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    catalog: BTreeMap<String, RuntimeCatalogEntry>,
}

/// Runtime sheet — just file path and grid info.
#[derive(Debug, serde::Serialize)]
struct RuntimeSheet {
    file: String,
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    rows: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
}

/// Runtime catalog entry — only sources, no analysis.
#[derive(Debug, serde::Serialize)]
struct RuntimeCatalogEntry {
    sources: Vec<Source>,
}

// ===========================================================================
// CLI
// ===========================================================================

enum CliMode {
    Single {
        meta_path: PathBuf,
        pack_root: PathBuf,
        output_dir: PathBuf,
    },
    Batch {
        config_path: PathBuf,
    },
}

fn parse_cli() -> CliMode {
    let args: Vec<String> = std::env::args().collect();

    let mut config_path = None;
    let mut meta_path = None;
    let mut pack_root = None;
    let mut output_dir = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                config_path = Some(PathBuf::from(&args[i]));
            }
            "--pack-root" => {
                i += 1;
                pack_root = Some(PathBuf::from(&args[i]));
            }
            "-o" | "--output" => {
                i += 1;
                output_dir = Some(PathBuf::from(&args[i]));
            }
            other if !other.starts_with('-') && meta_path.is_none() => {
                meta_path = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown argument: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    if let Some(config_path) = config_path {
        CliMode::Batch { config_path }
    } else if let Some(meta_path) = meta_path {
        CliMode::Single {
            meta_path,
            pack_root: pack_root.unwrap_or_else(|| {
                eprintln!("--pack-root is required");
                std::process::exit(1);
            }),
            output_dir: output_dir.unwrap_or_else(|| {
                eprintln!("-o / --output is required");
                std::process::exit(1);
            }),
        }
    } else {
        eprintln!("Usage: sprite_export <metadata.toml> --pack-root <dir> -o <output-dir>");
        eprintln!("       sprite_export --config sprite-metadata/sprite-packs.toml");
        std::process::exit(1);
    }
}

// ===========================================================================
// Export one pack
// ===========================================================================

struct ExportResult {
    pack_name: String,
    cells: usize,
    sheets: usize,
    copied: usize,
}

/// Export a single pack. Returns None on recoverable errors (for batch mode).
fn export_pack(
    meta_path: &Path,
    pack_root: &Path,
    output_dir: &Path,
) -> Option<ExportResult> {
    // Read metadata
    let meta_text = match std::fs::read_to_string(meta_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("  WARN: Cannot read {}: {e} — skipping", meta_path.display());
            return None;
        }
    };
    let meta: SpriteMetadata = match toml::from_str(&meta_text) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("  WARN: Cannot parse {}: {e} — skipping", meta_path.display());
            return None;
        }
    };

    let pack_name = meta.name.clone().unwrap_or_else(|| "(unnamed)".into());

    // Skip if no sheets (not yet gridded)
    if meta.sheets.is_empty() {
        eprintln!(
            "  WARN: {} has no sheets — run editor first, skipping",
            meta_path.display()
        );
        return None;
    }

    // Build runtime metadata by recomputing catalog from sheets + PNGs
    let mut runtime = RuntimeMetadata {
        name: meta.name.clone(),
        walk_defaults: meta.walk_defaults.clone(),
        sheets: BTreeMap::new(),
        catalog: BTreeMap::new(),
    };

    // Global hash dedup across all sheets
    let mut seen_hashes: BTreeMap<u64, String> = BTreeMap::new();

    for (sheet_id, sheet) in &meta.sheets {
        let img_path = pack_root.join(&sheet.file);
        let rgba = load_rgba_image(&img_path);
        let cols = rgba.width() / sheet.cell_w;
        let rows = rgba.height() / sheet.cell_h;

        runtime.sheets.insert(
            sheet_id.clone(),
            RuntimeSheet {
                file: sheet.file.clone(),
                cell_w: sheet.cell_w,
                cell_h: sheet.cell_h,
                cols,
                rows,
                category: sheet.category.clone(),
            },
        );

        // Build a set of cells covered by spans
        let mut spanned: BTreeSet<(u32, u32)> = BTreeSet::new();
        for span in &sheet.spans {
            for r in span.row..(span.row + span.row_span) {
                for c in span.col..(span.col + span.col_span) {
                    spanned.insert((c, r));
                }
            }
        }

        // Create catalog entries for spans
        for span in &sheet.spans {
            let x = span.col * sheet.cell_w;
            let y = span.row * sheet.cell_h;
            let w = span.col_span * sheet.cell_w;
            let h = span.row_span * sheet.cell_h;

            if !is_cell_occupied(&rgba, x, y, w, h) {
                continue;
            }

            let span_img = image::imageops::crop_imm(&rgba, x, y, w, h).to_image();
            let hash = fnv1a_hash(span_img.as_raw());

            let entry_id = format!("{sheet_id}.{}.{}", span.col, span.row);
            if let Some(_existing) = seen_hashes.get(&hash) {
                continue; // skip duplicate
            }
            seen_hashes.insert(hash, entry_id.clone());

            runtime.catalog.insert(
                entry_id,
                RuntimeCatalogEntry {
                    sources: vec![Source::sheet_span(
                        sheet_id,
                        span.col,
                        span.row,
                        span.col_span,
                        span.row_span,
                    )],
                },
            );
        }

        // Create catalog entries for individual occupied, non-spanned cells
        for row in 0..rows {
            for col in 0..cols {
                if spanned.contains(&(col, row)) {
                    continue;
                }

                let x = col * sheet.cell_w;
                let y = row * sheet.cell_h;

                if !is_cell_occupied(&rgba, x, y, sheet.cell_w, sheet.cell_h) {
                    continue;
                }

                let cell_img = crop_cell(&rgba, col, row, sheet.cell_w, sheet.cell_h);
                let hash = fnv1a_hash(cell_img.as_raw());

                let entry_id = format!("{sheet_id}.{col}.{row}");
                if let Some(_existing) = seen_hashes.get(&hash) {
                    continue; // skip duplicate
                }
                seen_hashes.insert(hash, entry_id.clone());

                runtime.catalog.insert(
                    entry_id,
                    RuntimeCatalogEntry {
                        sources: vec![Source::sheet_cell(sheet_id, col, row)],
                    },
                );
            }
        }
    }

    if runtime.catalog.is_empty() {
        eprintln!(
            "  WARN: {} produced no catalog entries — skipping",
            meta_path.display()
        );
        return None;
    }

    eprintln!(
        "Computed {} catalog entries from {} sheets",
        runtime.catalog.len(),
        runtime.sheets.len()
    );

    // Create output directory
    std::fs::create_dir_all(output_dir).unwrap_or_else(|e| {
        eprintln!(
            "Cannot create output directory {}: {e}",
            output_dir.display()
        );
        std::process::exit(1);
    });

    // Write runtime TOML
    let toml_path = output_dir.join(
        meta_path
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("sprites.toml")),
    );
    let toml_output = toml::to_string_pretty(&runtime).unwrap_or_else(|e| {
        eprintln!("Failed to serialize runtime TOML: {e}");
        std::process::exit(1);
    });
    std::fs::write(&toml_path, &toml_output).unwrap_or_else(|e| {
        eprintln!("Failed to write {}: {e}", toml_path.display());
        std::process::exit(1);
    });
    eprintln!("Wrote runtime TOML to {}", toml_path.display());

    // Copy sheet images to output directory
    let mut copied = 0;
    for sheet in runtime.sheets.values() {
        let src = pack_root.join(&sheet.file);
        let dst = output_dir.join(&sheet.file);

        // Create parent directory if needed
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        match std::fs::copy(&src, &dst) {
            Ok(_) => copied += 1,
            Err(e) => {
                eprintln!(
                    "WARNING: Cannot copy {} → {}: {e}",
                    src.display(),
                    dst.display()
                );
            }
        }
    }

    // Copy standalone file sources too
    for entry in runtime.catalog.values() {
        for source in &entry.sources {
            if let Some(ref file_path) = source.file {
                let src = pack_root.join(file_path);
                let dst = output_dir.join(file_path);
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                match std::fs::copy(&src, &dst) {
                    Ok(_) => copied += 1,
                    Err(e) => {
                        eprintln!(
                            "WARNING: Cannot copy {} → {}: {e}",
                            src.display(),
                            dst.display()
                        );
                    }
                }
            }
        }
    }

    eprintln!(
        "Copied {} asset files to {}",
        copied,
        output_dir.display()
    );

    Some(ExportResult {
        pack_name,
        cells: runtime.catalog.len(),
        sheets: runtime.sheets.len(),
        copied,
    })
}

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
// Main
// ===========================================================================

fn main() {
    let cli = parse_cli();

    match cli {
        CliMode::Batch { ref config_path } => {
            let config = load_pipeline_config(config_path);

            eprintln!("=== Batch export: {} packs ===", config.packs.len());

            let mut results: Vec<ExportResult> = Vec::new();

            for pack in &config.packs {
                let meta_path = config.meta_path(pack);
                let pack_root = config.pack_root(pack);
                let output_dir = config.export_path(pack);

                eprintln!();
                eprintln!("=== Exporting: {} ===", pack.name);

                if let Some(result) = export_pack(&meta_path, &pack_root, &output_dir) {
                    results.push(result);
                }
            }

            eprintln!();
            eprintln!("=== Batch export complete ===");
            for r in &results {
                eprintln!(
                    "  [OK] {} — {} cells, {} sheets, {} assets copied",
                    r.pack_name, r.cells, r.sheets, r.copied
                );
            }
            eprintln!();
        }
        CliMode::Single {
            ref meta_path,
            ref pack_root,
            ref output_dir,
        } => {
            if let Some(result) = export_pack(meta_path, pack_root, output_dir) {
                eprintln!(
                    "Export complete: {} cells, {} sheets, {} assets copied",
                    result.cells,
                    result.sheets,
                    result.copied,
                );
            } else {
                std::process::exit(1);
            }
        }
    }
}
