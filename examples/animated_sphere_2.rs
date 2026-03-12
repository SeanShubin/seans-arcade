//! Plasma circle: arcs of connected points born at center, traveling outward.
//!
//! 2D version — all math is in Vec2, rendered with Camera2d and Sprite-based
//! mesh quads. Each arc is a polyline of points that fly outward from the center
//! and die when they pass the circle radius.
//!
//! Run with: `cargo run --example animated_sphere_2`

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};
use rand::RngExt;
use std::f32::consts::TAU;

/// Max simultaneous arcs (pool size for mesh allocation).
const MAX_ARCS: usize = 512;
const MAX_POINTS: usize = 20;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Plasma Circle".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.01, 0.005, 0.02)))
        .init_resource::<Paused>()
        .init_resource::<Config>()
        .init_resource::<ArcPool>()
        .add_systems(Startup, setup)
        .add_systems(Update, toggle_pause)
        .add_systems(EguiPrimaryContextPass, ui_system)
        .add_systems(Update, (spawn_arcs, advance_and_cull_arcs, build_mesh)
            .chain()
            .run_if(|paused: Res<Paused>| !paused.0))
        .run();
}

// ── Config ──

#[derive(Resource)]
struct Config {
    variation: f32,
    arc_spawn_rate: f32,
    arc_points_min: usize,
    arc_points_max: usize,
    speed_base: f32,
    speed_variation: f32,
    /// Circle radius as a fraction of half the window's smaller dimension (0.0–1.0)
    circle_radius_frac: f32,
    ribbon_width: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            variation: 0.4,
            arc_spawn_rate: 80.0,
            arc_points_min: 5,
            arc_points_max: 10,
            speed_base: 200.0,
            speed_variation: 60.0,
            circle_radius_frac: 0.8,
            ribbon_width: 1.0,
        }
    }
}

#[derive(Resource, Default)]
struct Paused(bool);

fn toggle_pause(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        paused.0 = !paused.0;
    }
}

// ── HUD ──

fn ui_system(
    mut contexts: EguiContexts,
    mut config: ResMut<Config>,
    paused: Res<Paused>,
    mut pool: ResMut<ArcPool>,
    time: Res<Time>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::Window::new("Controls")
        .anchor(egui::Align2::RIGHT_TOP, [-8.0, 8.0])
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("sliders").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
                ui.label("Variation");
                ui.add(egui::DragValue::new(&mut config.variation).range(0.0..=1.0).speed(0.01));
                ui.end_row();

                ui.label("Spawn rate");
                ui.add(egui::DragValue::new(&mut config.arc_spawn_rate).range(1.0..=500.0).speed(1.0));
                ui.end_row();

                ui.label("Points min");
                let mut min = config.arc_points_min as f64;
                ui.add(egui::DragValue::new(&mut min).range(2.0..=config.arc_points_max as f64).speed(0.1));
                config.arc_points_min = min as usize;
                ui.end_row();

                ui.label("Points max");
                let mut max = config.arc_points_max as f64;
                ui.add(egui::DragValue::new(&mut max).range(config.arc_points_min as f64..=MAX_POINTS as f64).speed(0.1));
                config.arc_points_max = max as usize;
                ui.end_row();

                ui.label("Speed base");
                ui.add(egui::DragValue::new(&mut config.speed_base).range(1.0..=1000.0).speed(5.0));
                ui.end_row();

                ui.label("Speed variation");
                ui.add(egui::DragValue::new(&mut config.speed_variation).range(0.0..=500.0).speed(5.0));
                ui.end_row();

                ui.label("Circle radius %");
                ui.add(egui::DragValue::new(&mut config.circle_radius_frac).range(0.05..=1.0).speed(0.01));
                ui.end_row();

                ui.label("Ribbon width");
                ui.add(egui::DragValue::new(&mut config.ribbon_width).range(0.1..=20.0).speed(0.1));
                ui.end_row();
            });

            ui.separator();
            if paused.0 {
                ui.colored_label(egui::Color32::YELLOW, "PAUSED (Space)");
            } else {
                ui.label("Space to pause");
            }
            ui.label(format!("Active arcs: {}", pool.arcs.len()));
            if ui.button("Clear arcs").clicked() {
                pool.arcs.clear();
            }

            ui.separator();
            let dt = time.delta_secs();
            let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
            let color = if fps >= 55.0 {
                egui::Color32::GREEN
            } else if fps >= 30.0 {
                egui::Color32::YELLOW
            } else {
                egui::Color32::RED
            };
            ui.colored_label(color, format!("FPS: {fps:.0}"));
            ui.label(format!("Frame: {:.1} ms", dt * 1000.0));
        });
}

