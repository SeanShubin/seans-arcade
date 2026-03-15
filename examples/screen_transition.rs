//! Screen-transition prototype with hybrid scrolling.
//!
//! 20x10 wrapping arena of background textures.
//! - Inner area (90% of screen): avatar moves freely, camera stays still
//! - Buffer area (5% each side): camera tracks proportionally to buffer depth
//!
//! Controls: WASD or arrow keys to move
//!
//! Run with: `cargo run --example screen_transition`

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

const ARENA_COLS: usize = 20;
const ARENA_ROWS: usize = 10;
const TILE_PX: f32 = 1024.0;
const ARENA_PX_W: f32 = ARENA_COLS as f32 * TILE_PX;
const ARENA_PX_H: f32 = ARENA_ROWS as f32 * TILE_PX;
const VIEWPORT_PX: f32 = 1024.0;
const ORB_RADIUS: f32 = 14.0;
const GLOW_RADIUS: f32 = 22.0;
const AVATAR_SPEED: f32 = 400.0;
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
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Screen Transition Prototype".into(),
                    resolution: bevy::window::WindowResolution::new(1800, 1100),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<CameraPos>()
        .init_resource::<ScrollConfig>()
        .init_resource::<GamepadState>()
        .init_resource::<MovementInput>()
        .init_resource::<GhostConfig>()
        .insert_resource(CameraHome {
            pos: Vec2::new(ARENA_PX_W / 2.0 + TILE_PX / 2.0, ARENA_PX_H / 2.0 + TILE_PX / 2.0),
            prev_scale: 1,
        })
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, hud_system)
        .add_systems(Update, (read_gamepad_input, gather_input, apply_bumper_scale, manage_ghosts, animate_orb, move_avatar, update_camera, wrap_tiles, sync_borders, update_window_title).chain())
        .run();
}

#[derive(Component)]
struct Avatar;

#[derive(Component)]
struct Glow;

#[derive(Component)]
struct Border;

#[derive(Component)]
struct Tile {
    grid_x: usize,
    grid_y: usize,
    copy_x: i32,
    copy_y: i32,
}

/// Maps (col, row) grid coordinates to the asset path and image handle.
#[derive(Resource)]
struct TileMap {
    paths: Vec<Vec<String>>,          // paths[row][col]
    handles: Vec<Vec<Handle<Image>>>, // handles[row][col]
}

/// Tracks how many extra torus copies are currently spawned per axis direction.
#[derive(Resource, Default)]
struct GhostConfig {
    copies_x: i32,
    copies_y: i32,
}

/// Abstract movement direction (unit vector or zero). Written by `gather_input`,
/// read by anything that needs to know which way the player wants to go.
#[derive(Resource, Default)]
struct MovementInput(Vec2);

/// Logical camera position, separate from the Transform so we can wrap cleanly.
#[derive(Resource, Default)]
struct CameraPos(Vec2);

#[derive(Resource)]
struct ScrollConfig {
    buffer_frac: f32,
    scale: u32,
    speed_mult: f32,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self { buffer_frac: 0.25, scale: 1, speed_mult: 2.0 }
    }
}

