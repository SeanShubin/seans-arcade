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
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, EguiPlugin, egui};
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
        .add_plugins((DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Shape Viewer".into(),
                resolution: bevy::window::WindowResolution::new(900, 700),
                ..default()
            }),
            ..default()
        }), EguiPlugin::default()))
        .insert_resource(ShapeFilePath(shape_path))
        .insert_resource(NeedsReload(true))
        .insert_resource(OrbitState::default())
        .insert_resource(DebugSettings::default())
        .add_systems(Startup, (setup_scene, setup_hud))
        .add_systems(EguiPrimaryContextPass, part_tree_ui)
        .add_systems(Update, (
            reload_shape,
            orbit_camera,
            camera_zoom,
            keyboard_input,
            animate_shapes,
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
        Node { margin: UiRect::all(Val::Px(10.0)), left: Val::Px(220.0), ..default() },
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
    mut contexts: EguiContexts,
) {
    let egui_wants = contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_pointer_input());
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
    } else if mouse.pressed(MouseButton::Left) && !egui_wants {
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
    mut animators: Query<&mut ShapeAnimator>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        needs_reload.0 = true;
        info!("Reloading shape...");
    }
    if keys.just_pressed(KeyCode::F1) {
        debug.show_gizmos = !debug.show_gizmos;
    }
    if keys.just_pressed(KeyCode::Tab) {
        for mut animator in &mut animators {
            animator.cycle_state();
            info!("Animation: {}", animator.active_name());
        }
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

// =====================================================================
// Part tree UI
// =====================================================================

/// Tri-state: all visible, all hidden, or mixed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TriState {
    Visible,
    Hidden,
    Mixed,
}

fn part_tree_ui(
    mut contexts: EguiContexts,
    roots: Query<Entity, With<ShapeRoot>>,
    parts: Query<(&ShapePart, Option<&Children>, &Visibility)>,
    mut animators: Query<&mut ShapeAnimator>,
    mut commands: Commands,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut toggles: Vec<(Entity, Visibility)> = Vec::new();

    egui::SidePanel::left("part_tree").min_width(200.0).show(ctx, |ui| {
        // Animation controls
        for root in &roots {
            if let Ok(mut animator) = animators.get_mut(root) {
                ui.heading("Animation");
                ui.horizontal(|ui| {
                    ui.label("State:");
                    if ui.button(animator.active_name()).clicked() {
                        animator.cycle_state();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Speed:");
                    ui.add(egui::Slider::new(&mut animator.speed, 0.0..=5.0));
                });
                ui.separator();
            }
        }

        ui.heading("Part Tree");
        ui.separator();

        for root in &roots {
            draw_tree_node(ui, root, &parts, &mut toggles, 0, &[]);
        }
    });

    for (entity, vis) in toggles {
        commands.entity(entity).insert(vis);
    }
}

/// Compute the tri-state for a node based on its own visibility and all descendants.
fn compute_tri_state(
    entity: Entity,
    parts: &Query<(&ShapePart, Option<&Children>, &Visibility)>,
) -> TriState {
    let Ok((_part, children, vis)) = parts.get(entity) else {
        return TriState::Visible;
    };

    let self_visible = *vis != Visibility::Hidden;

    // Collect shape-part children
    let child_parts: Vec<Entity> = children
        .map(|c| c.iter().filter(|e| parts.get(*e).is_ok()).collect())
        .unwrap_or_default();

    if child_parts.is_empty() {
        // Leaf node — just this node's visibility
        return if self_visible { TriState::Visible } else { TriState::Hidden };
    }

    // Branch node — combine own state with children
    let mut all_visible = self_visible;
    let mut all_hidden = !self_visible;
    for child in &child_parts {
        match compute_tri_state(*child, parts) {
            TriState::Visible => all_hidden = false,
            TriState::Hidden => all_visible = false,
            TriState::Mixed => { all_visible = false; all_hidden = false; }
        }
    }

    if all_visible { TriState::Visible }
    else if all_hidden { TriState::Hidden }
    else { TriState::Mixed }
}

/// Collect all shape-part entities in the subtree (including the root).
fn collect_subtree(
    entity: Entity,
    parts: &Query<(&ShapePart, Option<&Children>, &Visibility)>,
    out: &mut Vec<Entity>,
) {
    if parts.get(entity).is_err() { return; }
    out.push(entity);
    if let Ok((_, Some(children), _)) = parts.get(entity) {
        for child in children.iter() {
            collect_subtree(child, parts, out);
        }
    }
}

fn draw_tree_node(
    ui: &mut egui::Ui,
    entity: Entity,
    parts: &Query<(&ShapePart, Option<&Children>, &Visibility)>,
    toggles: &mut Vec<(Entity, Visibility)>,
    depth: usize,
    ancestors: &[Entity],
) {
    let Ok((part, children, _vis)) = parts.get(entity) else { return };

    let state = compute_tri_state(entity, parts);
    let label = part.name.as_deref().unwrap_or("(unnamed)");
    let indent = "  ".repeat(depth);
    let icon = match state {
        TriState::Visible => "[+]",
        TriState::Hidden => "[-]",
        TriState::Mixed => "[~]",
    };

    if ui.selectable_label(false, format!("{indent}{icon} {label}")).clicked() {
        // Clicking visible or mixed → hide all. Clicking hidden → show all.
        let new_vis = match state {
            TriState::Hidden => Visibility::Inherited,
            _ => Visibility::Hidden,
        };
        let mut subtree = Vec::new();
        collect_subtree(entity, parts, &mut subtree);
        for e in subtree {
            toggles.push((e, new_vis));
        }
        // When showing, ensure all ancestors are also visible
        if new_vis == Visibility::Inherited {
            for &ancestor in ancestors {
                toggles.push((ancestor, Visibility::Inherited));
            }
        }
    }

    if let Some(children) = children {
        let mut path = ancestors.to_vec();
        path.push(entity);
        for child in children.iter() {
            if parts.get(child).is_ok() {
                draw_tree_node(ui, child, parts, toggles, depth + 1, &path);
            }
        }
    }
}
