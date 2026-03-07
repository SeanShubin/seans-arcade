//! Client configuration: identity, relay address, secrets.
//!
//! Config is stored at `%APPDATA%\seans-arcade\config.toml` by default,
//! overridable with `--config-dir`.

use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub identity_name: String,
    #[serde(default)]
    pub identity_secret: String,
    #[serde(default)]
    pub new_identity_secret: Option<String>,
    #[serde(default = "default_relay_address")]
    pub relay_address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_secret: Option<String>,
}

fn default_relay_address() -> String {
    "127.0.0.1:7700".into()
}

pub fn data_dir_from_args() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() - 1 {
        if args[i] == "--data-dir" {
            return PathBuf::from(&args[i + 1]);
        }
    }
    default_config_dir()
}

fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("no config directory found")
        .join("seans-arcade")
}

pub fn load_config(dir: &Path) -> Config {
    let path = dir.join("config.toml");
    match std::fs::read_to_string(&path) {
        Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
        Err(_) => Config {
            relay_address: default_relay_address(),
            ..Default::default()
        },
    }
}

pub fn save_config(dir: &Path, config: &Config) {
    std::fs::create_dir_all(dir).expect("failed to create config directory");
    let contents = toml::to_string_pretty(config).expect("failed to serialize config");
    std::fs::write(dir.join("config.toml"), contents).expect("failed to write config");
}

const BIP39_ENGLISH: &str = include_str!("bip39_english.txt");

pub fn generate_identity_secret() -> String {
    let words: Vec<&str> = BIP39_ENGLISH.lines().collect();
    let mut rng = rand::rng();
    (0..4)
        .map(|_| words[rng.random_range(0..words.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults() {
        let config = Config::default();
        assert_eq!(config.identity_name, "");
        assert_eq!(config.identity_secret, "");
        assert_eq!(config.relay_address, "");
        assert_eq!(config.relay_secret, None);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("arcade_config_test_{}", line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config {
            identity_name: "alice".into(),
            identity_secret: "secret words here now".into(),
            new_identity_secret: None,
            relay_address: "10.0.0.1:9999".into(),
            relay_secret: Some("relay_pass".into()),
        };
        save_config(&dir, &config);
        let loaded = load_config(&dir);
        assert_eq!(loaded.identity_name, "alice");
        assert_eq!(loaded.identity_secret, "secret words here now");
        assert_eq!(loaded.relay_address, "10.0.0.1:9999");
        assert_eq!(loaded.relay_secret, Some("relay_pass".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_from_nonexistent_dir() {
        let dir = std::env::temp_dir().join(format!("arcade_config_test_{}", line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = load_config(&dir);
        assert_eq!(config.relay_address, "127.0.0.1:7700");
    }

    #[test]
    fn relay_secret_none_not_serialized() {
        let dir = std::env::temp_dir().join(format!("arcade_config_test_{}", line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config {
            relay_secret: None,
            ..Default::default()
        };
        save_config(&dir, &config);
        let raw = std::fs::read_to_string(dir.join("config.toml")).unwrap();
        assert!(!raw.contains("relay_secret"), "relay_secret should not appear in TOML when None");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relay_secret_some_is_serialized() {
        let dir = std::env::temp_dir().join(format!("arcade_config_test_{}", line!()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = Config {
            relay_secret: Some("test".into()),
            ..Default::default()
        };
        save_config(&dir, &config);
        let loaded = load_config(&dir);
        assert_eq!(loaded.relay_secret, Some("test".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn generate_identity_secret_produces_4_words() {
        let secret = generate_identity_secret();
        let parts: Vec<&str> = secret.split(' ').collect();
        assert_eq!(parts.len(), 4, "expected 4 words, got: {secret}");
    }

    #[test]
    fn generate_identity_secret_words_from_bip39() {
        let wordlist: Vec<&str> = BIP39_ENGLISH.lines().collect();
        let secret = generate_identity_secret();
        for word in secret.split(' ') {
            assert!(wordlist.contains(&word), "word '{word}' not in BIP39 list");
        }
    }

    #[test]
    fn generate_identity_secret_is_nondeterministic() {
        let a = generate_identity_secret();
        let b = generate_identity_secret();
        assert_ne!(a, b, "two calls should produce different secrets");
    }
}
