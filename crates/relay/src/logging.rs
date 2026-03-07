//! Append-only message log writer.
//!
//! One line per message: `timestamp sender_name payload_summary`

use std::fs::{File, OpenOptions, create_dir_all};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use chrono::Local;

pub struct LogWriter {
    file: File,
}

impl LogWriter {
    pub fn new(log_dir: &Path) -> Self {
        create_dir_all(log_dir).expect("failed to create log directory");

        let now = Local::now();
        let filename = now.format("chat_%Y-%m-%d_%H-%M-%S.log").to_string();
        let path = log_dir.join(filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("failed to open log file");

        println!("relay: logging to {}", path.display());

        Self { file }
    }

    pub fn log_message(&mut self, from: &str, payload_summary: &str) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        let timestamp = now.as_secs();
        let _ = writeln!(self.file, "{timestamp} {from} {payload_summary}");
        let _ = self.file.flush();
    }

    pub fn log_dir_from_data_dir(data_dir: &Path) -> PathBuf {
        data_dir.join("logs")
    }
}
