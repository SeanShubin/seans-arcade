//! Bevy plugin for loading runtime sprite metadata and providing cell-based lookup.
//!
//! Loads a runtime TOML (produced by `sprite_export`) and resolves cell locations
//! at load time so systems can look up sprites by sheet + cell coordinate.
//!
//! Usage:
//! ```rust
//! app.add_plugins(SpriteMetadataPlugin::new("castle.toml"));
//!
//! fn my_system(db: Res<SpriteDatabase>) {
//!     if let Some(loc) = db.cell("castle-walls", 3, 5) {
//!         // loc.image_path, loc.rect
//!     }
//! }
//! ```
//!
//! Include in examples with:
//! ```rust
//! #[path = "shared/sprite_runtime.rs"]
//! mod sprite_runtime;
//! ```

#![allow(dead_code)]

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

// ===========================================================================
// Runtime TOML types (matches sprite_export output)
// ===========================================================================

#[derive(Debug, Deserialize)]
struct RuntimeMetadata {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    sheets: BTreeMap<String, RuntimeSheet>,
    #[serde(default)]
    catalog: BTreeMap<String, RuntimeCatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct RuntimeSheet {
    file: String,
    cell_w: u32,
    cell_h: u32,
    cols: u32,
    rows: u32,
}

#[derive(Debug, Deserialize)]
struct RuntimeCatalogEntry {
    sources: Vec<RuntimeSource>,
}

#[derive(Debug, Deserialize)]
struct RuntimeSource {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    sheet: Option<String>,
    #[serde(default)]
    col: Option<u32>,
    #[serde(default)]
    row: Option<u32>,
    #[serde(default)]
    col_span: Option<u32>,
    #[serde(default)]
    row_span: Option<u32>,
}

// ===========================================================================
// Public types
// ===========================================================================

/// A resolved cell location — everything needed to render a sprite from a sheet.
#[derive(Debug, Clone)]
pub struct CellLocation {
    /// Path to the image file (relative to the asset directory).
    pub image_path: String,
    /// Pixel rectangle within the image [x, y, width, height].
    pub rect: [u32; 4],
}

/// Sheet grid dimensions.
#[derive(Debug, Clone)]
struct SheetInfo {
    cols: u32,
    rows: u32,
}

/// Database of pre-resolved cell locations, loaded from a runtime TOML.
#[derive(Resource, Debug, Default)]
pub struct SpriteDatabase {
    /// (sheet_id, col, row) → resolved location.
    cells: BTreeMap<(String, u32, u32), CellLocation>,
    /// Sheet ID → grid dimensions.
    sheets: BTreeMap<String, SheetInfo>,
    /// Pack name.
    pub name: Option<String>,
}

impl SpriteDatabase {
    /// Look up a cell by sheet ID and grid coordinates.
    pub fn cell(&self, sheet: &str, col: u32, row: u32) -> Option<&CellLocation> {
        self.cells.get(&(sheet.to_string(), col, row))
    }

    /// Get the grid dimensions (cols, rows) for a sheet.
    pub fn sheet_dims(&self, sheet: &str) -> Option<(u32, u32)> {
        self.sheets.get(sheet).map(|s| (s.cols, s.rows))
    }

    /// Iterate all sheet IDs.
    pub fn sheet_ids(&self) -> impl Iterator<Item = &String> {
        self.sheets.keys()
    }

    /// Number of cells in the database.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Check if the database is empty.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

// ===========================================================================
// Plugin
// ===========================================================================

/// Bevy plugin that loads a runtime sprite TOML and provides `SpriteDatabase`.
pub struct SpriteMetadataPlugin {
    toml_path: String,
}

impl SpriteMetadataPlugin {
    pub fn new(toml_path: impl Into<String>) -> Self {
        Self {
            toml_path: toml_path.into(),
        }
    }
}

impl Plugin for SpriteMetadataPlugin {
    fn build(&self, app: &mut App) {
        let db = load_sprite_database(&self.toml_path);
        app.insert_resource(db);
    }
}

fn load_sprite_database(path: &str) -> SpriteDatabase {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Cannot read sprite database {path}: {e}");
        return String::new();
    });

    if text.is_empty() {
        eprintln!("WARNING: Empty sprite database, using defaults");
        return SpriteDatabase::default();
    }

    let meta: RuntimeMetadata = toml::from_str(&text).unwrap_or_else(|e| {
        eprintln!("Cannot parse sprite database {path}: {e}");
        std::process::exit(1);
    });

    let mut db = SpriteDatabase {
        name: meta.name,
        ..Default::default()
    };

    // Build sheet info
    for (sheet_id, sheet) in &meta.sheets {
        db.sheets.insert(
            sheet_id.clone(),
            SheetInfo {
                cols: sheet.cols,
                rows: sheet.rows,
            },
        );
    }

    // Resolve each catalog entry → cell location
    for (_cat_id, entry) in &meta.catalog {
        let Some(source) = entry.sources.first() else {
            continue;
        };

        let Some(ref sheet_id) = source.sheet else {
            continue;
        };

        let Some(sheet) = meta.sheets.get(sheet_id) else {
            eprintln!("WARNING: Source references unknown sheet '{sheet_id}'");
            continue;
        };

        let col = source.col.unwrap_or(0);
        let row = source.row.unwrap_or(0);
        let col_span = source.col_span.unwrap_or(1);
        let row_span = source.row_span.unwrap_or(1);

        let location = CellLocation {
            image_path: sheet.file.clone(),
            rect: [
                col * sheet.cell_w,
                row * sheet.cell_h,
                col_span * sheet.cell_w,
                row_span * sheet.cell_h,
            ],
        };

        db.cells
            .insert((sheet_id.clone(), col, row), location);
    }

    eprintln!(
        "Loaded sprite database: {} cells across {} sheets",
        db.cells.len(),
        db.sheets.len(),
    );

    db
}
