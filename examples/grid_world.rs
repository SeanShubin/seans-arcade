//! Grid world prototype — combines character animation with screen transitions.
//!
//! Map layout loaded from a RON file (default: maps/9keys.ron, or pass path as arg).
//! Each cell's tile variant (corner, edge, center, etc.) comes from LDtk's autotile
//! rules. Swapping tilesets keeps the autotile pattern but changes the art.
//!
//! View shows 5×5 cells at a time. Inner 3×3 is the dead zone (no scrolling).
//! Edge cells trigger proportional camera scrolling.
//!
//! Controls:
//!   Movement: WASD / arrows / d-pad / left stick
//!   Character: Q / E (or bumpers)
//!   Floor tile: Z / X
//!   Wall tile: C / V
//!
//! Run with: `cargo run --example grid_world [-- path/to/map.ron]`

use bevy::{camera::ScalingMode, prelude::*};
use serde::Deserialize;

const CELL_SIZE: f32 = 64.0;
const VIEW_CELLS: f32 = 5.0;
const VIEW_PX: f32 = VIEW_CELLS * CELL_SIZE; // 320
const VIEW_HALF: f32 = VIEW_PX / 2.0;        // 160
const BUFFER: f32 = CELL_SIZE;               // 64 — one edge cell
const MOVE_SPEED: f32 = 250.0;
const FRAME_DURATION: f32 = 0.1;
const STICK_DEADZONE: f32 = 0.2;

const DEFAULT_MAP: &str = "maps/9keys.ron";
const WALK_DIR: &str = "assets/external/64-bit/walk";

// ---------------------------------------------------------------------------
// RON map format (matches ldtk_convert output)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GridMap {
    cols: usize,
    rows: usize,
    start_col: usize,
    start_row: usize,
    cells: Vec<CellEntry>,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
enum CellKind {
    Floor,
    Wall,
}

#[derive(Deserialize, Clone, Copy)]
struct CellEntry {
    kind: CellKind,
    src_x: u32,
    src_y: u32,
}

impl CellEntry {
    fn src_rect(self) -> Rect {
        let x = self.src_x as f32;
        let y = self.src_y as f32;
        // Inset by half a texel to prevent sampling outside the tile boundary
        let inset = 0.5;
        Rect::new(x + inset, y + inset, x + CELL_SIZE - inset, y + CELL_SIZE - inset)
    }
}

fn map_path() -> String {
    std::env::args().nth(1).unwrap_or_else(|| DEFAULT_MAP.to_string())
}

fn load_map(path: &str) -> GridMap {
    let ron_str = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read {path}: {e}"));
    ron::from_str(&ron_str)
        .unwrap_or_else(|e| panic!("Failed to parse {path}: {e}"))
}

// ---------------------------------------------------------------------------
// XInput FFI
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
        fn GetProcAddress(module: *mut std::ffi::c_void, name: *const u8) -> *mut std::ffi::c_void;
    }
    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() { continue; }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

fn normalize_thumb(value: i16) -> f32 {
    if value >= 0 { value as f32 / 32767.0 } else { value as f32 / 32768.0 }
}

// ---------------------------------------------------------------------------
// Gamepad state
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct GamepadState {
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
            if loaded.is_none() { warn!("Failed to load XInput DLL"); }
            *xinput_fn = Some(loaded);
            match loaded { Some(f) => f, None => return }
        }
    };
    let mut xs = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xs.as_mut_ptr()) };
    if result != ERROR_SUCCESS {
        let (plb, prb) = (state.prev_left_bumper, state.prev_right_bumper);
        *state = GamepadState::default();
        state.prev_left_bumper = plb;
        state.prev_right_bumper = prb;
        return;
    }
    let xs = unsafe { xs.assume_init() };
    let gp = &xs.gamepad;
    let btn = |mask: u16| gp.buttons & mask != 0;
    let lb = btn(XINPUT_GAMEPAD_LEFT_SHOULDER);
    let rb = btn(XINPUT_GAMEPAD_RIGHT_SHOULDER);
    state.left_stick = Vec2::new(normalize_thumb(gp.thumb_lx), normalize_thumb(gp.thumb_ly));
    state.dpad_up = btn(XINPUT_GAMEPAD_DPAD_UP);
    state.dpad_down = btn(XINPUT_GAMEPAD_DPAD_DOWN);
    state.dpad_left = btn(XINPUT_GAMEPAD_DPAD_LEFT);
    state.dpad_right = btn(XINPUT_GAMEPAD_DPAD_RIGHT);
    state.left_bumper_just_pressed = lb && !state.prev_left_bumper;
    state.right_bumper_just_pressed = rb && !state.prev_right_bumper;
    state.prev_left_bumper = lb;
    state.prev_right_bumper = rb;
}

