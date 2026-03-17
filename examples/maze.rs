//! Maze with keys and gates.
//!
//! 3×3 cell grid, 1 px walls, 4 px corridors.
//! Collect color-coded keys to open matching gates. Reach the goal to win.
//!
//! Controls:
//!   Arrow keys / WASD / D-pad / left stick: move
//!   Mouse wheel: zoom (even multiples)
//!   R: restart
//!
//! Run with: `cargo run --example maze`

use bevy::asset::RenderAssetUsages;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

// ---------------------------------------------------------------------------
// Maze layout
// ---------------------------------------------------------------------------

const WALL_PX: usize = 1;
const CORR_PX: usize = 4;
const CELLS: usize = 3;
const MAZE_W: usize = CELLS * (CORR_PX + WALL_PX) + WALL_PX; // 16
const MAZE_H: usize = MAZE_W;
const HUD_H: usize = 4;
const IMG_W: usize = MAZE_W;
const IMG_H: usize = MAZE_H + HUD_H; // 20
const MOVE_SPEED: f32 = 40.0;
const STICK_DEAD: f32 = 0.3;

// Abstract 7×7 grid (each cell maps to either 1 px wall or 4 px corridor).
//
//   xxxxxxx        W  W  W  W  W  W  W
//   xexbxfx        W  K1 W  GL W  K2 W
//   x xdx x   =>   W  O  W  G2 W  O  W
//   x xax x        W  O  W  ST W  O  W
//   x x xcx        W  O  W  O  W  G1 W
//   x     x        W  O  O  O  O  O  W   (opened wall at col 4 for solvability)
//   xxxxxxx        W  W  W  W  W  W  W
//
// a=start  b=goal  c=gate(key1)  d=gate(key2)  e=key1  f=key2
const W: u8 = 0;
const O: u8 = 1;
const ST: u8 = 2;
const GL: u8 = 3;
const K1: u8 = 4;
const K2: u8 = 5;
const G1: u8 = 6; // gate requiring key 1
const G2: u8 = 7; // gate requiring key 2

const GRID: [[u8; 7]; 7] = [
    [W,  W,  W,  W,  W,  W,  W ],
    [W,  K1, W,  GL, W,  K2, W ],
    [W,  O,  W,  G2, W,  O,  W ],
    [W,  O,  W,  ST, W,  O,  W ],
    [W,  O,  W,  O,  W,  G1, W ],
    [W,  O,  O,  O,  O,  O,  W ],
    [W,  W,  W,  W,  W,  W,  W ],
];

// RGBA colors
const COL_BG: [u8; 4] = [16, 16, 16, 255];
const COL_WALL: [u8; 4] = [160, 160, 160, 255];
const COL_PLAYER: [u8; 4] = [0, 220, 0, 255];
const COL_GOAL: [u8; 4] = [255, 255, 0, 255];
const COL_KEY1: [u8; 4] = [255, 60, 60, 255];
const COL_KEY2: [u8; 4] = [60, 120, 255, 255];
const COL_DIM: [u8; 4] = [50, 50, 50, 255];
const COL_WIN: [u8; 4] = [255, 255, 255, 255];

// ---------------------------------------------------------------------------
// XInput FFI
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    _thumb_rx: i16,
    _thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    _packet_number: u32,
    gamepad: XInputGamepad,
}

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const DPAD_LEFT: u16 = 0x0004;
const DPAD_RIGHT: u16 = 0x0008;

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
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct PadState {
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    left_stick: Vec2,
}

#[derive(Resource)]
struct MazeImage(Handle<Image>);

#[derive(Resource)]
struct GameState {
    px: i32,
    py: i32,
    keys: [bool; 2],
    won: bool,
    move_timer: f32,
}

#[derive(Resource)]
struct Zoom(u32);

// ---------------------------------------------------------------------------
// Pixel helpers
// ---------------------------------------------------------------------------

/// Map abstract grid coordinate (0–6) to pixel range.
fn grid_to_px(g: usize) -> (usize, usize) {
    if g % 2 == 0 {
        let p = g / 2 * (CORR_PX + WALL_PX);
        (p, p + WALL_PX)
    } else {
        let p = g / 2 * (CORR_PX + WALL_PX) + WALL_PX;
        (p, p + CORR_PX)
    }
}

/// Build a pixel-level collision map. Gates are solid only when locked.
fn build_collision(keys: &[bool; 2]) -> [[bool; MAZE_W]; MAZE_H] {
    let mut map = [[false; MAZE_W]; MAZE_H];
    for gy in 0..7 {
        for gx in 0..7 {
            let solid = match GRID[gy][gx] {
                W => true,
                G1 => !keys[0],
                G2 => !keys[1],
                _ => false,
            };
            if solid {
                let (x0, x1) = grid_to_px(gx);
                let (y0, y1) = grid_to_px(gy);
                for y in y0..y1 { for x in x0..x1 { map[y][x] = true; } }
            }
        }
    }
    map
}

