//! Sprite walk animation prototype.
//!
//! Loads Time Fantasy character frames, displays a character on screen,
//! and plays the correct walk animation based on keyboard or gamepad input.
//! Q/E (or bumpers) cycles through available characters.
//! Movement via arrow keys, WASD, d-pad, or left stick.
//!
//! Run with: `cargo run --example sprite_walk`

use bevy::{camera::ScalingMode, prelude::*};
use std::fs;

const MOVE_SPEED: f32 = 125.0;
const STRIDE: f32 = 12.5;
const FRAME_DURATION: f32 = STRIDE / MOVE_SPEED;
const CANVAS_W: f32 = 320.0;
const CANVAS_H: f32 = 180.0;
const ASSET_ROOT: &str = "external/timefantasy";
const STICK_DEADZONE: f32 = 0.2;

// ---------------------------------------------------------------------------
// XInput FFI — bypasses Bevy's gilrs (see docs/gilrs-dual-gamepad-bug.md)
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

const XINPUT_GAMEPAD_DPAD_UP: u16 = 0x0001;
const XINPUT_GAMEPAD_DPAD_DOWN: u16 = 0x0002;
const XINPUT_GAMEPAD_DPAD_LEFT: u16 = 0x0004;
const XINPUT_GAMEPAD_DPAD_RIGHT: u16 = 0x0008;
const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 0x0100;
const XINPUT_GAMEPAD_RIGHT_SHOULDER: u16 = 0x0200;

const ERROR_SUCCESS: u32 = 0;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

fn load_xinput() -> Option<XInputGetStateFn> {
    use std::ffi::CString;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            module: *mut std::ffi::c_void,
            name: *const u8,
        ) -> *mut std::ffi::c_void;
    }

    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() {
            continue;
        }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

fn normalize_thumb(value: i16) -> f32 {
    if value >= 0 {
        value as f32 / 32767.0
    } else {
        value as f32 / 32768.0
    }
}

// ---------------------------------------------------------------------------
// Gamepad state resource (player 1 only)
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct GamepadState {
    connected: bool,
    left_stick: Vec2,
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    left_bumper_just_pressed: bool,
    right_bumper_just_pressed: bool,
    prev_left_bumper: bool,
    prev_right_bumper: bool,
}

fn read_gamepad_input(
    mut state: ResMut<GamepadState>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            if loaded.is_none() {
                warn!("Failed to load XInput DLL — gamepad input unavailable");
            }
            *xinput_fn = Some(loaded);
            match loaded {
                Some(f) => f,
                None => return,
            }
        }
    };

    let mut xinput_state = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xinput_state.as_mut_ptr()) };

    if result != ERROR_SUCCESS {
        let prev_lb = state.prev_left_bumper;
        let prev_rb = state.prev_right_bumper;
        *state = GamepadState::default();
        state.prev_left_bumper = prev_lb;
        state.prev_right_bumper = prev_rb;
        return;
    }

    let xs = unsafe { xinput_state.assume_init() };
    let gp = &xs.gamepad;
    let btn = |mask: u16| gp.buttons & mask != 0;

    let left_bumper = btn(XINPUT_GAMEPAD_LEFT_SHOULDER);
    let right_bumper = btn(XINPUT_GAMEPAD_RIGHT_SHOULDER);

    state.connected = true;
    state.left_stick = Vec2::new(normalize_thumb(gp.thumb_lx), normalize_thumb(gp.thumb_ly));
    state.dpad_up = btn(XINPUT_GAMEPAD_DPAD_UP);
    state.dpad_down = btn(XINPUT_GAMEPAD_DPAD_DOWN);
    state.dpad_left = btn(XINPUT_GAMEPAD_DPAD_LEFT);
    state.dpad_right = btn(XINPUT_GAMEPAD_DPAD_RIGHT);
    state.left_bumper_just_pressed = left_bumper && !state.prev_left_bumper;
    state.right_bumper_just_pressed = right_bumper && !state.prev_right_bumper;
    state.prev_left_bumper = left_bumper;
    state.prev_right_bumper = right_bumper;
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .init_resource::<GamepadState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (read_gamepad_input, switch_character, player_movement, update_camera_scale, wrap_position, animate_sprite, sync_ghosts, update_window_title).chain(),
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

type FrameSet = [[Handle<Image>; 3]; 4]; // [direction][stand, walk1, walk2]

#[derive(Resource)]
struct CharacterAssets {
    groups: Vec<(String, FrameSet)>,
    current: usize,
}