// ---------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Player;

#[derive(Component)]
struct MapTile {
    col: usize,
    row: usize,
    kind: CellKind,
    src_rect: Rect,
}

#[derive(Component)]
struct Border;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction { Down, Up, Left, Right }

#[derive(Component)]
struct Facing(Direction);

#[derive(Component)]
struct WalkAnimation {
    frame_index: usize,
    timer: Timer,
    moving: bool,
}

/// Remembers the last direction the player actually moved, for corner assist.
#[derive(Component, Default)]
struct LastMoveDir(Vec2);

#[derive(Clone)]
struct FrameRef {
    image: Handle<Image>,
    rect: Rect,
}

#[derive(Clone)]
struct CharacterFrames {
    walk: [[FrameRef; 4]; 4],
    idle: [[FrameRef; 4]; 4],
}

#[derive(Resource)]
struct CharacterAssets {
    groups: Vec<(String, CharacterFrames)>,
    current: usize,
}

#[derive(Resource)]
struct TileAssets {
    floor_sheets: Vec<(String, Handle<Image>)>,
    wall_sheets: Vec<(String, Handle<Image>)>,
    floor_idx: usize,
    wall_idx: usize,
}

#[derive(Resource)]
struct MapConfig {
    cols: usize,
    rows: usize,
    map_w: f32,
    map_h: f32,
    cells: Vec<CellEntry>,
}


// ---------------------------------------------------------------------------
// Direction helpers
// ---------------------------------------------------------------------------

fn direction_row(d: Direction) -> usize {
    match d { Direction::Down => 0, Direction::Up => 1, Direction::Left => 2, Direction::Right => 3 }
}

fn sheet_dir_row(d: Direction) -> usize {
    match d { Direction::Right => 0, Direction::Up => 1, Direction::Down => 2, Direction::Left => 3 }
}

fn build_frames(image: &Handle<Image>) -> [[FrameRef; 4]; 4] {
    [Direction::Down, Direction::Up, Direction::Left, Direction::Right].map(|dir| {
        let row = sheet_dir_row(dir) as f32;
        [0, 1, 2, 3].map(|col| {
            let x = col as f32 * CELL_SIZE;
            let y = row * CELL_SIZE;
            FrameRef { image: image.clone(), rect: Rect::new(x, y, x + CELL_SIZE, y + CELL_SIZE) }
        })
    })
}

// ---------------------------------------------------------------------------
// Asset discovery
// ---------------------------------------------------------------------------

fn discover_characters(asset_server: &AssetServer) -> Vec<(String, CharacterFrames)> {
    let mut entries: Vec<String> = std::fs::read_dir(WALK_DIR)
        .unwrap_or_else(|e| panic!("Failed to read {WALK_DIR}: {e}"))
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().into_owned();
            name.ends_with(".png").then_some(name)
        })
        .collect();
    entries.sort();
    entries.into_iter().map(|f| {
        let walk: Handle<Image> = asset_server.load(format!("external/64-bit/walk/{f}"));
        let idle: Handle<Image> = asset_server.load(format!("external/64-bit/idle/{f}"));
        let name = f.trim_end_matches(".png").to_string();
        (name, CharacterFrames { walk: build_frames(&walk), idle: build_frames(&idle) })
    }).collect()
}

