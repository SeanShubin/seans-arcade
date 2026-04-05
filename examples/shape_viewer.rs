//! Shape description viewer — loads a RON shape file and renders it.
//!
//! Demonstrates the shape description interpreter: templates, mirror combinator,
//! and hierarchical composition from pure data.
//!
//! Usage:
//!   cargo run --example shape_viewer
//!   cargo run --example shape_viewer -- path/to/shape.ron
//!
//! Controls:
//!   Scroll wheel — zoom
//!   Middle mouse drag — orbit
//!   Arrow keys — orbit
//!   R — reload shape file
//!   F1 — toggle debug gizmos
//!
//! Default shape: data/shapes/scout_bot.ron

#[path = "shared/shape.rs"]
mod shape;

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use shape::*;

// =====================================================================
// Constants
// =====================================================================

const DEFAULT_SHAPE_PATH: &str = "data/shapes/scout_bot.ron";

const ISO_DISTANCE: f32 = 15.0;
const ISO_PITCH: f32 = 35.264;
const ISO_YAW: f32 = 45.0;
const ZOOM_SPEED_LINE: f32 = 0.15;
const ZOOM_SPEED_PIXEL: f32 = 0.002;
const ZOOM_MIN: f32 = 0.002;
const ZOOM_MAX: f32 = 0.5;
const DEFAULT_ZOOM: f32 = 0.012;

// =====================================================================
// Resources & Components
// =====================================================================

#[derive(Resource)]
struct ShapeFilePath(String);

#[derive(Resource)]
struct NeedsReload(bool);

#[derive(Resource)]
struct OrbitState {
    yaw: f32,   // degrees
    pitch: f32, // degrees
    target: Vec3, // point the camera orbits around
}

impl Default for OrbitState {
    fn default() -> Self {
        Self { yaw: ISO_YAW, pitch: ISO_PITCH, target: Vec3::ZERO }
    }
}

#[derive(Resource, Default)]
struct DebugSettings {
    show_gizmos: bool,
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct HudLabel;

// =====================================================================
// Entry point
// =====================================================================

fn main() {
    let shape_path = std::env::args().nth(1).unwrap_or_else(|| DEFAULT_SHAPE_PATH.to_string());

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Shape Viewer".into(),
                resolution: bevy::window::WindowResolution::new(900, 700),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ShapeFilePath(shape_path))
        .insert_resource(NeedsReload(true))
        .insert_resource(OrbitState::default())
        .insert_resource(DebugSettings::default())
        .add_systems(Startup, (setup_scene, setup_hud))
        .add_systems(Update, (
            reload_shape,
            orbit_camera,
            camera_zoom,
            keyboard_input,
            update_hud,
            draw_gizmos,
        ))
        .run();
}

fn setup_scene(mut commands: Commands) {
    // Camera
    let pitch_rad = ISO_PITCH.to_radians();
    let yaw_rad = ISO_YAW.to_radians();
    let rotation = Quat::from_euler(EulerRot::YXZ, -yaw_rad, -pitch_rad, 0.0);
    let position = rotation * Vec3::new(0.0, 0.0, ISO_DISTANCE);

    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: DEFAULT_ZOOM,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));

    // Ambient light
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        affects_lightmapped_meshes: true,
    });
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        HudLabel,
        Text::new(""),
        TextFont { font_size: 18.0, ..default() },
        Node { margin: UiRect::all(Val::Px(10.0)), ..default() },
    ));
}

// =====================================================================
// Shape loading
// =====================================================================

fn reload_shape(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut needs_reload: ResMut<NeedsReload>,
    shape_path: Res<ShapeFilePath>,
    existing: Query<Entity, With<ShapeRoot>>,
) {
    if !needs_reload.0 { return; }
    needs_reload.0 = false;

    // Despawn existing
    let roots: Vec<Entity> = existing.iter().collect();
    despawn_shape(&mut commands, &roots);

    // Load and parse — try path as-is, then relative to the manifest dir
    let path = std::path::Path::new(&shape_path.0);
    let resolved = if path.exists() {
        path.to_path_buf()
    } else {
        // Try relative to the crate root (where Cargo.toml lives)
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join(path)
    };
    info!("Resolved shape path: {}", resolved.display());

    let ron_str = match std::fs::read_to_string(&resolved) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to read shape file '{}' (resolved: '{}'): {}",
                shape_path.0, resolved.display(), e);
            return;
        }
    };

    let shape_file = match load_shape(&ron_str) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to parse shape file '{}': {}", shape_path.0, e);
            return;
        }
    };

    info!("Loaded shape from '{}' — root has {} children, {} templates",
        shape_path.0, shape_file.root.children.len(), shape_file.templates.len());
    let root = spawn_shape(&mut commands, &mut meshes, &mut materials, &shape_file);
    info!("Spawned shape root entity: {:?}", root);
}

