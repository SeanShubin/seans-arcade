//! Operator CLI for Sean's Arcade.
//!
//! Subcommands:
//!   logs              — list log files
//!   logs <filename>   — print log file contents
//!   logs --latest     — print most recent log file

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Collect non-flag arguments (skip --data-dir and its value)
    let mut positional = Vec::new();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--data-dir" {
            i += 2; // skip flag and value
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }

    if positional.is_empty() {
        print_usage();
        return;
    }

    match positional[0].as_str() {
        "logs" => handle_logs(&positional[1..]),
        "version" => println!("arcade-ops {}", env!("GIT_COMMIT_HASH")),
        _ => print_usage(),
    }
}

fn print_usage() {
    eprintln!("arcade-ops {}", env!("GIT_COMMIT_HASH"));
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  arcade-ops [--data-dir DIR] logs              List log files");
    eprintln!("  arcade-ops [--data-dir DIR] logs <filename>   Print log file contents");
    eprintln!("  arcade-ops [--data-dir DIR] logs --latest     Print most recent log file");
    eprintln!("  arcade-ops version                            Print version");
}

fn handle_logs(args: &[String]) {
    let log_dir = log_dir_from_args();

    if args.is_empty() {
        // List log files
        list_logs(&log_dir);
        return;
    }

    if args[0] == "--latest" {
        print_latest_log(&log_dir);
        return;
    }

    // Print specific log file
    let path = log_dir.join(&args[0]);
    match std::fs::read_to_string(&path) {
        Ok(contents) => print_log_contents(&contents),
        Err(e) => eprintln!("Error reading {}: {e}", path.display()),
    }
}

fn print_log_contents(contents: &str) {
    use chrono::{Local, TimeZone};

    for line in contents.lines() {
        // Format: "1772863484 Alice Hello, bob!"
        // Split into: timestamp, rest
        let Some((timestamp_str, rest)) = line.split_once(' ') else {
            println!("{line}");
            continue;
        };
        let Ok(secs) = timestamp_str.parse::<i64>() else {
            println!("{line}");
            continue;
        };
        let dt = Local.timestamp_opt(secs, 0).single();
        match dt {
            Some(dt) => println!("{} {rest}", dt.format("%Y-%m-%d %H:%M:%S")),
            None => println!("{line}"),
        }
    }
}

fn data_dir_from_args() -> std::path::PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() - 1 {
        if args[i] == "--data-dir" {
            return std::path::PathBuf::from(&args[i + 1]);
        }
    }
    std::path::PathBuf::from(".")
}

fn log_dir_from_args() -> std::path::PathBuf {
    data_dir_from_args().join("logs")
}

fn list_logs(log_dir: &std::path::Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading {}: {e}", log_dir.display());
            return;
        }
    };

    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "log")
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    files.sort();

    if files.is_empty() {
        println!("No log files found in {}", log_dir.display());
    } else {
        for f in &files {
            println!("{f}");
        }
    }
}

fn print_latest_log(log_dir: &std::path::Path) {
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Error reading {}: {e}", log_dir.display());
            return;
        }
    };

    let latest = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "log")
        })
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()));

    match latest {
        Some(entry) => {
            let path = entry.path();
            println!("--- {} ---", path.display());
            match std::fs::read_to_string(&path) {
                Ok(contents) => print_log_contents(&contents),
                Err(e) => eprintln!("Error reading {}: {e}", path.display()),
            }
        }
        None => println!("No log files found in {}", log_dir.display()),
    }
}
