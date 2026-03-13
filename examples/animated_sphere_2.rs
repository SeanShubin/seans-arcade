//! Plasma circle: arcs of connected points born at center, traveling outward.
//!
//! 2D version — all math is in Vec2, rendered with Camera2d. Each arc is a
//! polyline of points that fly outward from the center and die at the circle edge.
//! Segments are subdivided with midpoint displacement to look like lightning.
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
/// Max subdivision depth (each level doubles the segment count)
const MAX_SUBDIVISIONS: usize = 5;

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
        .add_systems(Update, toggle_pause_keyboard)
        .add_systems(EguiPrimaryContextPass, ui_system)
        .add_systems(Update, (
            spawn_arcs,
            advance_and_cull_arcs,
            build_mesh,
            re_pause_after_step,
        ).chain().run_if(|paused: Res<Paused>| !paused.paused))
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
    /// Recursive midpoint displacement passes (0 = straight lines, 3-4 = good lightning)
    subdivisions: usize,
    /// How far midpoints displace, as fraction of segment length
    jitter: f32,
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
            subdivisions: 3,
            jitter: 5.0,
        }
    }
}

#[derive(Resource, Default)]
struct Paused {
    paused: bool,
    step: bool,
}

fn toggle_pause_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut paused: ResMut<Paused>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        paused.paused = !paused.paused;
    }
}

/// Runs at the end of the simulation chain — re-pauses after a step frame.
fn re_pause_after_step(mut paused: ResMut<Paused>) {
    if paused.step {
        paused.step = false;
        paused.paused = true;
    }
}

// ── HUD ──

fn ui_system(
    mut contexts: EguiContexts,
    mut config: ResMut<Config>,
    mut paused: ResMut<Paused>,
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

                ui.label("Subdivisions");
                let mut sub = config.subdivisions as f64;
                ui.add(egui::DragValue::new(&mut sub).range(0.0..=MAX_SUBDIVISIONS as f64).speed(0.1));
                config.subdivisions = sub as usize;
                ui.end_row();

                ui.label("Jitter");
                ui.add(egui::DragValue::new(&mut config.jitter).range(0.0..=20.0).speed(0.1));
                ui.end_row();
            });

            ui.separator();
            if paused.paused {
                ui.colored_label(egui::Color32::YELLOW, "PAUSED (Space)");
                if ui.button("Step 1 frame").clicked() {
                    paused.paused = false;
                    paused.step = true;
                }
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

    let max_angle = variation * std::f32::consts::FRAC_PI_2;

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

/// Recursively subdivide a segment with midpoint displacement (2D).
/// Pushes subdivided points into `out` (does NOT push `a`, only intermediates and `b`).
fn subdivide_2d(
    a: Vec2,
    b: Vec2,
    depth: usize,
    jitter_frac: f32,
    rng: &mut impl rand::Rng,
    out: &mut Vec<Vec2>,
) {
    if depth == 0 {
        out.push(b);
        return;
    }
    let mid = (a + b) * 0.5;
    let seg = b - a;
    let seg_len = seg.length();
    // Perpendicular in 2D
    let perp = Vec2::new(-seg.y, seg.x).normalize_or(Vec2::Y);
    let offset = perp * rng.random_range(-1.0..1.0_f32) * jitter_frac * seg_len;
    let displaced = mid + offset;

    subdivide_2d(a, displaced, depth - 1, jitter_frac, rng, out);
    subdivide_2d(displaced, b, depth - 1, jitter_frac, rng, out);
}

/// Subdivide an entire polyline into an existing buffer (avoids allocation per arc).
fn subdivide_polyline_into(
    points: &[Vec2],
    depth: usize,
    jitter_frac: f32,
    rng: &mut impl rand::Rng,
    out: &mut Vec<Vec2>,
) {
    if points.len() < 2 {
        out.extend_from_slice(points);
        return;
    }
    if depth == 0 {
        out.extend_from_slice(points);
        return;
    }
    out.push(points[0]);
    for window in points.windows(2) {
        subdivide_2d(window[0], window[1], depth, jitter_frac, rng, out);
    }
}

// ── Systems ──

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 0]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0f32; 4]; 0]);
    mesh.insert_indices(Indices::U32(Vec::new()));
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
    mut scratch: Local<Vec<Vec2>>,
) {
    let Ok(mesh_handle) = query.single() else {
        return;
    };
    let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
        return;
    };

    let ribbon_width = config.ribbon_width;
    let Ok(window) = windows.single() else { return };
    let cr = circle_radius(&config, window);
    let subdivisions = config.subdivisions.min(MAX_SUBDIVISIONS);
    let jitter = config.jitter / 10.0;

    let mut rng = rand::rng();

    // Build only what's needed — push instead of pre-allocating worst case
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for arc in pool.arcs.iter() {
        // Subdivide into reusable scratch buffer
        scratch.clear();
        let base_pts: Vec<Vec2> = arc.points.iter().map(|p| p.position).collect();
        subdivide_polyline_into(&base_pts, subdivisions, jitter, &mut rng, &mut scratch);
        let point_count = scratch.len();
        if point_count < 2 {
            continue;
        }

        let avg_dist = scratch.iter().map(|p| p.length()).sum::<f32>() / point_count as f32;
        let t = (avg_dist / cr).clamp(0.0, 1.0);
        let brightness = (1.0 - t.powi(4)).clamp(0.0, 1.0);
        let color = [brightness, brightness, brightness, 1.0];

        let seg_count = point_count - 1;
        let vert_base = positions.len() as u32;

        for s in 0..seg_count {
            let a = scratch[s];
            let b = scratch[s + 1];

            let tangent = (b - a).normalize_or(Vec2::X);
            let perp = Vec2::new(-tangent.y, tangent.x) * ribbon_width;

            positions.push([(a.x + perp.x), (a.y + perp.y), 0.0]);
            positions.push([(a.x - perp.x), (a.y - perp.y), 0.0]);
            positions.push([(b.x + perp.x), (b.y + perp.y), 0.0]);
            positions.push([(b.x - perp.x), (b.y - perp.y), 0.0]);

            colors.push(color);
            colors.push(color);
            colors.push(color);
            colors.push(color);

            let vi = vert_base + (s as u32) * 4;
            indices.push(vi);
            indices.push(vi + 1);
            indices.push(vi + 2);
            indices.push(vi + 1);
            indices.push(vi + 3);
            indices.push(vi + 2);
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}
