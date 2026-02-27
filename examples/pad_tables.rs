//! Pads all markdown tables in `.md` files so columns align in monospace editors.
//!
//! Run with: `cargo run --example pad_tables`
//!
//! Recursively scans the current directory for `.md` files, finds markdown tables,
//! and pads each cell so pipes align. Skips `.git`, `target`, `node_modules`, and
//! any dot-prefixed directory. Only writes files that actually change.

use std::fs;
use std::path::Path;

fn main() {
    let mut changed_files = Vec::new();
    visit_dir(Path::new("."), &mut changed_files);

    if changed_files.is_empty() {
        println!("All tables are already padded.");
    } else {
        for path in &changed_files {
            println!("  Padded: {}", path);
        }
        println!("\nChanged {} file(s).", changed_files.len());
    }
}

fn visit_dir(dir: &Path, changed: &mut Vec<String>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            visit_dir(&path, changed);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if process_file(&path) {
                changed.push(path.display().to_string());
            }
        }
    }
}

fn process_file(path: &Path) -> bool {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let lines: Vec<&str> = content.split('\n').collect();
    let mut result: Vec<String> = Vec::with_capacity(lines.len());
    let mut changed = false;
    let mut i = 0;

    while i < lines.len() {
        if parse_row(lines[i]).is_some() {
            // Collect consecutive table rows
            let mut table_lines = Vec::new();
            while i < lines.len() && parse_row(lines[i]).is_some() {
                table_lines.push(lines[i]);
                i += 1;
            }

            // Only pad if second row is a separator
            if table_lines.len() >= 2 {
                let second = parse_row(table_lines[1]).unwrap();
                if is_separator(&second) {
                    let padded = pad_table(&table_lines);
                    for (j, orig) in table_lines.iter().enumerate() {
                        if padded[j] != *orig {
                            changed = true;
                        }
                    }
                    result.extend(padded);
                } else {
                    result.extend(table_lines.iter().map(|s| s.to_string()));
                }
            } else {
                result.extend(table_lines.iter().map(|s| s.to_string()));
            }
        } else {
            result.push(lines[i].to_string());
            i += 1;
        }
    }

    if changed {
        let new_content = result.join("\n");
        fs::write(path, new_content).expect("Failed to write file");
        true
    } else {
        false
    }
}

/// Parse a markdown table row into trimmed cell contents.
/// Returns `None` if the line isn't a table row.
fn parse_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') || trimmed.len() < 2 {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    let cells: Vec<String> = inner.split('|').map(|c| c.trim().to_string()).collect();
    Some(cells)
}

/// Check if all cells in a row are separator patterns like `---`, `:---`, `---:`, `:---:`.
fn is_separator(cells: &[String]) -> bool {
    if cells.is_empty() {
        return false;
    }
    cells.iter().all(|c| {
        let mut chars = c.chars();
        // Must have at least one character
        let first = match chars.next() {
            Some(ch) => ch,
            None => return false,
        };
        // Strip optional leading colon
        let rest_start = if first == ':' {
            match chars.next() {
                Some(ch) => ch,
                None => return false, // just ":"
            }
        } else {
            first
        };
        // Must have at least one dash
        if rest_start != '-' {
            return false;
        }
        // Remaining chars: dashes, then optional trailing colon
        let mut saw_colon = false;
        for ch in chars {
            if saw_colon {
                return false; // something after the trailing colon
            }
            if ch == '-' {
                continue;
            } else if ch == ':' {
                saw_colon = true;
            } else {
                return false;
            }
        }
        true
    })
}

/// Visual width of a string — count chars, not bytes.
/// All characters in this project's docs are single-width in Western monospace fonts.
fn visual_width(s: &str) -> usize {
    s.chars().count()
}

/// Format a separator cell preserving alignment markers (`:---`, `---:`, `:---:`).
fn format_separator_cell(original: &str, width: usize) -> String {
    let left = original.starts_with(':');
    let right = original.ends_with(':');
    let colon_width = if left { 1 } else { 0 } + if right { 1 } else { 0 };
    let dash_count = if width > colon_width {
        width - colon_width
    } else {
        1
    };
    let mut s = String::with_capacity(width);
    if left {
        s.push(':');
    }
    for _ in 0..dash_count {
        s.push('-');
    }
    if right {
        s.push(':');
    }
    s
}

/// Pad a table so all columns align.
fn pad_table(lines: &[&str]) -> Vec<String> {
    // Parse all rows
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in lines {
        match parse_row(line) {
            Some(cells) => rows.push(cells),
            None => return lines.iter().map(|s| s.to_string()).collect(),
        }
    }
    if rows.len() < 2 {
        return lines.iter().map(|s| s.to_string()).collect();
    }

    // Normalize column count
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    for row in &mut rows {
        while row.len() < max_cols {
            row.push(String::new());
        }
    }

    // Calculate max width per column (skip separator row)
    let mut col_widths = vec![0usize; max_cols];
    for (i, row) in rows.iter().enumerate() {
        if i == 1 && is_separator(row) {
            continue;
        }
        for (j, cell) in row.iter().enumerate() {
            col_widths[j] = col_widths[j].max(visual_width(cell));
        }
    }

    // Minimum width of 3 so separators are at least `---`
    for w in &mut col_widths {
        if *w < 3 {
            *w = 3;
        }
    }

    // Format each row
    let mut result = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let mut line = String::from("|");
        if i == 1 && is_separator(row) {
            for (j, cell) in row.iter().enumerate() {
                line.push(' ');
                line.push_str(&format_separator_cell(cell, col_widths[j]));
                line.push(' ');
                line.push('|');
            }
        } else {
            for (j, cell) in row.iter().enumerate() {
                line.push(' ');
                line.push_str(cell);
                let padding = col_widths[j] - visual_width(cell);
                for _ in 0..padding {
                    line.push(' ');
                }
                line.push(' ');
                line.push('|');
            }
        }
        result.push(line);
    }
    result
}
