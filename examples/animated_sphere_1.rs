//! Plasma ball: lightning arcs born at the center, flowing outward, dying at the edges.
//! No solid core — the bright center is just dense overlapping arcs near their origin.
//!
//! Run with: `cargo run --example animated_sphere`

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::RngExt;
use std::f32::consts::TAU;

const FLARE_COUNT: usize = 150;
const SEGMENTS_PER_FLARE: usize = 14;
const SPHERE_RADIUS: f32 = 1.5;
const FLARE_ARC_ANGLE: f32 = 0.6; // radians of great-circle sweep
const FLARE_BULGE: f32 = 0.15;
const RIBBON_WIDTH: f32 = 0.025;
const ZIGZAG_AMOUNT: f32 = 0.08;
const NOISE_SPEED: f64 = 2.0;

/// Each flare lives this many seconds from center to edge.
const FLARE_LIFETIME_MIN: f32 = 0.8;
const FLARE_LIFETIME_MAX: f32 = 2.0;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Plasma Ball".into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.01, 0.005, 0.02)))
        .add_systems(Startup, setup)
        .add_systems(Update, (respawn_and_animate_flares, slow_rotate_sphere))
        .run();
}

struct FlareState {
    /// Direction the arc is centered on (unit vector)
    direction: Vec3,
    /// Axis to rotate around for the great-circle arc
    arc_axis: Vec3,
    /// Perpendicular for ribbon width and zigzag
    perp: Vec3,
    /// Noise seed
    seed: f64,
    /// When this flare was born (seconds)
    birth_time: f32,
    /// How long it takes to travel from center to edge
    lifetime: f32,
}

#[derive(Resource)]
struct FlarePool {
    flares: Vec<FlareState>,
}

#[derive(Component)]
struct FlareMesh;

#[derive(Component)]
struct EnergySphere;

fn random_direction(rng: &mut impl rand::Rng) -> Vec3 {
    let theta: f32 = rng.random_range(0.0..TAU);
    let cos_phi: f32 = rng.random_range(-1.0..1.0);
    let sin_phi = (1.0 - cos_phi * cos_phi).sqrt();
    Vec3::new(sin_phi * theta.cos(), sin_phi * theta.sin(), cos_phi)
}

