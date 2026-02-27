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
        .add_systems(
            Update,
            (player_movement, wrap_position, animate_sprite, sync_ghosts).chain(),
        )
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Ghost;

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
        Sprite::from_image(initial_sprite.clone()),
        Transform::from_scale(Vec3::splat(SPRITE_SCALE)),
    ));

    // Three ghosts handle edge wrapping: horizontal, vertical, and corner
    for _ in 0..3 {
        commands.spawn((
            Ghost,
            Sprite::from_image(initial_sprite.clone()),
            Transform::from_scale(Vec3::splat(SPRITE_SCALE)),
            Visibility::Hidden,
        ));
    }
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

fn wrap_position(
    windows: Query<&Window>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let half_w = window.width() / 2.0;
    let half_h = window.height() / 2.0;

    for mut transform in &mut query {
        let pos = &mut transform.translation;
        if pos.x > half_w {
            pos.x -= window.width();
        } else if pos.x < -half_w {
            pos.x += window.width();
        }
        if pos.y > half_h {
            pos.y -= window.height();
        } else if pos.y < -half_h {
            pos.y += window.height();
        }
    }
}

fn sync_ghosts(
    windows: Query<&Window>,
    player_query: Query<(&Transform, &Sprite), With<Player>>,
    mut ghost_query: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<Ghost>, Without<Player>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((player_tf, player_sprite)) = player_query.single() else {
        return;
    };

    let w = window.width();
    let h = window.height();
    let px = player_tf.translation.x;
    let py = player_tf.translation.y;

    // Pick which side each ghost wraps to based on which half the player is in
    let offset_x = if px > 0.0 { -w } else { w };
    let offset_y = if py > 0.0 { -h } else { h };

    let ghost_offsets = [
        Vec3::new(offset_x, 0.0, 0.0),
        Vec3::new(0.0, offset_y, 0.0),
        Vec3::new(offset_x, offset_y, 0.0),
    ];

    for (i, (mut ghost_tf, mut ghost_sprite, mut visibility)) in ghost_query.iter_mut().enumerate() {
        let Some(&offset) = ghost_offsets.get(i) else {
            break;
        };
        ghost_tf.translation = player_tf.translation + offset;
        ghost_tf.scale = player_tf.scale;
        ghost_sprite.image = player_sprite.image.clone();

        // Only show ghosts when the player is near the relevant edge
        let near_x = px.abs() > w / 2.0 - SPRITE_SCALE * 20.0;
        let near_y = py.abs() > h / 2.0 - SPRITE_SCALE * 20.0;

        let visible = match i {
            0 => near_x,
            1 => near_y,
            2 => near_x && near_y,
            _ => false,
        };
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
}
