//! Identity registry: maps identity names to secrets.
//!
//! For v1 local dev: stores plaintext secrets. Hash with SHA-256 later.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum ValidationResult {
    NewRegistration,
    Accepted,
    NameClaimed,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IdentityRegistry {
    #[serde(flatten)]
    entries: HashMap<String, String>,
}

impl IdentityRegistry {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let contents = toml::to_string_pretty(&self).expect("failed to serialize registry");
        std::fs::write(path, contents).expect("failed to write registry");
    }

    pub fn validate(
        &mut self,
        name: &str,
        secret: &str,
        new_secret: Option<&str>,
    ) -> ValidationResult {
        match self.entries.get(name) {
            None => {
                self.entries.insert(name.to_string(), secret.to_string());
                ValidationResult::NewRegistration
            }
            Some(stored) if stored == secret => {
                if let Some(new) = new_secret {
                    self.entries.insert(name.to_string(), new.to_string());
                }
                ValidationResult::Accepted
            }
            Some(_) => ValidationResult::NameClaimed,
        }
    }

    pub fn path_from_data_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("identity_registry.toml")
    }
}