fn discover_sheets(dir: &str, asset_prefix: &str, asset_server: &AssetServer) -> Vec<(String, Handle<Image>)> {
    let mut entries: Vec<(String, Handle<Image>)> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("Failed to read {dir}: {e}"))
        .filter_map(|e| {
            let name = e.ok()?.file_name().to_string_lossy().into_owned();
            if name.ends_with(".png") {
                let handle = asset_server.load(format!("{asset_prefix}{name}"));
                Some((name.trim_end_matches(".png").to_string(), handle))
            } else { None }
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

// ---------------------------------------------------------------------------
// Wrapping helpers
// ---------------------------------------------------------------------------

fn wrap_offset(delta: f32, period: f32) -> f32 {
    (delta + period / 2.0).rem_euclid(period) - period / 2.0
}

/// Per-screen camera: snaps to screen center, slides through buffer at edges.
/// Each buffer provides half the transition; the neighboring screen's buffer
/// provides the other half, giving a smooth full-screen slide.
fn screen_camera(pos: f32) -> f32 {
    let screen = (pos / VIEW_PX).floor();
    let center = screen * VIEW_PX + VIEW_HALF;
    let local = pos - screen * VIEW_PX;

    let inner_hi = VIEW_PX - BUFFER;

    if local > inner_hi {
        let depth = (local - inner_hi) / BUFFER;
        center + depth * VIEW_HALF
    } else if local < BUFFER {
        let depth = (BUFFER - local) / BUFFER;
        center - depth * VIEW_HALF
    } else {
        center
    }
}

// ---------------------------------------------------------------------------
// Collision
// ---------------------------------------------------------------------------

/// Returns true if the point (x, y) is inside a wall cell.
/// Uses a small inset so the player can walk flush against walls.
const COLLISION_INSET: f32 = 4.0;

fn is_wall_at(x: f32, y: f32, map: &MapConfig) -> bool {
    // Check all four corners of the player's collision box
    let half = CELL_SIZE / 2.0 - COLLISION_INSET;
    for &(dx, dy) in &[(-half, -half), (-half, half), (half, -half), (half, half)] {
        let cx = (x + dx).rem_euclid(map.map_w);
        let cy = (y + dy).rem_euclid(map.map_h);
        let col = (cx / CELL_SIZE) as usize;
        let row = (cy / CELL_SIZE) as usize;
        let col = col.min(map.cols - 1);
        let row = row.min(map.rows - 1);
        if map.cells[row * map.cols + col].kind == CellKind::Wall {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .init_resource::<GamepadState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            read_gamepad_input,
            player_movement,
            update_camera_scale,
            update_camera,
            wrap_tiles,
            switch_character,
            switch_tiles,
            update_tile_sprites,
            animate_sprite,
            sync_borders,
            update_window_title,
        ).chain())
        .run();
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            scale: 1.0 / 2.0,
            ..OrthographicProjection::default_2d()
        }),
    ));

    // Load map from RON
    let path = map_path();
    let map = load_map(&path);
    let map_w = map.cols as f32 * CELL_SIZE;
    let map_h = map.rows as f32 * CELL_SIZE;

    // Characters
    let char_groups = discover_characters(&asset_server);
    assert!(!char_groups.is_empty(), "No character sheets found in {WALK_DIR}");
    let initial = &char_groups[0].1.idle[direction_row(Direction::Down)][0];
    let char_image = initial.image.clone();
    let char_rect = initial.rect;
    commands.insert_resource(CharacterAssets { groups: char_groups, current: 0 });

    // Floor & wall sheets
    let floor_sheets = discover_sheets(
        "assets/external/64-bit/floor", "external/64-bit/floor/", &asset_server,
    );
    let wall_sheets = discover_sheets(
        "assets/external/64-bit/wall", "external/64-bit/wall/", &asset_server,
    );
    assert!(!floor_sheets.is_empty(), "No floor sheets found");
    assert!(!wall_sheets.is_empty(), "No wall sheets found");

    // Spawn map tiles with per-cell autotile rects from RON
    for row in 0..map.rows {
        for col in 0..map.cols {
            let cell = map.cells[row * map.cols + col];
            let image = match cell.kind {
                CellKind::Wall => wall_sheets[0].1.clone(),
                CellKind::Floor => floor_sheets[0].1.clone(),
            };
            let src_rect = cell.src_rect();
            commands.spawn((
                MapTile { col, row, kind: cell.kind, src_rect },
                Sprite {
                    image,
                    rect: Some(src_rect),
                    custom_size: Some(Vec2::splat(CELL_SIZE)),
                    ..default()
                },
                Transform::from_xyz(
                    col as f32 * CELL_SIZE + CELL_SIZE / 2.0,
                    row as f32 * CELL_SIZE + CELL_SIZE / 2.0,
                    -1.0,
                ),
            ));
        }
    }

    commands.insert_resource(TileAssets {
        floor_sheets, wall_sheets,
        floor_idx: 0, wall_idx: 0,
    });
    commands.insert_resource(MapConfig {
        cols: map.cols, rows: map.rows,
        map_w, map_h,
        cells: map.cells,
    });

    // Player
    let start_x = map.start_col as f32 * CELL_SIZE + CELL_SIZE / 2.0;
    let start_y = map.start_row as f32 * CELL_SIZE + CELL_SIZE / 2.0;
    commands.spawn((
        Player,
        Facing(Direction::Down),
        WalkAnimation { frame_index: 0, timer: Timer::from_seconds(FRAME_DURATION, TimerMode::Repeating), moving: false },
        LastMoveDir::default(),
        Sprite { image: char_image, rect: Some(char_rect), ..default() },
        Transform::from_xyz(start_x, start_y, 0.0),
    ));

    // Black border panels (mask area outside 5×5 viewport)
    for _ in 0..4 {
        commands.spawn((
            Border,
            Sprite { color: Color::BLACK, custom_size: Some(Vec2::ZERO), ..default() },
            Transform::from_xyz(0.0, 0.0, 10.0),
        ));
    }

}