/// The camera's snap position — always at a tile centre.  Shifts by one tile
/// when the avatar pushes through the buffer at the edge of the visible area.
#[derive(Resource)]
struct CameraHome {
    pos: Vec2,
    prev_scale: u32,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>, mut images: ResMut<Assets<Image>>) {
    // Collect all .bmp files under assets/external/texture/
    let texture_base = std::path::PathBuf::from("assets/external/texture");
    let mut texture_paths: Vec<String> = Vec::new();

    if let Ok(dirs) = std::fs::read_dir(&texture_base) {
        for dir_entry in dirs.flatten() {
            if !dir_entry.path().is_dir() {
                continue;
            }
            if let Ok(files) = std::fs::read_dir(dir_entry.path()) {
                for file in files.flatten() {
                    let path = file.path();
                    if path.extension().is_some_and(|e| e == "bmp") {
                        if let Ok(rel) = path.strip_prefix("assets") {
                            texture_paths.push(rel.to_string_lossy().replace('\\', "/"));
                        }
                    }
                }
            }
        }
    }

    texture_paths.sort();

    let needed = ARENA_COLS * ARENA_ROWS;
    assert!(
        texture_paths.len() >= needed,
        "Need {needed} textures but only found {}",
        texture_paths.len()
    );

    // Camera — full window, no viewport restriction
    commands.spawn(Camera2d);

    // Black border panels that mask everything outside the 1024x1024 game area.
    // Sized dynamically in sync_borders.
    for _ in 0..4 {
        commands.spawn((
            Border,
            Sprite {
                color: Color::BLACK,
                custom_size: Some(Vec2::ZERO),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, 10.0),
        ));
    }

    // Tiles — laid out left to right, top to bottom (top row = highest Y)
    let mut tile_paths = vec![vec![String::new(); ARENA_COLS]; ARENA_ROWS];
    let mut tile_handles: Vec<Vec<Handle<Image>>> = vec![vec![]; ARENA_ROWS];
    for row in 0..ARENA_ROWS {
        for col in 0..ARENA_COLS {
            // Top-to-bottom order: first texture goes to the top row (highest row index)
            let visual_row = ARENA_ROWS - 1 - row;
            let idx = visual_row * ARENA_COLS + col;
            let handle: Handle<Image> = asset_server.load(&texture_paths[idx]);
            tile_paths[row][col] = texture_paths[idx].clone();
            tile_handles[row].push(handle.clone());

            commands.spawn((
                Tile { grid_x: col, grid_y: row, copy_x: 0, copy_y: 0 },
                Sprite {
                    image: handle,
                    ..default()
                },
                Transform::from_xyz(
                    col as f32 * TILE_PX + TILE_PX / 2.0,
                    row as f32 * TILE_PX + TILE_PX / 2.0,
                    0.0,
                ),
            ));
        }
    }
    commands.insert_resource(TileMap { paths: tile_paths, handles: tile_handles });

    // --- Chromatic orb avatar ---
    let orb_tex = make_circle_texture(&mut images, (ORB_RADIUS * 2.0) as u32, false);
    let glow_tex = make_circle_texture(&mut images, (GLOW_RADIUS * 2.0) as u32, true);

    let start = Vec2::new(ARENA_PX_W / 2.0 + TILE_PX / 2.0, ARENA_PX_H / 2.0 + TILE_PX / 2.0);

    // Outer glow ring (pulsing, slightly larger)
    commands.spawn((
        Glow,
        Sprite {
            image: glow_tex,
            color: Color::srgba(1.0, 1.0, 1.0, 0.5),
            custom_size: Some(Vec2::splat(GLOW_RADIUS * 2.0)),
            ..default()
        },
        Transform::from_xyz(start.x, start.y, 1.0),
    ));

    // Core orb
    commands.spawn((
        Avatar,
        Sprite {
            image: orb_tex,
            color: Color::WHITE,
            custom_size: Some(Vec2::splat(ORB_RADIUS * 2.0)),
            ..default()
        },
        Transform::from_xyz(start.x, start.y, 2.0),
    ));

    // Camera starts centred on avatar
    commands.insert_resource(CameraPos(Vec2::new(ARENA_PX_W / 2.0 + TILE_PX / 2.0, ARENA_PX_H / 2.0 + TILE_PX / 2.0)));
}

