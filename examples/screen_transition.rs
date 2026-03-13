//! Screen-transition prototype with hybrid scrolling.
//!
//! 14x14 wrapping arena of random background textures.
//! - Inner area (90% of screen): avatar moves freely, camera stays still
//! - Buffer area (5% each side): camera tracks proportionally to buffer depth
//!
//! Controls: WASD or arrow keys to move
//!
//! Run with: `cargo run --example screen_transition`

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use rand::seq::SliceRandom;

const ARENA_CELLS: usize = 14;
const TILE_PX: f32 = 1024.0;
const ARENA_PX: f32 = ARENA_CELLS as f32 * TILE_PX;
const VIEWPORT_PX: f32 = 1024.0;
const AVATAR_SIZE: f32 = 24.0;
const AVATAR_SPEED: f32 = 400.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Screen Transition Prototype".into(),
                    resolution: bevy::window::WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(ClearColor(Color::BLACK))
        .init_resource::<CameraPos>()
        .init_resource::<ScrollConfig>()
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, hud_system)
        .add_systems(Update, (move_avatar, update_camera, wrap_tiles, sync_borders).chain())
        .run();
}

#[derive(Component)]
struct Avatar;

#[derive(Component)]
struct Border;

#[derive(Component)]
struct Tile {
    grid_x: usize,
    grid_y: usize,
}

/// Logical camera position, separate from the Transform so we can wrap cleanly.
#[derive(Resource, Default)]
struct CameraPos(Vec2);

