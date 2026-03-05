use bevy::prelude::*;

const COLOR_VIVID_BLUE: Color = Color::srgb_u8(0x15, 0x2e, 0xff);
const COLOR_RADIOACTIVE_GREEN: Color = Color::srgb_u8(0x2c, 0xfa, 0x1f);
const COLOR_RED_WINE: Color = Color::srgba_u8(140, 0, 52, 255);

const PADDLE_WIDTH: f32 = 15.0;
const PADDLE_HEIGHT: f32 = 80.0;
const PADDLE_COLOR: Color = COLOR_RADIOACTIVE_GREEN;
const BALL_COLOR: Color = COLOR_VIVID_BLUE;
const ARENA_WIDTH: f32 = 800.0;
const ARENA_HEIGHT: f32 = 500.0;
const BORDER_THICKNESS: f32 = 4.0;
const BORDER_COLOR: Color = COLOR_RED_WINE;
const PADDLE_X_OFFSET: f32 = 30.0;
const BALL_SIZE: f32 = 12.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    let paddle_offset = ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET;
    let paddle_x = paddle_bundle(-paddle_offset);
    let paddle_y = paddle_bundle(paddle_offset);
    let ball = ball_bundle();
    let top_border = border_bundle(ARENA_HEIGHT / 2.0);
    let bottom_border = border_bundle(-ARENA_HEIGHT / 2.0);
    commands.spawn(bottom_border);
    commands.spawn(top_border);
    commands.spawn(paddle_x);
    commands.spawn(paddle_y);
    commands.spawn(ball);
}

fn border_bundle(position_y: f32) -> (Sprite, Transform) {
    let sprite = Sprite {
            color: BORDER_COLOR,
            custom_size: Some(Vec2::new(ARENA_WIDTH, BORDER_THICKNESS)),
            ..default()
        };
    let position = Vec3::new(0.0, position_y, 0.0);
    let transform = Transform::from_translation(position);
    (sprite, transform)
}

fn paddle_bundle(x: f32) -> (Sprite, Transform) {
    let sprite = Sprite {
        custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
        color: PADDLE_COLOR,
        ..default()
    };
    let transform = Transform::from_xyz(x, 0.0, 0.0);
    (sprite, transform)
}

fn ball_bundle() -> (Sprite, Transform) {
    let sprite = Sprite {
        custom_size: Some(Vec2::splat(BALL_SIZE)),
        color: BALL_COLOR,
        ..default()
    };
    let transform = Transform::from_xyz(0.0, 0.0, 0.0);
    (sprite, transform)
}
