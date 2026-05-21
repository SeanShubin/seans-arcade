//! Procedural material lab — custom WGSL shader materials on 3D shapes.
//!
//! Demonstrates seamless procedural materials evaluated in world/object space.
//! No UV mapping, no texture files. Pattern flows across faces and parts.
//!
//! Controls:
//!   Left drag — orbit camera
//!   Middle drag — pan
//!   Scroll — zoom
//!   1-5 — material presets
//!   Left panel — tweak material parameters
//!
//! Run with: cargo run --example material_lab

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::pbr::Material;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, EguiPlugin, egui};

// =====================================================================
// Custom material
// =====================================================================

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ProceduralMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub noise_scale: f32,
    #[uniform(0)]
    pub noise_strength: f32,
    #[uniform(0)]
    pub color_variation: LinearRgba,
    #[uniform(0)]
    pub roughness: f32,
    #[uniform(0)]
    pub _padding: f32,
}

impl Material for ProceduralMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/procedural_material.wgsl".into()
    }
}

impl Default for ProceduralMaterial {
    fn default() -> Self {
        Self {
            base_color: LinearRgba::new(0.45, 0.45, 0.50, 1.0),
            noise_scale: 8.0,
            noise_strength: 0.15,
            color_variation: LinearRgba::new(0.08, 0.06, 0.04, 0.0),
            roughness: 0.6,
            _padding: 0.0,
        }
    }
}

// =====================================================================
// Presets
// =====================================================================

fn preset_brushed_metal() -> ProceduralMaterial {
    ProceduralMaterial {
        base_color: LinearRgba::new(0.5, 0.5, 0.55, 1.0),
        noise_scale: 12.0,
        noise_strength: 0.08,
        color_variation: LinearRgba::new(0.05, 0.05, 0.06, 0.0),
        roughness: 0.3,
        _padding: 0.0,
    }
}

fn preset_rusted_steel() -> ProceduralMaterial {
    ProceduralMaterial {
        base_color: LinearRgba::new(0.45, 0.3, 0.2, 1.0),
        noise_scale: 6.0,
        noise_strength: 0.3,
        color_variation: LinearRgba::new(0.15, 0.1, 0.05, 0.0),
        roughness: 0.8,
        _padding: 0.0,
    }
}

fn preset_dark_composite() -> ProceduralMaterial {
    ProceduralMaterial {
        base_color: LinearRgba::new(0.15, 0.15, 0.18, 1.0),
        noise_scale: 20.0,
        noise_strength: 0.05,
        color_variation: LinearRgba::new(0.03, 0.03, 0.04, 0.0),
        roughness: 0.4,
        _padding: 0.0,
    }
}

fn preset_stone() -> ProceduralMaterial {
    ProceduralMaterial {
        base_color: LinearRgba::new(0.4, 0.38, 0.35, 1.0),
        noise_scale: 5.0,
        noise_strength: 0.2,
        color_variation: LinearRgba::new(0.1, 0.08, 0.06, 0.0),
        roughness: 0.9,
        _padding: 0.0,
    }
}

fn preset_energy() -> ProceduralMaterial {
    ProceduralMaterial {
        base_color: LinearRgba::new(0.1, 0.4, 0.8, 1.0),
        noise_scale: 4.0,
        noise_strength: 0.4,
        color_variation: LinearRgba::new(0.2, 0.3, 0.1, 0.0),
        roughness: 0.1,
        _padding: 0.0,
    }
}

const PRESET_NAMES: &[&str] = &["Brushed Metal", "Rusted Steel", "Dark Composite", "Stone", "Energy"];

fn preset_by_index(i: usize) -> ProceduralMaterial {
    match i {
        0 => preset_brushed_metal(),
        1 => preset_rusted_steel(),
        2 => preset_dark_composite(),
        3 => preset_stone(),
        4 => preset_energy(),
        _ => ProceduralMaterial::default(),
    }
}

// =====================================================================
// Resources & components
// =====================================================================

#[derive(Resource)]
struct MaterialParams {
    base_color: [f32; 3],
    noise_scale: f32,
    noise_strength: f32,
    color_variation: [f32; 3],
    roughness: f32,
    active_preset: usize,
}