// =====================================================================
// Camera
// =====================================================================

fn orbit_camera(
    mut orbit: ResMut<OrbitState>,
    mut camera: Query<(&mut Transform, &Projection), With<MainCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let Ok((mut tf, proj)) = camera.single_mut() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };

    if mouse.pressed(MouseButton::Middle) {
        // Middle mouse drag to pan
        for ev in motion.read() {
            let right = tf.right();
            let up = tf.up();
            orbit.target += (-ev.delta.x * right + ev.delta.y * up) * scale;
        }
    } else if mouse.pressed(MouseButton::Left) {
        // Left mouse drag to orbit
        for ev in motion.read() {
            orbit.yaw += ev.delta.x * 0.3;
            orbit.pitch = (orbit.pitch + ev.delta.y * 0.3).clamp(5.0, 85.0);
        }
    } else {
        motion.clear();
    }

    // Arrow keys to orbit
    let speed = 60.0 * time.delta_secs();
    if keys.pressed(KeyCode::ArrowLeft) { orbit.yaw += speed; }
    if keys.pressed(KeyCode::ArrowRight) { orbit.yaw -= speed; }
    if keys.pressed(KeyCode::ArrowUp) { orbit.pitch = (orbit.pitch + speed).min(85.0); }
    if keys.pressed(KeyCode::ArrowDown) { orbit.pitch = (orbit.pitch - speed).max(5.0); }

    // Update camera position
    let pitch_rad = orbit.pitch.to_radians();
    let yaw_rad = orbit.yaw.to_radians();
    let rotation = Quat::from_euler(EulerRot::YXZ, -yaw_rad, -pitch_rad, 0.0);
    let position = orbit.target + rotation * Vec3::new(0.0, 0.0, ISO_DISTANCE);
    tf.translation = position;
    tf.look_at(orbit.target, Vec3::Y);
}

fn camera_zoom(
    mut query: Query<&mut Projection, With<MainCamera>>,
    mut scroll: MessageReader<MouseWheel>,
) {
    for ev in scroll.read() {
        for mut proj in &mut query {
            if let Projection::Orthographic(ortho) = proj.as_mut() {
                let delta = match ev.unit {
                    MouseScrollUnit::Line => -ev.y * ZOOM_SPEED_LINE,
                    MouseScrollUnit::Pixel => -ev.y * ZOOM_SPEED_PIXEL,
                };
                ortho.scale = (ortho.scale * (1.0 + delta)).clamp(ZOOM_MIN, ZOOM_MAX);
            }
        }
    }
}

// =====================================================================
// Input
// =====================================================================

fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut needs_reload: ResMut<NeedsReload>,
    mut debug: ResMut<DebugSettings>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        needs_reload.0 = true;
        info!("Reloading shape...");
    }
    if keys.just_pressed(KeyCode::F1) {
        debug.show_gizmos = !debug.show_gizmos;
    }
}

// =====================================================================
// HUD
// =====================================================================

fn update_hud(
    shape_path: Res<ShapeFilePath>,
    parts: Query<&ShapePart>,
    mut hud: Query<&mut Text, With<HudLabel>>,
) {
    let Ok(mut text) = hud.single_mut() else { return };
    let part_count = parts.iter().count();
    **text = format!(
        "File: {}  |  Parts: {}  |  R: reload  |  Scroll: zoom  |  Middle drag: orbit",
        shape_path.0, part_count,
    );
}

// =====================================================================
// Debug gizmos
// =====================================================================

fn draw_gizmos(
    debug: Res<DebugSettings>,
    mut gizmos: Gizmos,
    parts: Query<(&GlobalTransform, &ShapePart)>,
) {
    if !debug.show_gizmos { return; }

    // Draw origin axes
    gizmos.line(Vec3::ZERO, Vec3::X * 0.5, Color::srgb(1.0, 0.0, 0.0));
    gizmos.line(Vec3::ZERO, Vec3::Y * 0.5, Color::srgb(0.0, 1.0, 0.0));
    gizmos.line(Vec3::ZERO, Vec3::Z * 0.5, Color::srgb(0.0, 0.0, 1.0));

    // Draw part origins
    for (gtf, _part) in &parts {
        let pos = gtf.translation();
        gizmos.sphere(
            Isometry3d::from_translation(pos),
            0.02,
            Color::srgb(1.0, 1.0, 0.0),
        );
    }
}
