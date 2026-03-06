use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands
        .spawn((Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },))
        .with_child((
            Text::new(format!("arcade {}", env!("GIT_COMMIT_HASH"))),
            TextFont::from_font_size(48.0),
            TextColor::WHITE,
        ));
}
