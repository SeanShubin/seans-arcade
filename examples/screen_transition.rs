//! Screen-transition prototype with hybrid scrolling.
//!
//! 14x14 wrapping arena of random background textures.
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
use rand::seq::SliceRandom;

const ARENA_CELLS: usize = 14;
const TILE_PX: f32 = 1024.0;
const ARENA_PX: f32 = ARENA_CELLS as f32 * TILE_PX;
const VIEWPORT_PX: f32 = 1024.0;
const ORB_RADIUS: f32 = 14.0;
const GLOW_RADIUS: f32 = 22.0;
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
        .insert_resource(CameraHome(Vec2::new(
            ARENA_PX / 2.0 + TILE_PX / 2.0,
            ARENA_PX / 2.0 + TILE_PX / 2.0,
        )))
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, hud_system)
        .add_systems(Update, (animate_orb, move_avatar, update_camera, wrap_tiles, wrap_avatar, sync_borders).chain())
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
}

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
        Self { buffer_frac: 0.25, scale: 1, speed_mult: 1.0 }
    }
}

/// The camera's snap position — always at a tile centre.  Shifts by one tile
/// when the avatar pushes through the buffer at the edge of the visible area.
#[derive(Resource)]
struct CameraHome(Vec2);

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

    // --- Chromatic orb avatar ---
    let orb_tex = make_circle_texture(&mut images, (ORB_RADIUS * 2.0) as u32, false);
    let glow_tex = make_circle_texture(&mut images, (GLOW_RADIUS * 2.0) as u32, true);

    let start = Vec2::new(ARENA_PX / 2.0 + TILE_PX / 2.0, ARENA_PX / 2.0 + TILE_PX / 2.0);

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
    commands.insert_resource(CameraPos(Vec2::new(ARENA_PX / 2.0 + TILE_PX / 2.0, ARENA_PX / 2.0 + TILE_PX / 2.0)));
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

fn move_avatar(
    time: Res<Time>,
    config: Res<ScrollConfig>,
    input: Res<ButtonInput<KeyCode>>,
    mut avatar_q: Query<&mut Transform, (With<Avatar>, Without<Glow>)>,
    mut glow_q: Query<&mut Transform, (With<Glow>, Without<Avatar>)>,
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

    let Ok(mut tf) = avatar_q.single_mut() else { return };
    let delta = dir * AVATAR_SPEED * config.speed_mult * time.delta_secs();
    tf.translation.x = (tf.translation.x + delta.x).rem_euclid(ARENA_PX);
    tf.translation.y = (tf.translation.y + delta.y).rem_euclid(ARENA_PX);

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

    if config.scale <= 1 {
        // Scale 1: stateless per-tile camera (both buffers active per tile).
        cam_pos.0.x = tile_camera(ax, config.buffer_frac).rem_euclid(ARENA_PX);
        cam_pos.0.y = tile_camera(ay, config.buffer_frac).rem_euclid(ARENA_PX);
        home.0 = cam_pos.0;
    } else {
        // Scale > 1: stateful home-based camera.  Buffer only on edge tiles.
        let buffer = TILE_PX * config.buffer_frac;
        let view_half = scale * TILE_PX / 2.0;
        let dead_half = view_half - buffer;

        home.0.x = axis_home(home.0.x, ax, view_half);
        home.0.y = axis_home(home.0.y, ay, view_half);

        let offset_x = wrap_offset(ax - home.0.x, ARENA_PX);
        let offset_y = wrap_offset(ay - home.0.y, ARENA_PX);

        let scroll_x = axis_scroll(offset_x, dead_half, buffer);
        let scroll_y = axis_scroll(offset_y, dead_half, buffer);

        cam_pos.0.x = (home.0.x + scroll_x).rem_euclid(ARENA_PX);
        cam_pos.0.y = (home.0.y + scroll_y).rem_euclid(ARENA_PX);
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
        let offset = wrap_offset(avatar - home, ARENA_PX);
        if offset > view_half {
            home = (home + TILE_PX).rem_euclid(ARENA_PX);
        } else if offset < -view_half {
            home = (home - TILE_PX).rem_euclid(ARENA_PX);
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

/// Wrap the avatar's rendered position relative to the camera, just like tiles.
fn wrap_avatar(
    cam_pos: Res<CameraPos>,
    mut avatar_q: Query<&mut Transform, (With<Avatar>, Without<Glow>)>,
    mut glow_q: Query<&mut Transform, (With<Glow>, Without<Avatar>)>,
) {
    let Ok(mut tf) = avatar_q.single_mut() else { return };
    let cam = cam_pos.0;
    tf.translation.x = cam.x + wrap_offset(tf.translation.x - cam.x, ARENA_PX);
    tf.translation.y = cam.y + wrap_offset(tf.translation.y - cam.y, ARENA_PX);

    if let Ok(mut glow_tf) = glow_q.single_mut() {
        glow_tf.translation.x = tf.translation.x;
        glow_tf.translation.y = tf.translation.y;
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
            ui.add(egui::Slider::new(&mut scale_i32, 1..=7).text("scale"));
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
