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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_name_registration_succeeds() {
        // given an empty registry
        let mut registry = IdentityRegistry::default();

        // when we validate a new name
        let result = registry.validate("alice", "secret123", None);

        // then it returns NewRegistration
        assert_eq!(result, ValidationResult::NewRegistration);
    }

    #[test]
    fn same_name_correct_secret_returns_accepted() {
        // given a registry with alice registered
        let mut registry = IdentityRegistry::default();
        registry.validate("alice", "secret123", None);

        // when we validate the same name with the correct secret
        let result = registry.validate("alice", "secret123", None);

        // then it returns Accepted
        assert_eq!(result, ValidationResult::Accepted);
    }

    #[test]
    fn same_name_wrong_secret_returns_name_claimed() {
        // given a registry with alice registered
        let mut registry = IdentityRegistry::default();
        registry.validate("alice", "secret123", None);

        // when we validate the same name with a wrong secret
        let result = registry.validate("alice", "wrong_secret", None);

        // then it returns NameClaimed
        assert_eq!(result, ValidationResult::NameClaimed);
    }

    #[test]
    fn secret_rotation_updates_stored_secret() {
        // given a registry with alice registered
        let mut registry = IdentityRegistry::default();
        registry.validate("alice", "old_secret", None);

        // when we validate with the correct secret and provide a new_secret
        let result = registry.validate("alice", "old_secret", Some("new_secret"));

        // then it returns Accepted
        assert_eq!(result, ValidationResult::Accepted);
    }

    #[test]
    fn after_rotation_old_secret_is_rejected_new_secret_works() {
        // given a registry with alice whose secret has been rotated
        let mut registry = IdentityRegistry::default();
        registry.validate("alice", "old_secret", None);
        registry.validate("alice", "old_secret", Some("new_secret"));

        // when we validate with the old secret
        let old_result = registry.validate("alice", "old_secret", None);

        // then the old secret is rejected
        assert_eq!(old_result, ValidationResult::NameClaimed);

        // when we validate with the new secret
        let new_result = registry.validate("alice", "new_secret", None);

        // then the new secret is accepted
        assert_eq!(new_result, ValidationResult::Accepted);
    }

    #[test]
    fn load_from_nonexistent_file_returns_empty_registry() {
        // given a path that does not exist
        let path = Path::new("/nonexistent/path/identity_registry.toml");

        // when we load from that path
        let registry = IdentityRegistry::load(path);

        // then the registry is empty (validate any name returns NewRegistration)
        let mut registry = registry;
        let result = registry.validate("anyone", "any_secret", None);
        assert_eq!(result, ValidationResult::NewRegistration);
    }

    #[test]
    fn save_then_load_roundtrip_preserves_entries() {
        // given a registry with multiple entries
        let mut registry = IdentityRegistry::default();
        registry.validate("alice", "secret_a", None);
        registry.validate("bob", "secret_b", None);
        registry.validate("charlie", "secret_c", None);

        // when we save and reload
        let dir = std::env::temp_dir().join(format!(
            "relay_identity_test_{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("identity_registry.toml");
        registry.save(&path);
        let mut loaded = IdentityRegistry::load(&path);

        // then all entries are preserved (correct secrets accepted)
        assert_eq!(
            loaded.validate("alice", "secret_a", None),
            ValidationResult::Accepted
        );
        assert_eq!(
            loaded.validate("bob", "secret_b", None),
            ValidationResult::Accepted
        );
        assert_eq!(
            loaded.validate("charlie", "secret_c", None),
            ValidationResult::Accepted
        );

        // and wrong secrets are rejected
        assert_eq!(
            loaded.validate("alice", "wrong", None),
            ValidationResult::NameClaimed
        );

        // cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }
}
