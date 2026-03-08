//! Asset manifest: download assets from S3 on startup, only fetching changed files.
//!
//! The remote manifest at `assets-manifest.json` lists each asset's relative path and SHA-256 hash.
//! On startup, compare the remote manifest to the local one (stored in the data directory).
//! Download only changed or missing assets. Store them in the data directory alongside the manifest.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

const ASSETS_BASE_URL: &str = "https://arcade.seanshubin.com/assets";
const MANIFEST_URL: &str = "https://arcade.seanshubin.com/assets-manifest.json";
const MANIFEST_FILENAME: &str = "assets-manifest.json";

/// Resource holding the path to the local assets directory.
#[derive(Resource)]
pub struct AssetsDir(pub PathBuf);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetEntry {
    pub file: String,
    pub hash: String,
}

pub type Manifest = Vec<AssetEntry>;

/// Sync assets from S3 to the local data directory.
/// Returns the path to the local assets directory.
/// On network failure, uses whatever is already cached locally.
pub fn sync_assets(data_dir: &Path) -> PathBuf {
    let assets_dir = data_dir.join("assets");
    fs::create_dir_all(&assets_dir).expect("failed to create assets directory");

    match do_sync(&assets_dir) {
        Ok(count) => {
            if count > 0 {
                println!("asset sync: downloaded {count} file(s)");
            } else {
                println!("asset sync: all assets up to date");
            }
        }
        Err(e) => {
            println!("asset sync failed: {e}");
            println!("using cached assets");
        }
    }

    assets_dir
}

fn do_sync(assets_dir: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let remote_manifest = fetch_remote_manifest()?;
    let local_manifest = load_local_manifest(assets_dir);

    let mut downloaded = 0;
    for entry in &remote_manifest {
        let local_entry = local_manifest.iter().find(|e| e.file == entry.file);
        let needs_download = match local_entry {
            Some(local) => local.hash != entry.hash,
            None => true,
        };

        if needs_download {
            download_asset(assets_dir, &entry.file)?;
            downloaded += 1;
        }
    }

    // Remove assets that are no longer in the remote manifest
    for local_entry in &local_manifest {
        if !remote_manifest.iter().any(|e| e.file == local_entry.file) {
            let path = assets_dir.join(&local_entry.file);
            let _ = fs::remove_file(&path);
        }
    }

    save_local_manifest(assets_dir, &remote_manifest)?;
    Ok(downloaded)
}

fn fetch_remote_manifest() -> Result<Manifest, Box<dyn std::error::Error>> {
    let body = ureq::get(MANIFEST_URL).call()?.body_mut().read_to_string()?;
    let manifest: Manifest = serde_json::from_str(&body)?;
    Ok(manifest)
}

fn load_local_manifest(assets_dir: &Path) -> Manifest {
    let path = assets_dir.join(MANIFEST_FILENAME);
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

fn save_local_manifest(
    assets_dir: &Path,
    manifest: &Manifest,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(manifest)?;
    fs::write(assets_dir.join(MANIFEST_FILENAME), json)?;
    Ok(())
}

fn download_asset(assets_dir: &Path, relative_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{ASSETS_BASE_URL}/{relative_path}");
    let dest = assets_dir.join(relative_path);

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    println!("downloading asset: {relative_path}");
    let mut response = ureq::get(&url).call()?;
    let mut file = fs::File::create(&dest)?;
    let mut reader = response.body_mut().as_reader();
    std::io::copy(&mut reader, &mut file)?;
    file.flush()?;
    Ok(())
}