// ---------------------------------------------------------------------------
// Movement
// ---------------------------------------------------------------------------

fn player_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadState>,
    time: Res<Time>,
    map: Res<MapConfig>,
    mut query: Query<(&mut Transform, &mut Facing, &mut WalkAnimation, &mut LastMoveDir), With<Player>>,
) {
    let mut dir = Vec2::ZERO;
    let mut new_facing: Option<Direction> = None;

    if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) { dir.y += 1.0; new_facing = Some(Direction::Up); }
    if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) { dir.y -= 1.0; new_facing = Some(Direction::Down); }
    if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) { dir.x -= 1.0; new_facing = Some(Direction::Left); }
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) { dir.x += 1.0; new_facing = Some(Direction::Right); }

    if gamepad.dpad_up { dir.y += 1.0; new_facing = Some(Direction::Up); }
    if gamepad.dpad_down { dir.y -= 1.0; new_facing = Some(Direction::Down); }
    if gamepad.dpad_left { dir.x -= 1.0; new_facing = Some(Direction::Left); }
    if gamepad.dpad_right { dir.x += 1.0; new_facing = Some(Direction::Right); }

    if gamepad.left_stick.length() > STICK_DEADZONE {
        dir += gamepad.left_stick;
        if gamepad.left_stick.x.abs() > gamepad.left_stick.y.abs() {
            new_facing = Some(if gamepad.left_stick.x > 0.0 { Direction::Right } else { Direction::Left });
        } else {
            new_facing = Some(if gamepad.left_stick.y > 0.0 { Direction::Up } else { Direction::Down });
        }
    }

    for (mut tf, mut facing, mut anim, mut last_dir) in &mut query {
        anim.moving = dir != Vec2::ZERO;
        if anim.moving {
            let speed = MOVE_SPEED * time.delta_secs();
            let input = dir.normalize();

            // Probe each axis for wall collisions
            let probe_x = (tf.translation.x + input.x * speed).rem_euclid(map.map_w);
            let x_blocked = input.x != 0.0 && is_wall_at(probe_x, tf.translation.y, &map);
            let probe_y = (tf.translation.y + input.y * speed).rem_euclid(map.map_h);
            let y_blocked = input.y != 0.0 && is_wall_at(tf.translation.x, probe_y, &map);

            // Corner assist: if one axis is blocked and the other has no input,
            // continue moving on the unblocked axis using the remembered direction.
            // When an axis is blocked, redirect full speed to the unblocked axis.
            let mut eff_x = if x_blocked { 0.0 } else { input.x.signum() };
            let mut eff_y = if y_blocked { 0.0 } else { input.y.signum() };

            if x_blocked && eff_y == 0.0 {
                eff_y = last_dir.0.y.signum();
            }
            if y_blocked && eff_x == 0.0 {
                eff_x = last_dir.0.x.signum();
            }

            // Apply movement per-axis at full speed
            let mut moved = Vec2::ZERO;

            if eff_x != 0.0 {
                let new_x = (tf.translation.x + eff_x * speed).rem_euclid(map.map_w);
                if !is_wall_at(new_x, tf.translation.y, &map) {
                    tf.translation.x = new_x;
                    moved.x = eff_x;
                }
            }

            if eff_y != 0.0 {
                let new_y = (tf.translation.y + eff_y * speed).rem_euclid(map.map_h);
                if !is_wall_at(tf.translation.x, new_y, &map) {
                    tf.translation.y = new_y;
                    moved.y = eff_y;
                }
            }

            if moved != Vec2::ZERO {
                last_dir.0 = moved;
            }
        }
        if let Some(d) = new_facing { facing.0 = d; }
    }
}

