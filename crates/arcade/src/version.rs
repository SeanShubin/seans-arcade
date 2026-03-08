//! Version check and auto-update.
//!
//! On startup: fetch the remote version file, compare to the compiled-in commit hash.
//! If an update is available, download the new binary, replace self, and restart.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use bevy::prelude::*;

const VERSION_URL: &str = "https://arcade.seanshubin.com/version";
const COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");

#[cfg(target_os = "windows")]
const BINARY_URL: &str = "https://arcade.seanshubin.com/windows/arcade.exe";
#[cfg(target_os = "macos")]
const BINARY_URL: &str = "https://arcade.seanshubin.com/macos/arcade";
#[cfg(target_os = "linux")]
const BINARY_URL: &str = "https://arcade.seanshubin.com/linux/arcade";

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum VersionStatus {
    UpToDate,
    UpdateAvailable { remote_hash: String },
    Offline,
}

/// Check the remote version file and return the status.
/// This is a blocking call intended to run before the Bevy app starts.
pub fn check_version() -> VersionStatus {
    match fetch_remote_hash() {
        Ok(remote_hash) => {
            if remote_hash == COMMIT_HASH {
                println!("version check: up to date ({COMMIT_HASH})");
                VersionStatus::UpToDate
            } else {
                println!(
                    "version check: update available (local={COMMIT_HASH}, remote={remote_hash})"
                );
                VersionStatus::UpdateAvailable { remote_hash }
            }
        }
        Err(e) => {
            println!("version check failed: {e}");
            VersionStatus::Offline
        }
    }
}

/// Download the new binary, replace the running executable, spawn it, and exit.
/// On failure, prints the error and returns so the app can continue with the current version.
pub fn auto_update() {
    if let Err(e) = do_auto_update() {
        println!("auto-update failed: {e}");
        println!("continuing with current version");
    }
}

/// Clean up leftover files from a previous Windows update.
pub fn cleanup_old_binary() {
    #[cfg(target_os = "windows")]
    {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let old = exe.with_file_name("arcade-old.exe");
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
    }
}

fn fetch_remote_hash() -> Result<String, Box<dyn std::error::Error>> {
    let body = ureq::get(VERSION_URL).call()?.body_mut().read_to_string()?;
    Ok(body.trim().to_string())
}

fn do_auto_update() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or("cannot determine executable directory")?;

    // Download to a temp file in the same directory (same filesystem for rename)
    let temp_path = exe_dir.join(temp_filename());
    println!("downloading update from {BINARY_URL}...");
    download_binary(&temp_path)?;

    // Platform-specific replacement
    replace_binary(&exe_path, &temp_path)?;

    // Spawn the new binary with the same arguments and exit
    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("restarting...");
    Command::new(&exe_path).args(&args).spawn()?;
    std::process::exit(0);
}

fn download_binary(dest: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut response = ureq::get(BINARY_URL).call()?;
    let mut file = fs::File::create(dest)?;
    let mut reader = response.body_mut().as_reader();
    std::io::copy(&mut reader, &mut file)?;
    file.flush()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn temp_filename() -> String {
    "arcade-new.exe".into()
}

#[cfg(not(target_os = "windows"))]
fn temp_filename() -> String {
    "arcade-new".into()
}

#[cfg(target_os = "windows")]
fn replace_binary(
    exe_path: &std::path::Path,
    temp_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Windows: cannot overwrite a running exe. Rename current to -old, rename new to current.
    let old_path = exe_path.with_file_name("arcade-old.exe");
    // Clean up any leftover from a previous failed update
    if old_path.exists() {
        fs::remove_file(&old_path)?;
    }
    fs::rename(exe_path, &old_path)?;
    fs::rename(temp_path, exe_path)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn replace_binary(
    exe_path: &std::path::Path,
    temp_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Unix: can overwrite a running binary (OS keeps inode alive until process exits).
    use std::os::unix::fs::PermissionsExt;
    fs::copy(temp_path, exe_path)?;
    fs::set_permissions(exe_path, fs::Permissions::from_mode(0o755))?;
    fs::remove_file(temp_path)?;
    Ok(())
}