/// Can the 4×4 avatar be placed at pixel (px, py) without hitting walls?
fn can_move(px: i32, py: i32, col: &[[bool; MAZE_W]; MAZE_H]) -> bool {
    for dy in 0..CORR_PX as i32 {
        for dx in 0..CORR_PX as i32 {
            let x = px + dx;
            let y = py + dy;
            if x < 0 || x >= MAZE_W as i32 || y < 0 || y >= MAZE_H as i32 { return false; }
            if col[y as usize][x as usize] { return false; }
        }
    }
    true
}

/// Find the grid cell the avatar's center sits in, return (col, row) 0-indexed.
fn player_cell(px: i32, py: i32) -> Option<(usize, usize)> {
    let cx = px + CORR_PX as i32 / 2;
    let cy = py + CORR_PX as i32 / 2;
    for col in 0..CELLS {
        for row in 0..CELLS {
            let (x0, x1) = grid_to_px(1 + col * 2);
            let (y0, y1) = grid_to_px(1 + row * 2);
            if cx >= x0 as i32 && cx < x1 as i32 && cy >= y0 as i32 && cy < y1 as i32 {
                return Some((col, row));
            }
        }
    }
    None
}

fn start_pos() -> (i32, i32) {
    for gy in 0..7 {
        for gx in 0..7 {
            if GRID[gy][gx] == ST {
                let (x, _) = grid_to_px(gx);
                let (y, _) = grid_to_px(gy);
                return (x as i32, y as i32);
            }
        }
    }
    (1, 1)
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, windows: Query<&Window>) {
    let (sx, sy) = start_pos();
    commands.insert_resource(GameState { px: sx, py: sy, keys: [false; 2], won: false, move_timer: 0.0 });

    let Ok(win) = windows.single() else { return };
    let max_x = (win.width() / IMG_W as f32).floor() as u32;
    let max_y = (win.height() / IMG_H as f32).floor() as u32;
    let zoom = (max_x.min(max_y) / 2 * 2).max(2);
    commands.insert_resource(Zoom(zoom));

    let pixels = vec![0u8; IMG_W * IMG_H * 4];
    let image = Image::new(
        Extent3d { width: IMG_W as u32, height: IMG_H as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.insert_resource(MazeImage(handle.clone()));
    commands.spawn(Sprite { image: handle, ..default() });
    commands.spawn(Camera2d);
}

fn read_gamepad(
    mut state: ResMut<PadState>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            *xinput_fn = Some(loaded);
            match loaded { Some(f) => f, None => return }
        }
    };
    let mut xs = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(0, xs.as_mut_ptr()) };
    if result != 0 { *state = PadState::default(); return; }
    let xs = unsafe { xs.assume_init() };
    let gp = &xs.gamepad;
    let btn = |mask: u16| gp.buttons & mask != 0;
    state.dpad_up = btn(DPAD_UP);
    state.dpad_down = btn(DPAD_DOWN);
    state.dpad_left = btn(DPAD_LEFT);
    state.dpad_right = btn(DPAD_RIGHT);
    state.left_stick = Vec2::new(normalize_thumb(gp.thumb_lx), normalize_thumb(gp.thumb_ly));
}

fn movement(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    pad: Res<PadState>,
    mut gs: ResMut<GameState>,
) {
    if gs.won { return; }

    let mut dx: i32 = 0;
    let mut dy: i32 = 0;
    if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) { dx += 1; }
    if keyboard.pressed(KeyCode::ArrowLeft)  || keyboard.pressed(KeyCode::KeyA) { dx -= 1; }
    if keyboard.pressed(KeyCode::ArrowDown)  || keyboard.pressed(KeyCode::KeyS) { dy += 1; }
    if keyboard.pressed(KeyCode::ArrowUp)    || keyboard.pressed(KeyCode::KeyW) { dy -= 1; }
    if pad.dpad_right { dx += 1; }
    if pad.dpad_left  { dx -= 1; }
    if pad.dpad_down  { dy += 1; }
    if pad.dpad_up    { dy -= 1; }
    if pad.left_stick.x >  STICK_DEAD { dx += 1; }
    if pad.left_stick.x < -STICK_DEAD { dx -= 1; }
    if pad.left_stick.y >  STICK_DEAD { dy -= 1; } // stick up = pixel y decreases
    if pad.left_stick.y < -STICK_DEAD { dy += 1; }
    dx = dx.clamp(-1, 1);
    dy = dy.clamp(-1, 1);

    if dx == 0 && dy == 0 { gs.move_timer = 0.0; return; }

    let col = build_collision(&gs.keys);
    gs.move_timer += time.delta_secs();
    let interval = 1.0 / MOVE_SPEED;
    while gs.move_timer >= interval {
        gs.move_timer -= interval;
        if dx != 0 && can_move(gs.px + dx, gs.py, &col) {
            gs.px += dx;
        } else if dy != 0 && can_move(gs.px, gs.py + dy, &col) {
            gs.py += dy;
        }
    }

    // Pickups
    if let Some((c, r)) = player_cell(gs.px, gs.py) {
        match GRID[1 + r * 2][1 + c * 2] {
            K1 => gs.keys[0] = true,
            K2 => gs.keys[1] = true,
            GL => gs.won = true,
            _ => {}
        }
    }
}