// ── Data ──

struct ArcPoint {
    position: Vec2,
    velocity: Vec2,
}

struct Arc {
    points: Vec<ArcPoint>,
}

#[derive(Resource, Default)]
struct ArcPool {
    arcs: Vec<Arc>,
    spawn_accumulator: f32,
}

#[derive(Component)]
struct ArcMesh;

// ── Helpers ──

fn random_direction_2d(rng: &mut impl rand::Rng) -> Vec2 {
    let angle: f32 = rng.random_range(0.0..TAU);
    Vec2::new(angle.cos(), angle.sin())
}

fn new_arc(rng: &mut impl rand::Rng, config: &Config) -> Arc {
    let count = rng.random_range(config.arc_points_min..=config.arc_points_max);
    let variation = config.variation;

    let shared_dir = random_direction_2d(rng);
    let shared_speed = config.speed_base
        + rng.random_range(-config.speed_variation..config.speed_variation) * (1.0 - variation);

    // Max angular spread in radians, controlled by variation (0 = no spread, 1 = up to ±90°)
    let max_angle = variation * std::f32::consts::FRAC_PI_2;

    // Pre-generate per-point random angle offsets, then sort them so point order
    // matches spatial order (leftmost deviation → rightmost deviation)
    let mut angle_offsets: Vec<f32> = (0..count)
        .map(|_| rng.random_range(-max_angle..max_angle))
        .collect();
    angle_offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let speed_offsets: Vec<f32> = (0..count)
        .map(|_| {
            let point_speed = config.speed_base
                + rng.random_range(-config.speed_variation..config.speed_variation);
            shared_speed * (1.0 - variation) + point_speed * variation
        })
        .collect();

    let points = angle_offsets
        .iter()
        .enumerate()
        .map(|(i, &angle)| {
            // Rotate shared_dir by the sorted angle offset
            let (sin, cos) = angle.sin_cos();
            let dir = Vec2::new(
                shared_dir.x * cos - shared_dir.y * sin,
                shared_dir.x * sin + shared_dir.y * cos,
            );
            let speed = speed_offsets[i];

            ArcPoint {
                position: Vec2::ZERO,
                velocity: dir * speed,
            }
        })
        .collect();

    Arc { points }
}

// ── Systems ──

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    // Pre-allocate mesh
    let max_segments_per_arc = MAX_POINTS - 1;
    let max_verts_per_arc = max_segments_per_arc * 4;
    let max_indices_per_arc = max_segments_per_arc * 6;
    let total_verts = MAX_ARCS * max_verts_per_arc;
    let total_indices = MAX_ARCS * max_indices_per_arc;

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; total_verts]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; total_verts]);
    mesh.insert_indices(Indices::U32(vec![0u32; total_indices]));
    let mesh_handle = meshes.add(mesh);

    let material = materials.add(ColorMaterial {
        color: Color::srgb(0.8, 0.4, 1.0),
        ..default()
    });

    commands.spawn((
        ArcMesh,
        Mesh2d(mesh_handle),
        MeshMaterial2d(material),
    ));
}

fn spawn_arcs(time: Res<Time>, config: Res<Config>, mut pool: ResMut<ArcPool>) {
    let mut rng = rand::rng();
    pool.spawn_accumulator += time.delta_secs() * config.arc_spawn_rate;

    while pool.spawn_accumulator >= 1.0 && pool.arcs.len() < MAX_ARCS {
        pool.spawn_accumulator -= 1.0;
        pool.arcs.push(new_arc(&mut rng, &config));
    }
    if pool.arcs.len() >= MAX_ARCS {
        pool.spawn_accumulator = 0.0;
    }
}