impl Default for MaterialParams {
    fn default() -> Self {
        Self {
            base_color: [0.5, 0.5, 0.55],
            noise_scale: 8.0,
            noise_strength: 0.15,
            color_variation: [0.08, 0.06, 0.04],
            roughness: 0.6,
            active_preset: 0,
        }
    }
}

#[derive(Resource)]
struct MaterialDirty(bool);

#[derive(Resource)]
struct OrbitState {
    yaw: f32,
    pitch: f32,
    target: Vec3,
}

impl Default for OrbitState {
    fn default() -> Self {
        Self { yaw: 45.0, pitch: 35.0, target: Vec3::ZERO }
    }
}

#[derive(Component)]
struct MainCamera;

#[derive(Component)]
struct DemoShape;

// =====================================================================
// Constants
// =====================================================================

const ISO_DISTANCE: f32 = 15.0;
const ZOOM_MIN: f32 = 0.002;
const ZOOM_MAX: f32 = 0.5;
const DEFAULT_ZOOM: f32 = 0.015;

// =====================================================================
// Entry point
// =====================================================================

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Procedural Material Lab".into(),
                    resolution: bevy::window::WindowResolution::new(1100, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
            MaterialPlugin::<ProceduralMaterial>::default(),
        ))
        .insert_resource(MaterialParams::default())
        .insert_resource(MaterialDirty(true))
        .insert_resource(OrbitState::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(Update, (
            apply_material,
            orbit_camera,
            camera_zoom,
            keyboard_input,
        ))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ProceduralMaterial>>,
) {
    // Camera
    let pitch_rad = 35.0f32.to_radians();
    let yaw_rad = 45.0f32.to_radians();
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

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        affects_lightmapped_meshes: true,
    });

    let mat = materials.add(ProceduralMaterial::default());

    // Demo shapes — multiple primitives sharing the same material
    // The procedural texture should flow seamlessly across all of them

    // Large box (chassis-like)
    commands.spawn((
        DemoShape,
        Mesh3d(meshes.add(Cuboid::new(1.4, 0.5, 0.8))),
        MeshMaterial3d(mat.clone()),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    // Sphere on top (head-like)
    commands.spawn((
        DemoShape,
        Mesh3d(meshes.add(Sphere::new(0.3))),
        MeshMaterial3d(mat.clone()),
        Transform::from_xyz(0.0, 1.05, 0.0),
    ));

    // Cylinders on sides (arm-like)
    commands.spawn((
        DemoShape,
        Mesh3d(meshes.add(Cylinder::new(0.1, 0.6))),
        MeshMaterial3d(mat.clone()),
        Transform::from_xyz(-0.85, 0.5, 0.0),
    ));
    commands.spawn((
        DemoShape,
        Mesh3d(meshes.add(Cylinder::new(0.1, 0.6))),
        MeshMaterial3d(mat.clone()),
        Transform::from_xyz(0.85, 0.5, 0.0),
    ));

    // Small boxes (wheel-like)
    for (x, z) in [(-0.5, 0.3), (0.5, 0.3), (-0.5, -0.3), (0.5, -0.3)] {
        commands.spawn((
            DemoShape,
            Mesh3d(meshes.add(Cuboid::new(0.15, 0.3, 0.15))),
            MeshMaterial3d(mat.clone()),
            Transform::from_xyz(x, 0.15, z),
        ));
    }
}

// =====================================================================
// UI
// =====================================================================

fn ui_panel(
    mut contexts: EguiContexts,
    mut params: ResMut<MaterialParams>,
    mut dirty: ResMut<MaterialDirty>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("material_params").min_width(280.0).show(ctx, |ui| {
        ui.heading("Procedural Material");
        ui.separator();

        // Presets
        ui.label("Presets");
        for (i, name) in PRESET_NAMES.iter().enumerate() {
            if ui.selectable_label(params.active_preset == i, *name).clicked() {
                params.active_preset = i;
                let p = preset_by_index(i);
                params.base_color = [p.base_color.red, p.base_color.green, p.base_color.blue];
                params.noise_scale = p.noise_scale;
                params.noise_strength = p.noise_strength;
                params.color_variation = [p.color_variation.red, p.color_variation.green, p.color_variation.blue];
                params.roughness = p.roughness;
                dirty.0 = true;
            }
        }
        ui.separator();

        // Parameters
        ui.label("Base Color");
        if ui.color_edit_button_rgb(&mut params.base_color).changed() { dirty.0 = true; }

        ui.label("Noise Scale");
        if ui.add(egui::Slider::new(&mut params.noise_scale, 1.0..=40.0)).changed() { dirty.0 = true; }

        ui.label("Noise Strength");
        if ui.add(egui::Slider::new(&mut params.noise_strength, 0.0..=1.0)).changed() { dirty.0 = true; }

        ui.label("Color Variation");
        if ui.color_edit_button_rgb(&mut params.color_variation).changed() { dirty.0 = true; }

        ui.label("Roughness");
        if ui.add(egui::Slider::new(&mut params.roughness, 0.05..=1.0)).changed() { dirty.0 = true; }

        ui.separator();
        ui.small("1-5: preset shortcuts");
        ui.small("Pattern is in world space — seamless across all shapes");
    });
}

// =====================================================================
// Apply material changes
// =====================================================================

fn apply_material(
    mut dirty: ResMut<MaterialDirty>,
    params: Res<MaterialParams>,
    shapes: Query<&MeshMaterial3d<ProceduralMaterial>, With<DemoShape>>,
    mut materials: ResMut<Assets<ProceduralMaterial>>,
) {
    if !dirty.0 { return; }
    dirty.0 = false;

    for mat_handle in &shapes {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = LinearRgba::new(
                params.base_color[0], params.base_color[1], params.base_color[2], 1.0,
            );
            mat.noise_scale = params.noise_scale;
            mat.noise_strength = params.noise_strength;
            mat.color_variation = LinearRgba::new(
                params.color_variation[0], params.color_variation[1], params.color_variation[2], 0.0,
            );
            mat.roughness = params.roughness;
        }
    }
}