#[derive(Component)]
struct WalkAnimation {
    frame_index: usize,
    timer: Timer,
    moving: bool,
}

fn direction_row(direction: Direction) -> usize {
    match direction {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::Left => 2,
        Direction::Right => 3,
    }
}

fn load_frame_set(asset_server: &AssetServer, path: &str) -> FrameSet {
    [
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
    ]
}

/// Scans the asset directory for character folders that have the full 12-frame walk set.
fn discover_character_paths() -> Vec<String> {
    let base = std::path::Path::new("assets").join(ASSET_ROOT);
    let required = [
        "down_stand.png", "down_walk1.png", "down_walk2.png",
        "up_stand.png", "up_walk1.png", "up_walk2.png",
        "left_stand.png", "left_walk1.png", "left_walk2.png",
        "right_stand.png", "right_walk1.png", "right_walk2.png",
    ];

    let mut paths = Vec::new();

    let Ok(categories) = fs::read_dir(&base) else {
        return paths;
    };

    for category in categories.flatten() {
        if !category.file_type().map_or(false, |ft| ft.is_dir()) {
            continue;
        }
        let Ok(entries) = fs::read_dir(category.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().map_or(false, |ft| ft.is_dir()) {
                continue;
            }
            let has_all = required.iter().all(|name| entry.path().join(name).exists());
            if has_all {
                // Build the asset path relative to assets/
                let category_name = category.file_name();
                let entry_name = entry.file_name();
                let asset_path = format!(
                    "{ASSET_ROOT}/{}/{}",
                    category_name.to_string_lossy(),
                    entry_name.to_string_lossy()
                );
                paths.push(asset_path);
            }
        }
    }

    paths.sort();
    paths
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0 / 4.0, // initial value for 1280×720
            ..OrthographicProjection::default_2d()
        }),
    ));

    let paths = discover_character_paths();
    assert!(!paths.is_empty(), "No character assets found. Run setup-assets.sh first.");

    let groups: Vec<(String, FrameSet)> = paths
        .into_iter()
        .map(|path| {
            let frames = load_frame_set(&asset_server, &path);
            (path, frames)
        })
        .collect();

    let initial_sprite = groups[0].1[0][0].clone();

    commands.insert_resource(CharacterAssets {
        groups,
        current: 0,
    });

    commands.spawn((
        Player,
        Facing(Direction::Down),
        WalkAnimation {
            frame_index: 0,
            timer: Timer::from_seconds(FRAME_DURATION, TimerMode::Repeating),
            moving: false,
        },
        Sprite::from_image(initial_sprite.clone()),
    ));

    for _ in 0..3 {
        commands.spawn((
            Ghost,
            Sprite::from_image(initial_sprite.clone()),
            Visibility::Hidden,
        ));
    }
}

fn switch_character(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadState>,
    mut assets: ResMut<CharacterAssets>,
    mut query: Query<(&Facing, &mut WalkAnimation, &mut Sprite), With<Player>>,
) {
    let count = assets.groups.len();
    let prev = assets.current;

    if keyboard.just_pressed(KeyCode::KeyQ) || gamepad.left_bumper_just_pressed {
        assets.current = (assets.current + count - 1) % count;
    }
    if keyboard.just_pressed(KeyCode::KeyE) || gamepad.right_bumper_just_pressed {
        assets.current = (assets.current + 1) % count;
    }

    if assets.current == prev {
        return;
    }

    info!("Character: {} ({}/{})", assets.groups[assets.current].0, assets.current + 1, count);

    for (facing, mut anim, mut sprite) in &mut query {
        let row = direction_row(facing.0);
        let frames = &assets.groups[assets.current].1;
        anim.frame_index = 0;
        anim.timer.reset();
        sprite.image = frames[row][0].clone();
    }
}

fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadState>,
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut Facing, &mut WalkAnimation), With<Player>>,
) {
    let mut direction = Vec2::ZERO;
    let mut new_facing: Option<Direction> = None;

    // Keyboard
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

    // Gamepad d-pad
    if gamepad.dpad_up {
        direction.y += 1.0;
        new_facing = Some(Direction::Up);
    }
    if gamepad.dpad_down {
        direction.y -= 1.0;
        new_facing = Some(Direction::Down);
    }
    if gamepad.dpad_left {
        direction.x -= 1.0;
        new_facing = Some(Direction::Left);
    }
    if gamepad.dpad_right {
        direction.x += 1.0;
        new_facing = Some(Direction::Right);
    }

    // Gamepad left stick
    if gamepad.left_stick.length() > STICK_DEADZONE {
        direction += gamepad.left_stick;
        if gamepad.left_stick.x.abs() > gamepad.left_stick.y.abs() {
            new_facing = Some(if gamepad.left_stick.x > 0.0 {
                Direction::Right
            } else {
                Direction::Left
            });
        } else {
            new_facing = Some(if gamepad.left_stick.y > 0.0 {
                Direction::Up
            } else {
                Direction::Down
            });
        }
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
    assets: Res<CharacterAssets>,
    mut query: Query<(&Facing, &mut WalkAnimation, &mut Sprite), With<Player>>,
) {
    let frames = &assets.groups[assets.current].1;

    for (facing, mut anim, mut sprite) in &mut query {
        let row = direction_row(facing.0);

        if !anim.moving {
            anim.frame_index = 0;
            anim.timer.reset();
            sprite.image = frames[row][0].clone();
            continue;
        }

        anim.timer.tick(time.delta());
        if anim.timer.just_finished() {
            anim.frame_index = if anim.frame_index == 1 { 2 } else { 1 };
        }

        sprite.image = frames[row][anim.frame_index].clone();
    }
}

fn update_camera_scale(
    windows: Query<&Window>,
    mut projection_query: Query<&mut Projection, With<Camera2d>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(mut projection) = projection_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    let integer_scale = (window.width() / CANVAS_W)
        .min(window.height() / CANVAS_H)
        .floor()
        .max(1.0);
    let new_scale = 1.0 / integer_scale;

    if (ortho.scale - new_scale).abs() > f32::EPSILON {
        ortho.scale = new_scale;
    }
}

fn wrap_position(
    windows: Query<&Window>,
    projection_query: Query<&Projection, With<Camera2d>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };

    let world_w = window.width() * ortho.scale;
    let world_h = window.height() * ortho.scale;
    let half_w = world_w / 2.0;
    let half_h = world_h / 2.0;

    for mut transform in &mut query {
        let pos = &mut transform.translation;
        if pos.x > half_w {
            pos.x -= world_w;
        } else if pos.x < -half_w {
            pos.x += world_w;
        }
        if pos.y > half_h {
            pos.y -= world_h;
        } else if pos.y < -half_h {
            pos.y += world_h;
        }
    }
}

fn sync_ghosts(
    windows: Query<&Window>,
    projection_query: Query<&Projection, With<Camera2d>>,
    player_query: Query<(&Transform, &Sprite), With<Player>>,
    mut ghost_query: Query<(&mut Transform, &mut Sprite, &mut Visibility), (With<Ghost>, Without<Player>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let Ok((player_tf, player_sprite)) = player_query.single() else {
        return;
    };

    let w = window.width() * ortho.scale;
    let h = window.height() * ortho.scale;
    let px = player_tf.translation.x;
    let py = player_tf.translation.y;

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
        ghost_sprite.image = player_sprite.image.clone();

        let near_x = px.abs() > w / 2.0 - 20.0;
        let near_y = py.abs() > h / 2.0 - 20.0;

        let visible = match i {
            0 => near_x,
            1 => near_y,
            2 => near_x && near_y,
            _ => false,
        };
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn update_window_title(
    mut windows: Query<&mut Window>,
    projection_query: Query<&Projection, With<Camera2d>>,
    player_query: Query<(&Transform, &Sprite), With<Player>>,
    images: Res<Assets<Image>>,
    assets: Res<CharacterAssets>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let Ok((player_tf, player_sprite)) = player_query.single() else {
        return;
    };

    let cam_scale = (1.0 / ortho.scale).round() as i32;
    let win_w = window.width() as i32;
    let win_h = window.height() as i32;
    let world_w = (window.width() * ortho.scale) as i32;
    let world_h = (window.height() * ortho.scale) as i32;
    let px = player_tf.translation.x as i32;
    let py = player_tf.translation.y as i32;

    let sprite_size = images
        .get(&player_sprite.image)
        .map(|img| format!("{}x{}", img.width(), img.height()))
        .unwrap_or_else(|| "?x?".into());

    let char_path = &assets.groups[assets.current].0;

    window.title = format!(
        "pos ({px},{py}) | sprite {sprite_size} | cam {cam_scale}x | window {win_w}x{win_h} | world {world_w}x{world_h} | {char_path}"
    );
}
