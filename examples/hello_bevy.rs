use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, hello)
        .run();
}

fn hello() {
    info!("Hello, Bevy!");
}