fn circle_radius(config: &Config, window: &Window) -> f32 {
    let half_min = window.width().min(window.height()) / 2.0;
    half_min * config.circle_radius_frac
}

fn advance_and_cull_arcs(
    time: Res<Time>,
    config: Res<Config>,
    mut pool: ResMut<ArcPool>,
    windows: Query<&Window>,
) {
    let dt = time.delta_secs();

    for arc in pool.arcs.iter_mut() {
        for point in arc.points.iter_mut() {
            point.position += point.velocity * dt;
        }
    }

    let Ok(window) = windows.single() else { return };
    let radius = circle_radius(&config, window);
    pool.arcs.retain(|arc| {
        let com: Vec2 =
            arc.points.iter().map(|p| p.position).sum::<Vec2>() / arc.points.len() as f32;
        com.length() < radius
    });
}

fn build_mesh(
    pool: Res<ArcPool>,
    config: Res<Config>,
    query: Query<&Mesh2d, With<ArcMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    windows: Query<&Window>,
) {
    let Ok(mesh_handle) = query.single() else {
        return;
    };
    let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
        return;
    };

    let max_segments_per_arc = MAX_POINTS - 1;
    let max_verts_per_arc = max_segments_per_arc * 4;
    let max_indices_per_arc = max_segments_per_arc * 6;
    let total_verts = MAX_ARCS * max_verts_per_arc;
    let total_indices = MAX_ARCS * max_indices_per_arc;

    let mut positions = vec![[0.0f32; 3]; total_verts];
    let mut colors = vec![[0.0f32; 4]; total_verts];
    let mut indices = vec![0u32; total_indices];

    let ribbon_width = config.ribbon_width;
    let Ok(window) = windows.single() else { return };
    let cr = circle_radius(&config, window);

    for (ai, arc) in pool.arcs.iter().enumerate() {
        let v_base = ai * max_verts_per_arc;
        let i_base = ai * max_indices_per_arc;
        let point_count = arc.points.len().min(MAX_POINTS);

        let avg_dist = arc.points.iter().map(|p| p.position.length()).sum::<f32>() / point_count as f32;
        // Stay bright until close to the edge, then drop sharply
        let t = (avg_dist / cr).clamp(0.0, 1.0);
        let brightness = (1.0 - t.powi(4)).clamp(0.0, 1.0);

        let seg_count = if point_count > 1 { point_count - 1 } else { 0 };

        for s in 0..seg_count {
            let a = arc.points[s].position;
            let b = arc.points[s + 1].position;

            // Tangent along segment, perpendicular is 90° rotation in 2D
            let tangent = (b - a).normalize_or(Vec2::X);
            let perp = Vec2::new(-tangent.y, tangent.x);

            let vi = v_base + s * 4;
            // z=0 for all 2D verts
            positions[vi]     = [(a + perp * ribbon_width).x, (a + perp * ribbon_width).y, 0.0];
            positions[vi + 1] = [(a - perp * ribbon_width).x, (a - perp * ribbon_width).y, 0.0];
            positions[vi + 2] = [(b + perp * ribbon_width).x, (b + perp * ribbon_width).y, 0.0];
            positions[vi + 3] = [(b - perp * ribbon_width).x, (b - perp * ribbon_width).y, 0.0];

            colors[vi]     = [brightness, brightness, brightness, 1.0];
            colors[vi + 1] = [brightness, brightness, brightness, 1.0];
            colors[vi + 2] = [brightness, brightness, brightness, 1.0];
            colors[vi + 3] = [brightness, brightness, brightness, 1.0];

            let ii = i_base + s * 6;
            let base = vi as u32;
            indices[ii]     = base;
            indices[ii + 1] = base + 1;
            indices[ii + 2] = base + 2;
            indices[ii + 3] = base + 1;
            indices[ii + 4] = base + 3;
            indices[ii + 5] = base + 2;
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}
