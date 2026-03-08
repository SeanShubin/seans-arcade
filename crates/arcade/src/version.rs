//! Version check: compare compiled-in commit hash against the remote version file.

use bevy::prelude::*;

const VERSION_URL: &str = "https://arcade.seanshubin.com/version";
const COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");

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

fn fetch_remote_hash() -> Result<String, Box<dyn std::error::Error>> {
    let body = ureq::get(VERSION_URL).call()?.body_mut().read_to_string()?;
    Ok(body.trim().to_string())
}