// ---------------------------------------------------------------------------
// Camera
// ---------------------------------------------------------------------------

fn update_camera_scale(
    windows: Query<&Window>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
) {
    let Ok(win) = windows.single() else { return };
    let Ok(mut proj) = proj_q.single_mut() else { return };
    let Projection::Orthographic(ref mut ortho) = *proj else { return };
    let integer_scale = (win.width() / VIEW_PX).min(win.height() / VIEW_PX).floor().max(1.0);
    let new_scale = 1.0 / integer_scale;
    if (ortho.scale - new_scale).abs() > f32::EPSILON { ortho.scale = new_scale; }
}

fn update_camera(
    player_q: Query<&Transform, With<Player>>,
    proj_q: Query<&Projection, With<Camera2d>>,
    mut cam_q: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let Ok(ptf) = player_q.single() else { return };
    let Ok(mut cam_tf) = cam_q.single_mut() else { return };

    // Snap to the zoom's pixel grid to prevent sub-pixel cracks between tiles.
    // At Nx zoom, one screen pixel = 1/N world units, so round to that grid.
    let zoom = if let Ok(Projection::Orthographic(ortho)) = proj_q.single() {
        (1.0 / ortho.scale).round().max(1.0)
    } else { 1.0 };
    let snap = |v: f32| (v * zoom).round() / zoom;

    cam_tf.translation.x = snap(screen_camera(ptf.translation.x));
    cam_tf.translation.y = snap(screen_camera(ptf.translation.y));
}

fn wrap_tiles(
    cam_q: Query<&Transform, With<Camera2d>>,
    map: Res<MapConfig>,
    mut tiles: Query<(&MapTile, &mut Transform), Without<Camera2d>>,
) {
    let Ok(cam_tf) = cam_q.single() else { return };
    let cx = cam_tf.translation.x;
    let cy = cam_tf.translation.y;
    for (tile, mut tf) in &mut tiles {
        let bx = tile.col as f32 * CELL_SIZE + CELL_SIZE / 2.0;
        let by = tile.row as f32 * CELL_SIZE + CELL_SIZE / 2.0;
        tf.translation.x = cx + wrap_offset(bx - cx, map.map_w);
        tf.translation.y = cy + wrap_offset(by - cy, map.map_h);
    }
}

// ---------------------------------------------------------------------------
// Switching
// ---------------------------------------------------------------------------

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
    if assets.current == prev { return; }
    for (facing, mut anim, mut sprite) in &mut query {
        let f = &assets.groups[assets.current].1.idle[direction_row(facing.0)][0];
        anim.frame_index = 0;
        anim.timer.reset();
        sprite.image = f.image.clone();
        sprite.rect = Some(f.rect);
    }
}

fn switch_tiles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut assets: ResMut<TileAssets>,
) {
    let fc = assets.floor_sheets.len();
    let wc = assets.wall_sheets.len();
    if keyboard.just_pressed(KeyCode::KeyZ) { assets.floor_idx = (assets.floor_idx + fc - 1) % fc; }
    if keyboard.just_pressed(KeyCode::KeyX) { assets.floor_idx = (assets.floor_idx + 1) % fc; }
    if keyboard.just_pressed(KeyCode::KeyC) { assets.wall_idx = (assets.wall_idx + wc - 1) % wc; }
    if keyboard.just_pressed(KeyCode::KeyV) { assets.wall_idx = (assets.wall_idx + 1) % wc; }
}

fn update_tile_sprites(
    assets: Res<TileAssets>,
    mut tiles: Query<(&MapTile, &mut Sprite)>,
) {
    if !assets.is_changed() { return; }
    for (tile, mut sprite) in &mut tiles {
        sprite.image = match tile.kind {
            CellKind::Wall => assets.wall_sheets[assets.wall_idx].1.clone(),
            CellKind::Floor => assets.floor_sheets[assets.floor_idx].1.clone(),
        };
        sprite.rect = Some(tile.src_rect);
    }
}