fn restart(keyboard: Res<ButtonInput<KeyCode>>, mut gs: ResMut<GameState>) {
    if keyboard.just_pressed(KeyCode::KeyR) {
        let (sx, sy) = start_pos();
        *gs = GameState { px: sx, py: sy, keys: [false; 2], won: false, move_timer: 0.0 };
    }
}

fn zoom_system(
    mut scroll: MessageReader<MouseWheel>,
    mut zoom: ResMut<Zoom>,
    windows: Query<&Window>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
) {
    let mut delta = 0i32;
    for ev in scroll.read() {
        delta += if ev.y > 0.0 { 1 } else if ev.y < 0.0 { -1 } else { 0 };
    }
    if delta != 0 {
        let Ok(win) = windows.single() else { return };
        let max_x = (win.width() / IMG_W as f32).floor() as u32;
        let max_y = (win.height() / IMG_H as f32).floor() as u32;
        let max = (max_x.min(max_y) / 2 * 2).max(2);
        zoom.0 = (zoom.0 as i32 + delta * 2).clamp(2, max as i32) as u32;
    }
    if let Ok(mut proj) = proj_q.single_mut() {
        if let Projection::Orthographic(ref mut ortho) = *proj {
            let new_scale = 1.0 / zoom.0 as f32;
            if (ortho.scale - new_scale).abs() > f32::EPSILON { ortho.scale = new_scale; }
        }
    }
}

fn render(gs: Res<GameState>, maze_img: Res<MazeImage>, mut images: ResMut<Assets<Image>>) {
    let Some(image) = images.get_mut(&maze_img.0) else { return };
    let data = image.data.as_mut().unwrap();

    let set = |data: &mut Vec<u8>, x: usize, y: usize, c: [u8; 4]| {
        let i = (y * IMG_W + x) * 4;
        data[i..i + 4].copy_from_slice(&c);
    };

    // Clear
    for y in 0..IMG_H { for x in 0..IMG_W { set(data, x, y, COL_BG); } }

    // Walls
    let collision = build_collision(&gs.keys);
    for y in 0..MAZE_H { for x in 0..MAZE_W { if collision[y][x] { set(data, x, y, COL_WALL); } } }

    // Gates (draw colored even when locked — collision already handles them)
    for gy in 0..7 { for gx in 0..7 {
        let col = match GRID[gy][gx] {
            G1 if !gs.keys[0] => COL_KEY1,
            G2 if !gs.keys[1] => COL_KEY2,
            _ => continue,
        };
        let (x0, x1) = grid_to_px(gx);
        let (y0, y1) = grid_to_px(gy);
        for y in y0..y1 { for x in x0..x1 { set(data, x, y, col); } }
    }}

    // Goal (2×2 centered in cell)
    for gy in (1..7).step_by(2) { for gx in (1..7).step_by(2) {
        if GRID[gy][gx] == GL {
            let (x0, _) = grid_to_px(gx);
            let (y0, _) = grid_to_px(gy);
            for dy in 0..2 { for dx in 0..2 { set(data, x0 + 1 + dx, y0 + 1 + dy, COL_GOAL); } }
        }
    }}

    // Keys (if not collected)
    for gy in (1..7).step_by(2) { for gx in (1..7).step_by(2) {
        let (ki, col) = match GRID[gy][gx] {
            K1 => (0, COL_KEY1),
            K2 => (1, COL_KEY2),
            _ => continue,
        };
        if gs.keys[ki] { continue; }
        let (x0, _) = grid_to_px(gx);
        let (y0, _) = grid_to_px(gy);
        for dy in 0..2 { for dx in 0..2 { set(data, x0 + 1 + dx, y0 + 1 + dy, col); } }
    }}

    // Player (4×4)
    let pc = if gs.won { COL_WIN } else { COL_PLAYER };
    for dy in 0..CORR_PX { for dx in 0..CORR_PX {
        let x = gs.px as usize + dx;
        let y = gs.py as usize + dy;
        if x < MAZE_W && y < MAZE_H { set(data, x, y, pc); }
    }}

    // HUD: key indicators (2×2 each, centered below maze)
    let hud_y = MAZE_H + 2;
    for (i, &kc) in [COL_KEY1, COL_KEY2].iter().enumerate() {
        let kx = IMG_W / 2 - 3 + i * 4;
        let c = if gs.keys[i] { kc } else { COL_DIM };
        for dy in 0..2 { for dx in 0..2 { set(data, kx + dx, hud_y + dy, c); } }
    }
}

fn update_title(mut windows: Query<&mut Window>, gs: Res<GameState>, zoom: Res<Zoom>) {
    let Ok(mut win) = windows.single_mut() else { return };
    let status = if gs.won { "YOU WIN! (R to restart)" } else { "maze" };
    let k = format!("keys: [{}{}]",
        if gs.keys[0] { "1" } else { "." },
        if gs.keys[1] { "2" } else { "." },
    );
    win.title = format!("{status} | {k} | zoom {}", zoom.0);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .init_resource::<PadState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (
            read_gamepad,
            restart,
            movement,
            zoom_system,
            render,
            update_title,
        ).chain())
        .run();
}
