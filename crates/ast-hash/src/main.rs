use sha2::{Digest, Sha256};
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: ast-hash <crate-path>");
        process::exit(1);
    }

    let crate_path = PathBuf::from(&args[1]);
    let src_dir = crate_path.join("src");

    if !src_dir.is_dir() {
        eprintln!("Error: {}/src is not a directory", crate_path.display());
        process::exit(1);
    }

    let mut paths = collect_rs_files(&src_dir);
    paths.sort();

    let mut hasher = Sha256::new();

    for path in &paths {
        let source = fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {e}", path.display());
            process::exit(1);
        });

        let file = syn::parse_file(&source).unwrap_or_else(|e| {
            eprintln!("Error parsing {}: {e}", path.display());
            process::exit(1);
        });

        let ast_text = format!("{file:#?}");

        let relative = path.strip_prefix(&crate_path).unwrap_or(path);
        hasher.update(relative.to_string_lossy().as_bytes());
        hasher.update(ast_text.as_bytes());
    }

    let hash = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in hash {
        write!(hex, "{byte:02x}").unwrap();
    }

    println!("{hex}");
}

fn collect_rs_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(dir).unwrap_or_else(|e| {
        eprintln!("Error reading directory {}: {e}", dir.display());
        process::exit(1);
    });
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| {
            eprintln!("Error reading directory entry: {e}");
            process::exit(1);
        });
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_rs_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}
