//! Shared directory-filtering config for the sprite pipeline.
//!
//! Both `asset_browser` and `sprite_discover` use `BrowserConfig` to skip
//! redundant directories (RPG Maker variants, individual frames, individual
//! icons) before scanning for PNGs.
//!
//! The config is stored as `asset_browser.toml` in the asset root directory.
//! If the file doesn't exist, one is created with sensible defaults.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const CONFIG_FILENAME: &str = "asset_browser.toml";

#[derive(Serialize, Deserialize)]
pub struct BrowserConfig {
    pub skip_directories: Vec<String>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            skip_directories: vec![
                "rpgmaker".into(),
                "rpgmakermv".into(),
                "rmmv".into(),
                "rmvx".into(),
                "frames".into(),
                "individual_16x16".into(),
                "individual_24x24".into(),
                "individual_32x32".into(),
            ],
        }
    }
}

pub fn load_config(root: &Path) -> BrowserConfig {
    let path = root.join(CONFIG_FILENAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<BrowserConfig>(&text) {
            Ok(cfg) => {
                eprintln!("Loaded config from {}", path.display());
                cfg
            }
            Err(e) => {
                eprintln!("Failed to parse {}: {e} — using defaults", path.display());
                BrowserConfig::default()
            }
        },
        Err(_) => {
            let config = BrowserConfig::default();
            let toml_str = toml::to_string_pretty(&config).expect("failed to serialize defaults");
            if let Err(e) = std::fs::write(&path, &toml_str) {
                eprintln!("Could not write default config to {}: {e}", path.display());
            } else {
                eprintln!("Created default config at {}", path.display());
            }
            config
        }
    }
}