/// Position four black panels around the 1024×1024 play area to mask overflow.
/// At scale S the camera shows S×1024 world units in 1024 screen pixels, so
/// all sizes must be converted to world units (screen pixels × scale).
fn sync_borders(
    config: Res<ScrollConfig>,
    window_q: Query<&Window>,
    cam_pos: Res<CameraPos>,
    mut borders: Query<(&mut Transform, &mut Sprite), With<Border>>,
) {
    let Ok(win) = window_q.single() else { return };
    let scale = config.scale as f32;

    // Everything in world units (screen pixels × scale).
    let win_w = win.width() * scale;
    let win_h = win.height() * scale;
    let half_vp = VIEWPORT_PX * scale / 2.0;
    let cx = cam_pos.0.x;
    let cy = cam_pos.0.y;

    let bar_w = (win_w - VIEWPORT_PX * scale) / 2.0;
    let bar_h = (win_h - VIEWPORT_PX * scale) / 2.0;

    let panels = [
        (-(half_vp + bar_w / 2.0), 0.0, bar_w.max(0.0), win_h),
        (half_vp + bar_w / 2.0, 0.0, bar_w.max(0.0), win_h),
        (0.0, half_vp + bar_h / 2.0, win_w, bar_h.max(0.0)),
        (0.0, -(half_vp + bar_h / 2.0), win_w, bar_h.max(0.0)),
    ];

    for (i, (mut tf, mut sprite)) in borders.iter_mut().enumerate() {
        let (ox, oy, w, h) = panels[i];
        tf.translation.x = cx + ox;
        tf.translation.y = cy + oy;
        tf.translation.z = 10.0;
        sprite.custom_size = Some(Vec2::new(w, h));
    }
}

fn update_window_title(
    mut windows: Query<&mut Window>,
    avatar_q: Query<&Transform, With<Avatar>>,
    tile_map: Res<TileMap>,
) {
    let Ok(mut window) = windows.single_mut() else { return };
    let Ok(avatar_tf) = avatar_q.single() else { return };

    let ax = avatar_tf.translation.x.rem_euclid(ARENA_PX_W);
    let ay = avatar_tf.translation.y.rem_euclid(ARENA_PX_H);
    let col = ((ax / TILE_PX).floor() as usize).min(ARENA_COLS - 1);
    let row = ((ay / TILE_PX).floor() as usize).min(ARENA_ROWS - 1);
    let path = &tile_map.paths[row][col];

    let w = window.width() as u32;
    let h = window.height() as u32;

    window.title = format!("{w}x{h} | {path}");
}

/// Generate a white circle texture. If `soft_edge` is true the alpha fades
/// out toward the rim (for the glow ring).
fn make_circle_texture(images: &mut Assets<Image>, size: u32, soft_edge: bool) -> Handle<Image> {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let center = (size - 1) as f32 / 2.0;
    let radius = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = if dist > radius {
                0.0
            } else if soft_edge {
                // Hollow ring: transparent in the centre, opaque near the edge, fading out.
                let t = dist / radius; // 0 at centre, 1 at edge
                let ring = (t - 0.6).max(0.0) / 0.4; // ramp from 0.6..1.0
                let fade = 1.0 - ((t - 0.85) / 0.15).max(0.0); // fade out near rim
                ring * fade
            } else {
                1.0
            };
            let i = ((y * size + x) * 4) as usize;
            pixels[i] = 255;
            pixels[i + 1] = 255;
            pixels[i + 2] = 255;
            pixels[i + 3] = (alpha * 255.0) as u8;
        }
    }
    images.add(Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    ))
}

/// Cycle the orb's hue and pulse the glow ring.
fn animate_orb(
    time: Res<Time>,
    mut orb_q: Query<&mut Sprite, (With<Avatar>, Without<Glow>)>,
    mut glow_q: Query<(&mut Sprite, &mut Transform), (With<Glow>, Without<Avatar>)>,
) {
    let t = time.elapsed_secs();
    let hue = (t * 60.0) % 360.0; // full cycle every 6 seconds

    // Core orb — cycling hue, high lightness
    if let Ok(mut sprite) = orb_q.single_mut() {
        sprite.color = Color::oklch(0.8, 0.3, hue);
    }

    // Glow — complementary hue, pulsing size and alpha
    if let Ok((mut sprite, mut tf)) = glow_q.single_mut() {
        let pulse = 1.0 + 0.15 * (t * 4.0).sin(); // gentle size pulse
        let alpha = 0.35 + 0.2 * (t * 3.0).sin(); // alpha pulse
        let glow_hue = (hue + 180.0) % 360.0;
        sprite.color = Color::oklch(0.7, 0.3, glow_hue).with_alpha(alpha);
        tf.scale = Vec3::splat(pulse);
    }
}