// ---------------------------------------------------------------------------
// Animation
// ---------------------------------------------------------------------------

fn animate_sprite(
    time: Res<Time>,
    assets: Res<CharacterAssets>,
    mut query: Query<(&Facing, &mut WalkAnimation, &mut Sprite), With<Player>>,
) {
    let cf = &assets.groups[assets.current].1;
    for (facing, mut anim, mut sprite) in &mut query {
        let row = direction_row(facing.0);
        anim.timer.tick(time.delta());
        if anim.timer.just_finished() { anim.frame_index = (anim.frame_index + 1) % 4; }
        let f = if anim.moving { &cf.walk[row][anim.frame_index] } else { &cf.idle[row][anim.frame_index] };
        sprite.image = f.image.clone();
        sprite.rect = Some(f.rect);
    }
}

// ---------------------------------------------------------------------------
// Borders
// ---------------------------------------------------------------------------

fn sync_borders(
    windows: Query<&Window>,
    proj_q: Query<&Projection, With<Camera2d>>,
    cam_q: Query<&Transform, With<Camera2d>>,
    mut borders: Query<(&mut Transform, &mut Sprite), (With<Border>, Without<Camera2d>)>,
) {
    let Ok(win) = windows.single() else { return };
    let Ok(proj) = proj_q.single() else { return };
    let Projection::Orthographic(ref ortho) = *proj else { return };
    let Ok(cam_tf) = cam_q.single() else { return };

    let s = ortho.scale;
    let ww = win.width() * s;
    let wh = win.height() * s;
    let half = VIEW_PX / 2.0;
    let cx = cam_tf.translation.x;
    let cy = cam_tf.translation.y;
    let bw = (ww - VIEW_PX) / 2.0;
    let bh = (wh - VIEW_PX) / 2.0;

    let panels = [
        (cx - half - bw / 2.0, cy, bw.max(0.0), wh),
        (cx + half + bw / 2.0, cy, bw.max(0.0), wh),
        (cx, cy + half + bh / 2.0, ww, bh.max(0.0)),
        (cx, cy - half - bh / 2.0, ww, bh.max(0.0)),
    ];
    for (i, (mut tf, mut sprite)) in borders.iter_mut().enumerate() {
        let (ox, oy, w, h) = panels[i];
        tf.translation = Vec3::new(ox, oy, 10.0);
        sprite.custom_size = Some(Vec2::new(w, h));
    }
}

// ---------------------------------------------------------------------------
// Window title
// ---------------------------------------------------------------------------

fn update_window_title(
    mut windows: Query<&mut Window>,
    player_q: Query<&Transform, With<Player>>,
    proj_q: Query<&Projection, With<Camera2d>>,
    chars: Res<CharacterAssets>,
    tiles: Res<TileAssets>,
    map: Res<MapConfig>,
) {
    let Ok(mut win) = windows.single_mut() else { return };
    let Ok(ptf) = player_q.single() else { return };

    let zoom = if let Ok(Projection::Orthographic(ortho)) = proj_q.single() {
        (1.0 / ortho.scale).round() as i32
    } else { 1 };

    let px = ptf.translation.x as i32;
    let py = ptf.translation.y as i32;
    let cell_col = ((ptf.translation.x.rem_euclid(map.map_w) / CELL_SIZE) as usize).min(map.cols - 1);
    let cell_row = ((ptf.translation.y.rem_euclid(map.map_h) / CELL_SIZE) as usize).min(map.rows - 1);
    let cell = &map.cells[cell_row * map.cols + cell_col];
    let cell_type = match cell.kind { CellKind::Wall => "wall", CellKind::Floor => "floor" };

    let char_name = &chars.groups[chars.current].0;
    let floor_name = &tiles.floor_sheets[tiles.floor_idx].0;
    let wall_name = &tiles.wall_sheets[tiles.wall_idx].0;

    win.title = format!(
        "{zoom}x | ({px},{py}) [{cell_col},{cell_row}] {cell_type} | {char_name} | F: {floor_name} | W: {wall_name}"
    );
}
