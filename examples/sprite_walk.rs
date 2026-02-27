//! Sprite walk animation prototype.
//!
//! Loads Time Fantasy character frames, displays a character on screen,
//! and plays the correct walk animation based on keyboard input direction.
//!
//! Run with: `cargo run --example sprite_walk`

use bevy::prelude::*;

const MOVE_SPEED: f32 = 400.0;
const FRAME_DURATION: f32 = 0.15;
const SPRITE_SCALE: f32 = 4.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, setup)
        .add_systems(Update, (player_movement, animate_sprite).chain())
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Down,
    Up,
    Left,
    Right,
}

#[derive(Component)]
struct Facing(Direction);

#[derive(Component)]
struct WalkAnimation {
    frames: [[Handle<Image>; 3]; 4], // [direction][stand, walk1, walk2]
    frame_index: usize,
    timer: Timer,
    moving: bool,
}

impl WalkAnimation {
    fn direction_row(direction: Direction) -> usize {
        match direction {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::Left => 2,
            Direction::Right => 3,
        }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    let path = "external/timefantasy/chara/chara2_1";
    let frames = [
        [
            asset_server.load(format!("{path}/down_stand.png")),
            asset_server.load(format!("{path}/down_walk1.png")),
            asset_server.load(format!("{path}/down_walk2.png")),
        ],
        [
            asset_server.load(format!("{path}/up_stand.png")),
            asset_server.load(format!("{path}/up_walk1.png")),
            asset_server.load(format!("{path}/up_walk2.png")),
        ],
        [
            asset_server.load(format!("{path}/left_stand.png")),
            asset_server.load(format!("{path}/left_walk1.png")),
            asset_server.load(format!("{path}/left_walk2.png")),
        ],
        [
            asset_server.load(format!("{path}/right_stand.png")),
            asset_server.load(format!("{path}/right_walk1.png")),
            asset_server.load(format!("{path}/right_walk2.png")),
        ],
    ];

    let initial_sprite = frames[0][0].clone();

    commands.spawn((
        Player,
        Facing(Direction::Down),
        WalkAnimation {
            frames,
            frame_index: 0,
            timer: Timer::from_seconds(FRAME_DURATION, TimerMode::Repeating),
            moving: false,
        },
        Sprite::from_image(initial_sprite),
        Transform::from_scale(Vec3::splat(SPRITE_SCALE)),
    ));
}

fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Facing, &mut WalkAnimation), With<Player>>,
) {
    let mut direction = Vec2::ZERO;
    let mut new_facing: Option<Direction> = None;

    if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
        new_facing = Some(Direction::Up);
    }
    if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
        new_facing = Some(Direction::Down);
    }
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
        new_facing = Some(Direction::Left);
    }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
        new_facing = Some(Direction::Right);
    }

    for (mut transform, mut facing, mut anim) in &mut query {
        let is_moving = direction != Vec2::ZERO;
        anim.moving = is_moving;

        if is_moving {
            let movement = direction.normalize() * MOVE_SPEED * time.delta_secs();
            transform.translation += movement.extend(0.0);
        }

        if let Some(dir) = new_facing {
            facing.0 = dir;
        }
    }
}

fn animate_sprite(
    time: Res<Time>,
    mut query: Query<(&Facing, &mut WalkAnimation, &mut Sprite), With<Player>>,
) {
    for (facing, mut anim, mut sprite) in &mut query {
        let row = WalkAnimation::direction_row(facing.0);

        if !anim.moving {
            anim.frame_index = 0;
            anim.timer.reset();
            sprite.image = anim.frames[row][0].clone();
            continue;
        }

        anim.timer.tick(time.delta());
        if anim.timer.just_finished() {
            // Alternate between walk1 (index 1) and walk2 (index 2)
            anim.frame_index = if anim.frame_index == 1 { 2 } else { 1 };
        }

        sprite.image = anim.frames[row][anim.frame_index].clone();
    }
}