/// Merge all input sources into a single abstract direction vector.
fn gather_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Res<GamepadState>,
    mut movement: ResMut<MovementInput>,
) {
    let mut dir = Vec2::ZERO;

    // Keyboard
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }

    // Gamepad d-pad
    if gamepad.dpad_up { dir.y += 1.0; }
    if gamepad.dpad_down { dir.y -= 1.0; }
    if gamepad.dpad_left { dir.x -= 1.0; }
    if gamepad.dpad_right { dir.x += 1.0; }

    // Gamepad left stick
    if gamepad.left_stick.length() > STICK_DEADZONE {
        dir += gamepad.left_stick;
    }

    movement.0 = if dir == Vec2::ZERO { Vec2::ZERO } else { dir.normalize() };
}

/// Bumpers adjust scale: LB decreases, RB increases.
fn apply_bumper_scale(
    gamepad: Res<GamepadState>,
    mut config: ResMut<ScrollConfig>,
) {
    if gamepad.left_bumper_just_pressed && config.scale > 1 {
        config.scale -= 1;
    }
    if gamepad.right_bumper_just_pressed && config.scale < 20 {
        config.scale += 1;
    }
}

fn move_avatar(
    time: Res<Time>,
    config: Res<ScrollConfig>,
    movement: Res<MovementInput>,
    mut avatar_q: Query<&mut Transform, (With<Avatar>, Without<Glow>)>,
    mut glow_q: Query<&mut Transform, (With<Glow>, Without<Avatar>)>,
) {
    let dir = movement.0;
    if dir == Vec2::ZERO {
        return;
    }

    let Ok(mut tf) = avatar_q.single_mut() else { return };
    let delta = dir * AVATAR_SPEED * config.speed_mult * time.delta_secs();
    tf.translation.x += delta.x;
    tf.translation.y += delta.y;

    // Keep glow centered on avatar
    if let Ok(mut glow_tf) = glow_q.single_mut() {
        glow_tf.translation.x = tf.translation.x;
        glow_tf.translation.y = tf.translation.y;
    }
}

/// Stateful tile-relative camera with scale support.
///
/// At scale S the view covers S×S tiles.  The camera has a "home" position
/// (always a tile centre) and only shifts by one tile when the avatar pushes
/// through the per-texture buffer on an edge tile.  The buffer fraction is
/// relative to a single tile, not the whole view.
fn update_camera(
    config: Res<ScrollConfig>,
    mut home: ResMut<CameraHome>,
    mut cam_pos: ResMut<CameraPos>,
    avatar_q: Query<&Transform, With<Avatar>>,
    mut cam_q: Query<&mut Transform, (With<Camera2d>, Without<Avatar>)>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
) {
    let Ok(avatar_tf) = avatar_q.single() else { return };
    let Ok(mut cam_tf) = cam_q.single_mut() else { return };
    let Ok(mut proj) = proj_q.single_mut() else { return };

    let scale = config.scale as f32;
    if let Projection::Orthographic(ref mut ortho) = *proj {
        ortho.scale = scale;
    }

    let ax = avatar_tf.translation.x;
    let ay = avatar_tf.translation.y;

    let view_half = scale * TILE_PX / 2.0;

    // When scale actually changes, re-snap home.
    if config.scale != home.prev_scale {
        home.pos.x = snap_home(ax, config.scale);
        home.pos.y = snap_home(ay, config.scale);
        home.prev_scale = config.scale;
    }

    if config.scale <= 1 {
        // Scale 1: stateless per-tile camera (both buffers active per tile).
        cam_pos.0.x = tile_camera(ax, config.buffer_frac);
        cam_pos.0.y = tile_camera(ay, config.buffer_frac);
        home.pos = cam_pos.0;
    } else {
        let buffer = TILE_PX * config.buffer_frac;
        let dead_half = view_half - buffer;

        // Both axes use the same logic — ghost tiles handle visual tiling,
        // so no special case when the view exceeds the arena dimension.
        home.pos.x = axis_home(home.pos.x, ax, view_half);
        let offset_x = ax - home.pos.x;
        let scroll_x = axis_scroll(offset_x, dead_half, buffer);
        cam_pos.0.x = home.pos.x + scroll_x;

        home.pos.y = axis_home(home.pos.y, ay, view_half);
        let offset_y = ay - home.pos.y;
        let scroll_y = axis_scroll(offset_y, dead_half, buffer);
        cam_pos.0.y = home.pos.y + scroll_y;
    }

    cam_tf.translation.x = cam_pos.0.x;
    cam_tf.translation.y = cam_pos.0.y;
}