// =====================================================================
// Input
// =====================================================================

fn keyboard_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut params: ResMut<MaterialParams>,
    mut dirty: ResMut<MaterialDirty>,
) {
    for (key, idx) in [
        (KeyCode::Digit1, 0),
        (KeyCode::Digit2, 1),
        (KeyCode::Digit3, 2),
        (KeyCode::Digit4, 3),
        (KeyCode::Digit5, 4),
    ] {
        if keys.just_pressed(key) {
            params.active_preset = idx;
            let p = preset_by_index(idx);
            params.base_color = [p.base_color.red, p.base_color.green, p.base_color.blue];
            params.noise_scale = p.noise_scale;
            params.noise_strength = p.noise_strength;
            params.color_variation = [p.color_variation.red, p.color_variation.green, p.color_variation.blue];
            params.roughness = p.roughness;
            dirty.0 = true;
        }
    }
}

// =====================================================================
// Camera
// =====================================================================

fn orbit_camera(
    mut orbit: ResMut<OrbitState>,
    mut camera: Query<(&mut Transform, &Projection), With<MainCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut contexts: EguiContexts,
) {
    let egui_wants = contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_pointer_input());
    let Ok((mut tf, proj)) = camera.single_mut() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };

    if mouse.pressed(MouseButton::Middle) {
        for ev in motion.read() {
            let right = tf.right();
            let up = tf.up();
            orbit.target += (-ev.delta.x * right + ev.delta.y * up) * scale;
        }
    } else if mouse.pressed(MouseButton::Left) && !egui_wants {
        for ev in motion.read() {
            orbit.yaw += ev.delta.x * 0.3;
            orbit.pitch = (orbit.pitch + ev.delta.y * 0.3).clamp(5.0, 85.0);
        }
    } else {
        motion.clear();
    }

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
                    MouseScrollUnit::Line => -ev.y * 0.15,
                    MouseScrollUnit::Pixel => -ev.y * 0.002,
                };
                ortho.scale = (ortho.scale * (1.0 + delta)).clamp(ZOOM_MIN, ZOOM_MAX);
            }
        }
    }
}