fn new_flare(rng: &mut impl rand::Rng, birth_time: f32) -> FlareState {
    let direction = random_direction(rng);
    let arbitrary = if direction.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let perp = direction.cross(arbitrary).normalize();
    let arc_axis = direction.cross(perp).normalize();
    let seed: f64 = rng.random_range(0.0..1000.0);
    let lifetime: f32 = rng.random_range(FLARE_LIFETIME_MIN..FLARE_LIFETIME_MAX);

    FlareState {
        direction,
        arc_axis,
        perp,
        seed,
        birth_time,
        lifetime,
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::rng();

    // Stagger initial birth times so arcs are at various lifecycle stages
    let flares: Vec<FlareState> = (0..FLARE_COUNT)
        .map(|i| {
            let stagger = -(i as f32 / FLARE_COUNT as f32)
                * rng.random_range(FLARE_LIFETIME_MIN..FLARE_LIFETIME_MAX);
            new_flare(&mut rng, stagger)
        })
        .collect();

    commands.insert_resource(FlarePool { flares });

    // Empty mesh, rebuilt each frame
    let pts = SEGMENTS_PER_FLARE + 1;
    let vert_count = FLARE_COUNT * pts * 2;
    let idx_count = FLARE_COUNT * SEGMENTS_PER_FLARE * 6;

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; vert_count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32; 3]; vert_count]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32; 4]; vert_count]);
    mesh.insert_indices(Indices::U32(vec![0u32; idx_count]));
    let flare_mesh = meshes.add(mesh);

    let flare_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(6.0, 1.0, 12.0, 1.0),
        ..default()
    });

    let sphere_entity = commands
        .spawn((
            EnergySphere,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    let flare_entity = commands
        .spawn((
            FlareMesh,
            Mesh3d(flare_mesh),
            MeshMaterial3d(flare_material),
            Transform::default(),
        ))
        .id();

    commands.entity(sphere_entity).add_child(flare_entity);

    // Point light at center for some surface shading
    commands.spawn((
        PointLight {
            color: Color::srgb(0.6, 0.2, 1.0),
            intensity: 80_000.0,
            range: 10.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::default(),
    ));

    commands.spawn((
        Camera3d::default(),
        Bloom {
            intensity: 0.5,
            low_frequency_boost: 0.8,
            high_pass_frequency: 0.6,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn respawn_and_animate_flares(
    time: Res<Time>,
    mut pool: ResMut<FlarePool>,
    query: Query<&Mesh3d, With<FlareMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let perlin = Perlin::new(0);
    let now = time.elapsed_secs();
    let noise_t = time.elapsed_secs_f64() * NOISE_SPEED;
    let mut rng = rand::rng();

    // Respawn dead flares
    for flare in pool.flares.iter_mut() {
        let age = (now - flare.birth_time) / flare.lifetime;
        if age > 1.0 {
            *flare = new_flare(&mut rng, now);
        }
    }

    let Ok(mesh_handle) = query.single() else {
        return;
    };
    let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
        return;
    };

    let pts_per_flare = SEGMENTS_PER_FLARE + 1;
    let verts_per_flare = pts_per_flare * 2;
    let indices_per_flare = SEGMENTS_PER_FLARE * 6;
    let total_verts = FLARE_COUNT * verts_per_flare;
    let total_indices = FLARE_COUNT * indices_per_flare;

    let mut positions = vec![[0.0f32; 3]; total_verts];
    let mut normals = vec![[0.0f32; 3]; total_verts];
    let mut colors = vec![[0.0f32; 4]; total_verts];
    let mut indices = vec![0u32; total_indices];

    for (fi, flare) in pool.flares.iter().enumerate() {
        let age = ((now - flare.birth_time) / flare.lifetime).clamp(0.0, 1.0);

        // Current radial distance from center: 0 at birth → SPHERE_RADIUS at death
        let radial_pos = age * SPHERE_RADIUS;

        // Brightness fades as arc moves outward
        let brightness = (1.0 - age).powi(2);

        // Width shrinks as it moves outward
        let width = RIBBON_WIDTH * (1.0 + 2.0 * (1.0 - age));

        let v_base = fi * verts_per_flare;
        let i_base = fi * indices_per_flare;
        let s = flare.seed;

        for seg in 0..pts_per_flare {
            let seg_frac = seg as f32 / SEGMENTS_PER_FLARE as f32;

            // Arc sweeps from -half_angle to +half_angle around the direction
            let half = FLARE_ARC_ANGLE * 0.5;
            let arc_angle = -half + seg_frac * FLARE_ARC_ANGLE;
            let rot = Quat::from_axis_angle(flare.arc_axis, arc_angle);
            let dir_on_shell = rot * flare.direction;

            // Bulge: sine hump along arc, peak in the middle
            let hump = (seg_frac * std::f32::consts::PI).sin();
            let bulge_noise = perlin.get([s, seg as f64 * 0.5, noise_t]) as f32;
            let bulge = hump * FLARE_BULGE * (0.6 + 0.4 * bulge_noise);

            // Zigzag perpendicular to arc
            let zigzag_noise = perlin.get([seg as f64 * 0.5, noise_t, s + 500.0]) as f32;
            let zigzag = zigzag_noise * ZIGZAG_AMOUNT;

            let lateral_dir = rot * flare.perp;
            let center_point =
                dir_on_shell * (radial_pos + bulge) + lateral_dir * zigzag;

            // Per-segment brightness: slightly brighter in middle of arc
            let seg_brightness = brightness * (0.5 + 0.5 * hump);

            let ribbon_perp = lateral_dir.normalize();
            let left = center_point + ribbon_perp * width;
            let right = center_point - ribbon_perp * width;

            let vi = v_base + seg * 2;
            positions[vi] = left.into();
            positions[vi + 1] = right.into();
            normals[vi] = dir_on_shell.into();
            normals[vi + 1] = dir_on_shell.into();
            colors[vi] = [seg_brightness, seg_brightness, seg_brightness, 1.0];
            colors[vi + 1] = [seg_brightness, seg_brightness, seg_brightness, 1.0];
        }

        for seg in 0..SEGMENTS_PER_FLARE {
            let vi = (v_base + seg * 2) as u32;
            let ii = i_base + seg * 6;
            indices[ii] = vi;
            indices[ii + 1] = vi + 1;
            indices[ii + 2] = vi + 2;
            indices[ii + 3] = vi + 1;
            indices[ii + 4] = vi + 3;
            indices[ii + 5] = vi + 2;
        }
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

fn slow_rotate_sphere(
    time: Res<Time>,
    mut spheres: Query<&mut Transform, With<EnergySphere>>,
) {
    let dt = time.delta_secs();
    for mut transform in &mut spheres {
        transform.rotate(Quat::from_euler(EulerRot::YXZ, dt * 0.12, dt * 0.07, 0.0));
    }
}
