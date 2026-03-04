//! Shared types for the sprite metadata pipeline.
//!
//! This module defines the TOML-serializable types for:
//! - Pack manifests (input to the scanner)
//! - Sprite metadata (output of the scanner, input to AI/review)
//!
//! Include in examples with: `#[path = "sprite_meta.rs"] mod sprite_meta;`

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

// ===========================================================================
// Pack manifest types
// ===========================================================================

/// A pack manifest describes an asset pack for scanning.
#[derive(Debug, Serialize, Deserialize)]
pub struct PackManifest {
    pub name: String,
    pub root: String,
    pub sheets: Vec<PackSheet>,
    #[serde(default)]
    pub file_groups: Vec<FileGroup>,
    #[serde(default)]
    pub scale_groups: Vec<ScaleGroup>,
}

/// A sheet declaration in the pack manifest.
#[derive(Debug, Serialize, Deserialize)]
pub struct PackSheet {
    pub id: String,
    pub file: String,
    pub cell_w: u32,
    pub cell_h: u32,
}

/// A group of individual image files matched by glob pattern.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileGroup {
    pub glob: String,
    pub id_pattern: String,
}

/// Declares scale relationships between file groups.
#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleGroup {
    pub base_pattern: String,
    pub variants: Vec<ScaleVariant>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScaleVariant {
    pub pattern: String,
    pub scale: f64,
}

// ===========================================================================
// Pipeline config (drives discover, editor, and export)
// ===========================================================================

fn default_export_dir() -> String {
    "assets".to_string()
}

/// Config file listing multiple asset packs for the whole pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub asset_root: String,
    pub output_dir: String,
    #[serde(default = "default_export_dir")]
    pub export_dir: String,
    pub packs: Vec<PipelinePack>,
}

/// One asset pack entry in a pipeline config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelinePack {
    pub name: String,
    pub dir: String,
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl PipelineConfig {
    /// Absolute path to a pack's source image directory.
    pub fn pack_root(&self, pack: &PipelinePack) -> PathBuf {
        let dir_path = PathBuf::from(&pack.dir);
        if dir_path.is_absolute() {
            dir_path
        } else {
            PathBuf::from(&self.asset_root).join(&pack.dir)
        }
    }

    /// Path to a pack's metadata TOML file.
    pub fn meta_path(&self, pack: &PipelinePack) -> PathBuf {
        PathBuf::from(&self.output_dir).join(format!("{}.toml", pack.name))
    }

    /// Path to a pack's export output directory.
    pub fn export_path(&self, pack: &PipelinePack) -> PathBuf {
        PathBuf::from(&self.export_dir).join(&pack.name)
    }
}

// ===========================================================================
// Sprite metadata types (the TOML format)
// ===========================================================================

/// Top-level sprite metadata file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SpriteMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // Pipeline context — written by sprite_discover
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_root: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_sheet: Option<String>,

    // Discovery data — written by sprite_discover
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, ImageEntry>,

    // Enriched data — written by AI and sprite_analyze
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sheets: BTreeMap<String, Sheet>,
    #[serde(default)]
    pub catalog: BTreeMap<String, CatalogEntry>,
}

// ===========================================================================
// Discovery types (per-image mechanical facts)
// ===========================================================================

/// Per-image mechanical facts recorded by sprite_discover.
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageEntry {
    pub width: u32,
    pub height: u32,
    pub file_size_bytes: u64,
    pub color_count: u32,
    pub transparent_pct: u32,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid_cell_widths: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub valid_cell_heights: Vec<u32>,
}

/// A sprite sheet definition.
#[derive(Debug, Serialize, Deserialize)]
pub struct Sheet {
    pub file: String,
    pub cell_w: u32,
    pub cell_h: u32,
    pub cols: u32,
    pub rows: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transparent_pct: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A catalog entry — physical asset inventory.
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub sources: Vec<Source>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<CatalogDerivedFrom>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[u32; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
}

/// Mechanical relationship between catalog entries.
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogDerivedFrom {
    pub entry: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degrees: Option<f64>,
}

/// A physical source location for a sprite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col_span: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_span: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rect: Option<[u32; 4]>,
}

impl Source {
    /// Create a file source.
    pub fn file(path: impl Into<String>) -> Self {
        Source {
            file: Some(path.into()),
            sheet: None,
            col: None,
            row: None,
            col_span: None,
            row_span: None,
            rect: None,
        }
    }

    /// Create a sheet-cell source.
    pub fn sheet_cell(sheet: impl Into<String>, col: u32, row: u32) -> Self {
        Source {
            file: None,
            sheet: Some(sheet.into()),
            col: Some(col),
            row: Some(row),
            col_span: None,
            row_span: None,
            rect: None,
        }
    }

    /// Create a sheet-cell source with span.
    pub fn sheet_span(
        sheet: impl Into<String>,
        col: u32,
        row: u32,
        col_span: u32,
        row_span: u32,
    ) -> Self {
        Source {
            file: None,
            sheet: Some(sheet.into()),
            col: Some(col),
            row: Some(row),
            col_span: if col_span > 1 { Some(col_span) } else { None },
            row_span: if row_span > 1 { Some(row_span) } else { None },
            rect: None,
        }
    }
}

// ===========================================================================
// Verification
// ===========================================================================

/// Verify internal consistency of a metadata file.
/// Returns a list of error messages (empty = valid).
pub fn verify(meta: &SpriteMetadata) -> Vec<String> {
    let mut errors = Vec::new();

    // Every catalog source referencing a sheet must have that sheet defined
    for (id, entry) in &meta.catalog {
        for (i, source) in entry.sources.iter().enumerate() {
            if let Some(ref sheet_id) = source.sheet {
                if !meta.sheets.contains_key(sheet_id) {
                    errors.push(format!(
                        "catalog.{id}: source[{i}] references undefined sheet '{sheet_id}'"
                    ));
                }
            }
        }
        // derived_from must reference existing catalog entry
        if let Some(ref df) = entry.derived_from {
            if !meta.catalog.contains_key(&df.entry) {
                errors.push(format!(
                    "catalog.{id}: derived_from references undefined catalog entry '{}'",
                    df.entry
                ));
            }
        }
    }

    errors
}