/// Scale-1 camera: stateless, per-tile.  Each buffer slides the camera half a
/// tile toward the boundary.  The matching buffer on the neighbouring tile
/// provides the other half, giving a smooth full-tile transition.
fn tile_camera(pos: f32, buffer_frac: f32) -> f32 {
    let tile = (pos / TILE_PX).floor();
    let center = tile * TILE_PX + TILE_PX / 2.0;
    let local = pos - tile * TILE_PX;

    let buffer = TILE_PX * buffer_frac;
    let inner_hi = TILE_PX - buffer;
    let half = TILE_PX / 2.0;

    if local > inner_hi {
        let depth = (local - inner_hi) / buffer;
        center + depth * half
    } else if local < buffer {
        let depth = (buffer - local) / buffer;
        center - depth * half
    } else {
        center
    }
}

/// Shift `home` by whole tiles until the avatar is inside the visible area.
fn axis_home(mut home: f32, avatar: f32, view_half: f32) -> f32 {
    loop {
        let offset = avatar - home;
        if offset > view_half {
            home += TILE_PX;
        } else if offset < -view_half {
            home -= TILE_PX;
        } else {
            return home;
        }
    }
}

/// Compute the camera scroll offset for one axis.
///
/// Inside the dead zone: no scroll.  In the buffer strip of an edge tile the
/// camera slides up to one full tile toward the avatar, proportionally to
/// buffer depth.
fn axis_scroll(offset: f32, dead_half: f32, buffer: f32) -> f32 {
    if offset > dead_half {
        let t = ((offset - dead_half) / buffer).clamp(0.0, 1.0);
        t * TILE_PX
    } else if offset < -dead_half {
        let t = ((-offset - dead_half) / buffer).clamp(0.0, 1.0);
        -t * TILE_PX
    } else {
        0.0
    }
}



fn hud_system(
    mut contexts: EguiContexts,
    mut config: ResMut<ScrollConfig>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Scroll Config")
        .anchor(egui::Align2::RIGHT_TOP, [-4.0, 4.0])
        .resizable(false)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(format!("Buffer: {:.1}%", config.buffer_frac * 100.0));
            ui.add(egui::Slider::new(&mut config.buffer_frac, 0.0..=0.5).text("buffer"));

            ui.separator();

            let mut scale_i32 = config.scale as i32;
            ui.label(format!("Scale: {}x{}", config.scale, config.scale));
            ui.add(egui::Slider::new(&mut scale_i32, 1..=20).text("scale"));
            config.scale = scale_i32.max(1) as u32;

            ui.separator();

            ui.label(format!("Speed: {:.1}x", config.speed_mult));
            ui.add(egui::Slider::new(&mut config.speed_mult, 1.0..=10.0).text("speed"));
        });
}

