//! Sean's Arcade — chat client.
//!
//! Usage: `cargo run -p arcade [-- --data-dir local/alice]`

mod chat;
mod config;
mod fraktur;
mod net;

use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::window::WindowResolution;

fn log_layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    let data_dir = config::data_dir_from_args();
    std::fs::create_dir_all(&data_dir).ok();
    let log_file = std::fs::File::create(data_dir.join("arcade.log"))
        .expect("failed to create log file");
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(log_file)
        .with_ansi(false);
    Some(Box::new(layer))
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Sean's Arcade {}", &env!("GIT_COMMIT_HASH")[..8]),
                    resolution: WindowResolution::new(600, 500),
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                custom_layer: log_layer,
                ..default()
            })
        )
        .add_plugins(net::NetPlugin)
        .add_plugins(chat::ChatPlugin)
        .run();
}
