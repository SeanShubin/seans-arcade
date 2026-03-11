//! File system browser using bevy_egui's CollapsingHeader as a tree widget.
//!
//! Run with:
//!   cargo run --example tree_browser
//!   cargo run --example tree_browser -- D:/some/path

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use std::path::PathBuf;

#[derive(Resource)]
struct BrowserRoot {
    path: PathBuf,
}

fn main() {
    let root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    App::new()
        .add_plugins((DefaultPlugins, EguiPlugin::default()))
        .insert_resource(BrowserRoot { path: root })
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Camera2d);
        })
        .add_systems(EguiPrimaryContextPass, browser_ui)
        .run();
}

fn browser_ui(mut contexts: EguiContexts, root: Res<BrowserRoot>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("File Browser");
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            show_dir(ui, &root.path, 0);
        });
    });
}

const MAX_DEPTH: usize = 10;

fn show_dir(ui: &mut egui::Ui, path: &std::path::Path, depth: usize) {
    if depth > MAX_DEPTH {
        ui.label("(max depth reached)");
        return;
    }

    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(e) => {
            ui.colored_label(egui::Color32::RED, format!("Error: {e}"));
            return;
        }
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            dirs.push(p);
        } else {
            files.push(p);
        }
    }

    dirs.sort();
    files.sort();

    for dir in &dirs {
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");

        egui::CollapsingHeader::new(format!("📁 {name}"))
            .id_salt(dir)
            .show(ui, |ui| {
                show_dir(ui, dir, depth + 1);
            });
    }

    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?");

        ui.label(format!("  📄 {name}"));
    }
}
