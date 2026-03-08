//! Unzips every `.zip` file in a source directory into a destination directory.
//! Each zip file is extracted into a subdirectory named after the zip (without extension).
//!
//! Usage: `cargo run --example unzip_all -- <source_dir> <dest_dir>`

use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <source_dir> <dest_dir>", args[0]);
        std::process::exit(1);
    }

    let source_dir = PathBuf::from(&args[1]);
    let dest_dir = PathBuf::from(&args[2]);

    for entry in fs::read_dir(&source_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("zip") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("zip file has no stem");

        let out_dir = dest_dir.join(stem);
        fs::create_dir_all(&out_dir)?;

        let file = fs::File::open(&path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let entry_path = out_dir.join(entry.name());

            if entry.is_dir() {
                fs::create_dir_all(&entry_path)?;
            } else {
                if let Some(parent) = entry_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out_file = fs::File::create(&entry_path)?;
                io::copy(&mut entry, &mut out_file)?;
            }
        }

        println!("{} -> {}", path.display(), out_dir.display());
    }

    Ok(())
}