/// Wrap `delta` into [-period/2, period/2).
fn wrap_offset(delta: f32, period: f32) -> f32 {
    (delta + period / 2.0).rem_euclid(period) - period / 2.0
}

/// Pick the grid-aligned home position that places `avatar_pos` closest to
/// the centre of the view.  Odd scales use tile-centre grids; even scales
/// use tile-boundary grids.
fn snap_home(avatar_pos: f32, scale: u32) -> f32 {
    if scale % 2 == 1 {
        // Odd grid: TILE_PX/2, 3·TILE_PX/2, …
        ((avatar_pos / TILE_PX - 0.5).round() + 0.5) * TILE_PX
    } else {
        // Even grid: 0, TILE_PX, 2·TILE_PX, …
        (avatar_pos / TILE_PX).round() * TILE_PX
    }
}

/// Spawn or despawn ghost tile copies when the scale changes so the torus
/// visually tiles at any zoom level.
fn manage_ghosts(
    mut commands: Commands,
    config: Res<ScrollConfig>,
    mut ghost_config: ResMut<GhostConfig>,
    tile_map: Res<TileMap>,
    tiles: Query<(Entity, &Tile)>,
) {
    let scale = config.scale as f32;
    let view_half = scale * TILE_PX / 2.0;

    // How many extra full-arena copies we need in each direction per axis.
    let needed_x = ((view_half + ARENA_PX_W / 2.0) / ARENA_PX_W).ceil() as i32 - 1;
    let needed_y = ((view_half + ARENA_PX_H / 2.0) / ARENA_PX_H).ceil() as i32 - 1;

    if needed_x == ghost_config.copies_x && needed_y == ghost_config.copies_y {
        return;
    }

    // Despawn all existing ghosts.
    for (entity, tile) in &tiles {
        if tile.copy_x != 0 || tile.copy_y != 0 {
            commands.entity(entity).despawn();
        }
    }

    // Spawn new ghosts for every (nx, ny) pair except (0, 0).
    for nx in -needed_x..=needed_x {
        for ny in -needed_y..=needed_y {
            if nx == 0 && ny == 0 {
                continue;
            }
            for row in 0..ARENA_ROWS {
                for col in 0..ARENA_COLS {
                    let handle = tile_map.handles[row][col].clone();
                    commands.spawn((
                        Tile { grid_x: col, grid_y: row, copy_x: nx, copy_y: ny },
                        Sprite {
                            image: handle,
                            ..default()
                        },
                        Transform::from_xyz(
                            col as f32 * TILE_PX + TILE_PX / 2.0 + nx as f32 * ARENA_PX_W,
                            row as f32 * TILE_PX + TILE_PX / 2.0 + ny as f32 * ARENA_PX_H,
                            0.0,
                        ),
                    ));
                }
            }
        }
    }

    ghost_config.copies_x = needed_x;
    ghost_config.copies_y = needed_y;
}

/// Reposition tiles so they wrap seamlessly around the camera.
/// Ghost copies are offset by their copy_x/copy_y periods.
fn wrap_tiles(cam_pos: Res<CameraPos>, mut tiles: Query<(&Tile, &mut Transform)>) {
    let cam = cam_pos.0;

    for (tile, mut tf) in &mut tiles {
        let base_x = tile.grid_x as f32 * TILE_PX + TILE_PX / 2.0;
        let base_y = tile.grid_y as f32 * TILE_PX + TILE_PX / 2.0;

        tf.translation.x = cam.x + wrap_offset(base_x - cam.x, ARENA_PX_W) + tile.copy_x as f32 * ARENA_PX_W;
        tf.translation.y = cam.y + wrap_offset(base_y - cam.y, ARENA_PX_H) + tile.copy_y as f32 * ARENA_PX_H;
    }
}
