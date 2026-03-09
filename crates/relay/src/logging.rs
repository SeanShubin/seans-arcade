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
    log_dir: PathBuf,
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

        Self {
            file,
            log_dir: log_dir.to_path_buf(),
        }
    }

    /// Return all log files in the log directory as (filename, contents) pairs.
    pub fn all_log_files(&self) -> Vec<(String, String)> {
        let Ok(entries) = std::fs::read_dir(&self.log_dir) else {
            return Vec::new();
        };
        let mut files: Vec<(String, String)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "log")
            })
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let contents = std::fs::read_to_string(e.path()).ok()?;
                Some((name, contents))
            })
            .collect();
        files.sort_by(|a, b| a.0.cmp(&b.0));
        files
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
