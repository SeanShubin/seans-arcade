use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::Path;

fn inspect_zip<R: Read + std::io::Seek>(
    reader: R,
    prefix: &str,
    all_files: &mut Vec<String>,
    by_name: &mut HashMap<String, Vec<String>>,
) {
    let mut archive = match zip::ZipArchive::new(reader) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Warning: failed to open {}: {}", prefix, e);
            return;
        }
    };

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Warning: failed to read entry in {}: {}", prefix, e);
                continue;
            }
        };

        let name = entry.name().to_string();

        // Skip directories
        if name.ends_with('/') {
            continue;
        }

        let full_path = format!("{}/{}", prefix, name);

        // If this entry is itself a zip, recurse into it
        if name.to_lowercase().ends_with(".zip") {
            let mut buf = Vec::new();
            if let Err(e) = entry.read_to_end(&mut buf) {
                eprintln!("Warning: failed to read nested zip {}: {}", full_path, e);
                continue;
            }
            inspect_zip(Cursor::new(buf), &full_path, all_files, by_name);
        } else {
            all_files.push(full_path.clone());

            let file_name = Path::new(&name)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or(name);
            by_name.entry(file_name).or_default().push(full_path);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: zip_inspect <directory>");
        std::process::exit(1);
    }

    let dir = &args[1];
    let pattern = format!("{}/*.zip", dir);

    let mut all_files = Vec::new();
    let mut by_name: HashMap<String, Vec<String>> = HashMap::new();

    for entry in glob::glob(&pattern).expect("Invalid glob pattern") {
        match entry {
            Ok(path) => {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Warning: failed to open {}: {}", path.display(), e);
                        continue;
                    }
                };
                let prefix = path.file_name().unwrap().to_string_lossy().to_string();
                inspect_zip(std::io::BufReader::new(file), &prefix, &mut all_files, &mut by_name);
            }
            Err(e) => eprintln!("Warning: glob error: {}", e),
        }
    }

    println!("=== All files ===");
    for f in &all_files {
        println!("{}", f);
    }

    // Collect duplicates (names appearing more than once)
    let mut duplicates: Vec<_> = by_name
        .iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();
    duplicates.sort_by_key(|(name, _)| (*name).clone());

    if !duplicates.is_empty() {
        println!();
        println!("=== Duplicates ===");
        for (name, paths) in &duplicates {
            println!("{}", name);
            for p in *paths {
                println!("  {}", p);
            }
        }
    }
}