#[derive(Resource)]
struct ScrollConfig {
    buffer_frac: f32,
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self { buffer_frac: 0.25 }
    }
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
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

    let mut rng = rand::rng();
    texture_paths.shuffle(&mut rng);

    let needed = ARENA_CELLS * ARENA_CELLS;
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

    // Tiles
    for row in 0..ARENA_CELLS {
        for col in 0..ARENA_CELLS {
            let idx = row * ARENA_CELLS + col;
            let handle: Handle<Image> = asset_server.load(&texture_paths[idx]);

            commands.spawn((
                Tile { grid_x: col, grid_y: row },
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

    // Avatar — small green square, starts at arena centre
    commands.spawn((
        Avatar,
        Sprite {
            color: Color::srgb(0.2, 0.9, 0.2),
            custom_size: Some(Vec2::splat(AVATAR_SIZE)),
            ..default()
        },
        Transform::from_xyz(ARENA_PX / 2.0 + TILE_PX / 2.0, ARENA_PX / 2.0 + TILE_PX / 2.0, 1.0),
    ));

    // Camera starts centred on avatar
    commands.insert_resource(CameraPos(Vec2::new(ARENA_PX / 2.0 + TILE_PX / 2.0, ARENA_PX / 2.0 + TILE_PX / 2.0)));
}

/// Position four black panels around the 1024x1024 game area to mask overflow.
/// The borders follow the camera so they stay fixed on screen.
fn sync_borders(
    window_q: Query<&Window>,
    cam_pos: Res<CameraPos>,
    mut borders: Query<(&mut Transform, &mut Sprite), With<Border>>,
) {
    let Ok(win) = window_q.single() else { return };
    let win_w = win.width();
    let win_h = win.height();
    let half_vp = VIEWPORT_PX / 2.0;
    let cx = cam_pos.0.x;
    let cy = cam_pos.0.y;

    // Side bars (left, right) and top/bottom bars
    let bar_w = (win_w - VIEWPORT_PX) / 2.0;
    let bar_h = (win_h - VIEWPORT_PX) / 2.0;

    // (offset_x, offset_y, width, height)
    let panels = [
        // left
        (-(half_vp + bar_w / 2.0), 0.0, bar_w.max(0.0), win_h),
        // right
        (half_vp + bar_w / 2.0, 0.0, bar_w.max(0.0), win_h),
        // top
        (0.0, half_vp + bar_h / 2.0, win_w, bar_h.max(0.0)),
        // bottom
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

fn move_avatar(
    time: Res<Time>,
    input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, With<Avatar>>,
) {
    let mut dir = Vec2::ZERO;
    if input.pressed(KeyCode::KeyW) || input.pressed(KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if input.pressed(KeyCode::KeyS) || input.pressed(KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if input.pressed(KeyCode::KeyA) || input.pressed(KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if input.pressed(KeyCode::KeyD) || input.pressed(KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    if dir == Vec2::ZERO {
        return;
    }
    dir = dir.normalize();

    let Ok(mut tf) = query.single_mut() else { return };
    let delta = dir * AVATAR_SPEED * time.delta_secs();
    tf.translation.x = (tf.translation.x + delta.x).rem_euclid(ARENA_PX);
    tf.translation.y = (tf.translation.y + delta.y).rem_euclid(ARENA_PX);
}

/// Tile-relative hybrid camera.
///
/// The inner/buffer zones are relative to the current tile, not the viewport.
/// While the avatar is in the inner 90% of a tile the camera locks to that
/// tile's centre (you see only one texture).  In the 5% buffer strip on any
/// edge, the camera slides toward the neighbouring tile proportionally to
/// depth into the buffer.  Works for diagonals — each axis is independent.
fn update_camera(
    config: Res<ScrollConfig>,
    mut cam_pos: ResMut<CameraPos>,
    avatar_q: Query<&Transform, With<Avatar>>,
    mut cam_q: Query<&mut Transform, (With<Camera2d>, Without<Avatar>)>,
) {
    let Ok(avatar_tf) = avatar_q.single() else { return };
    let Ok(mut cam_tf) = cam_q.single_mut() else { return };

    let ax = avatar_tf.translation.x;
    let ay = avatar_tf.translation.y;

    cam_pos.0.x = tile_camera(ax, config.buffer_frac).rem_euclid(ARENA_PX);
    cam_pos.0.y = tile_camera(ay, config.buffer_frac).rem_euclid(ARENA_PX);

    cam_tf.translation.x = cam_pos.0.x;
    cam_tf.translation.y = cam_pos.0.y;
}

/// For one axis, compute the camera position from the avatar's world coordinate.
///
/// Returns the tile centre when the avatar is in the inner zone, and smoothly
/// slides toward the tile boundary when the avatar enters the buffer strip.
fn tile_camera(pos: f32, buffer_frac: f32) -> f32 {
    let tile = (pos / TILE_PX).floor();
    let center = tile * TILE_PX + TILE_PX / 2.0;
    let local = pos - tile * TILE_PX; // [0, TILE_PX)

    let buffer = TILE_PX * buffer_frac;
    let inner_lo = buffer;
    let inner_hi = TILE_PX - buffer;
    let half = TILE_PX / 2.0;

    if local > inner_hi {
        let depth = (local - inner_hi) / buffer;
        center + depth * half
    } else if local < inner_lo {
        let depth = (inner_lo - local) / buffer;
        center - depth * half
    } else {
        center
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
        });
}

/// Wrap `delta` into [-period/2, period/2).
fn wrap_offset(delta: f32, period: f32) -> f32 {
    (delta + period / 2.0).rem_euclid(period) - period / 2.0
}

/// Reposition tiles so they wrap seamlessly around the camera.
fn wrap_tiles(cam_pos: Res<CameraPos>, mut tiles: Query<(&Tile, &mut Transform)>) {
    let cam = cam_pos.0;

    for (tile, mut tf) in &mut tiles {
        let base_x = tile.grid_x as f32 * TILE_PX + TILE_PX / 2.0;
        let base_y = tile.grid_y as f32 * TILE_PX + TILE_PX / 2.0;

        tf.translation.x = cam.x + wrap_offset(base_x - cam.x, ARENA_PX);
        tf.translation.y = cam.y + wrap_offset(base_y - cam.y, ARENA_PX);
    }
}
