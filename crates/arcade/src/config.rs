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
