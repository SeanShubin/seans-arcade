//! Interactive biome blender prototype.
//!
//! Generates a multi-biome terrain map using noise layers for elevation,
//! moisture, and drainage. Demonstrates smooth blending between biome
//! regions with configurable transition widths, lighting, and color palettes.
//!
//! Usage:
//!   cargo run --example biome_blender
//!
//! Controls:
//!   Left panel — tweak biome parameters, noise layers, blending, lighting
//!   Scroll wheel — zoom in/out
//!   Middle mouse drag — pan
//!   Arrow keys — pan

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, EguiPlugin, egui};
use noise::{NoiseFn, OpenSimplex};

// =====================================================================
// Biome definitions
// =====================================================================

#[derive(Clone, Copy)]
struct BiomeColors {
    base: [f64; 3],
    detail: [f64; 3], // secondary color for noise variation
}

const DEEP_WATER: BiomeColors = BiomeColors { base: [0.06, 0.12, 0.40], detail: [0.04, 0.10, 0.35] };
const SHALLOW_WATER: BiomeColors = BiomeColors { base: [0.15, 0.30, 0.55], detail: [0.12, 0.28, 0.50] };
const BEACH: BiomeColors = BiomeColors { base: [0.82, 0.76, 0.58], detail: [0.78, 0.72, 0.52] };
const DESERT: BiomeColors = BiomeColors { base: [0.76, 0.68, 0.42], detail: [0.70, 0.62, 0.38] };
const SAVANNA: BiomeColors = BiomeColors { base: [0.60, 0.58, 0.30], detail: [0.55, 0.52, 0.28] };
const GRASSLAND: BiomeColors = BiomeColors { base: [0.30, 0.55, 0.20], detail: [0.25, 0.50, 0.18] };
const FOREST: BiomeColors = BiomeColors { base: [0.15, 0.40, 0.12], detail: [0.10, 0.32, 0.10] };
const DENSE_FOREST: BiomeColors = BiomeColors { base: [0.08, 0.28, 0.08], detail: [0.06, 0.22, 0.06] };
const SWAMP: BiomeColors = BiomeColors { base: [0.20, 0.30, 0.15], detail: [0.18, 0.25, 0.12] };
const MARSH: BiomeColors = BiomeColors { base: [0.25, 0.38, 0.28], detail: [0.22, 0.35, 0.25] };
const ROCK: BiomeColors = BiomeColors { base: [0.42, 0.38, 0.35], detail: [0.38, 0.35, 0.32] };
const SNOW: BiomeColors = BiomeColors { base: [0.90, 0.90, 0.92], detail: [0.85, 0.85, 0.88] };

/// Look up biome from elevation, moisture, drainage (all 0..1).
fn biome_lookup(elevation: f64, moisture: f64, drainage: f64) -> BiomeColors {
    if elevation < 0.30 {
        return DEEP_WATER;
    }
    if elevation < 0.38 {
        return SHALLOW_WATER;
    }
    if elevation < 0.42 {
        return BEACH;
    }
    if elevation > 0.85 {
        return SNOW;
    }
    if elevation > 0.75 {
        return ROCK;
    }

    // Land biomes: moisture x drainage grid
    if moisture < 0.25 {
        // Dry
        if drainage < 0.4 { DESERT } else { SAVANNA }
    } else if moisture < 0.55 {
        // Moderate
        if drainage < 0.35 { MARSH } else { GRASSLAND }
    } else {
        // Wet
        if drainage < 0.35 { SWAMP }
        else if drainage < 0.65 { FOREST }
        else { DENSE_FOREST }
    }
}

// =====================================================================
// Parameters
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Biomes,
    Elevation,
    Moisture,
    Drainage,
    BiomesLit,
    Weights,
}

impl ViewMode {
    const ALL: &[ViewMode] = &[
        ViewMode::BiomesLit,
        ViewMode::Biomes,
        ViewMode::Elevation,
        ViewMode::Moisture,
        ViewMode::Drainage,
        ViewMode::Weights,
    ];

    fn label(self) -> &'static str {
        match self {
            ViewMode::Biomes => "Biomes (flat)",
            ViewMode::Elevation => "Elevation",
            ViewMode::Moisture => "Moisture",
            ViewMode::Drainage => "Drainage",
            ViewMode::BiomesLit => "Biomes (lit)",
            ViewMode::Weights => "Blend Weights",
        }
    }
}

#[derive(Resource, Clone)]
struct BlenderParams {
    view_mode: ViewMode,
    seed: u32,
    // Per-layer frequencies
    elevation_freq: f64,
    moisture_freq: f64,
    drainage_freq: f64,
    // Noise params
    octaves: u32,
    lacunarity: f64,
    gain: f64,
    // Blending
    blend_width: f64,
    blend_samples: u32,
    // Lighting
    light_azimuth: f64,
    light_elevation: f64,
    ambient: f64,
    height_scale: f64,
    // Detail noise
    detail_strength: f64,
    detail_freq: f64,
}

impl Default for BlenderParams {
    fn default() -> Self {
        Self {
            view_mode: ViewMode::BiomesLit,
            seed: 42,
            elevation_freq: 3.0,
            moisture_freq: 4.0,
            drainage_freq: 5.0,
            octaves: 6,
            lacunarity: 2.0,
            gain: 0.5,
            blend_width: 0.06,
            blend_samples: 8,
            light_azimuth: 225.0,
            light_elevation: 45.0,
            ambient: 0.15,
            height_scale: 1.0,
            detail_strength: 0.3,
            detail_freq: 20.0,
        }
    }
}

#[derive(Resource)]
struct RenderDirty(bool);

#[derive(Component)]
struct DisplaySprite;

const TEX_W: u32 = 512;
const TEX_H: u32 = 512;

// =====================================================================
// Entry point
// =====================================================================

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Biome Blender".into(),
                    resolution: bevy::window::WindowResolution::new(1100, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(BlenderParams::default())
        .insert_resource(RenderDirty(true))
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(Update, (regenerate_texture, camera_zoom, camera_pan))
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

    let image = Image::new_fill(
        Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    let handle = images.add(image);

    commands.spawn((
        Sprite {
            image: handle,
            custom_size: Some(Vec2::new(TEX_W as f32, TEX_H as f32)),
            ..default()
        },
        DisplaySprite,
    ));
}

// =====================================================================
// UI
// =====================================================================

fn ui_panel(mut contexts: EguiContexts, mut params: ResMut<BlenderParams>, mut dirty: ResMut<RenderDirty>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("blender_params").min_width(280.0).show(ctx, |ui| {
        ui.heading("Biome Blender");
        ui.separator();

        ui.label("View Mode");
        for &vm in ViewMode::ALL {
            if ui.radio_value(&mut params.view_mode, vm, vm.label()).changed() { dirty.0 = true; }
        }
        ui.separator();

        ui.label("Seed");
        if ui.add(egui::DragValue::new(&mut params.seed).range(0..=9999)).changed() { dirty.0 = true; }

        ui.separator();
        ui.heading("Noise Layers");

        ui.label("Elevation Frequency");
        if ui.add(egui::Slider::new(&mut params.elevation_freq, 0.5..=20.0).logarithmic(true)).changed() { dirty.0 = true; }

        ui.label("Moisture Frequency");
        if ui.add(egui::Slider::new(&mut params.moisture_freq, 0.5..=20.0).logarithmic(true)).changed() { dirty.0 = true; }

        ui.label("Drainage Frequency");
        if ui.add(egui::Slider::new(&mut params.drainage_freq, 0.5..=20.0).logarithmic(true)).changed() { dirty.0 = true; }

        ui.label("Octaves");
        if ui.add(egui::Slider::new(&mut params.octaves, 1..=10)).changed() { dirty.0 = true; }

        ui.label("Lacunarity");
        if ui.add(egui::Slider::new(&mut params.lacunarity, 1.0..=4.0)).changed() { dirty.0 = true; }

        ui.label("Gain");
        if ui.add(egui::Slider::new(&mut params.gain, 0.1..=0.9)).changed() { dirty.0 = true; }

        ui.separator();
        ui.heading("Blending");

        ui.label("Blend Width");
        if ui.add(egui::Slider::new(&mut params.blend_width, 0.0..=0.2)).changed() { dirty.0 = true; }

        ui.label("Blend Samples");
        let mut s = params.blend_samples as i32;
        if ui.add(egui::Slider::new(&mut s, 1..=16)).changed() {
            params.blend_samples = s as u32;
            dirty.0 = true;
        }

        ui.separator();
        ui.heading("Detail Noise");

        ui.label("Detail Strength");
        if ui.add(egui::Slider::new(&mut params.detail_strength, 0.0..=1.0)).changed() { dirty.0 = true; }

        ui.label("Detail Frequency");
        if ui.add(egui::Slider::new(&mut params.detail_freq, 5.0..=60.0)).changed() { dirty.0 = true; }

        if matches!(params.view_mode, ViewMode::BiomesLit) {
            ui.separator();
            ui.heading("Lighting");

            ui.label("Light Azimuth");
            if ui.add(egui::Slider::new(&mut params.light_azimuth, 0.0..=360.0).suffix("°")).changed() { dirty.0 = true; }

            ui.label("Light Elevation");
            if ui.add(egui::Slider::new(&mut params.light_elevation, 5.0..=85.0).suffix("°")).changed() { dirty.0 = true; }

            ui.label("Ambient");
            if ui.add(egui::Slider::new(&mut params.ambient, 0.0..=0.5)).changed() { dirty.0 = true; }

            ui.label("Height Scale");
            if ui.add(egui::Slider::new(&mut params.height_scale, 0.1..=5.0)).changed() { dirty.0 = true; }
        }
    });
}

// =====================================================================
// Texture regeneration
// =====================================================================

fn regenerate_texture(
    mut dirty: ResMut<RenderDirty>,
    params: Res<BlenderParams>,
    mut images: ResMut<Assets<Image>>,
    sprites: Query<&Sprite, With<DisplaySprite>>,
) {
    if !dirty.0 { return; }
    dirty.0 = false;

    let Ok(sprite) = sprites.single() else { return };
    let Some(image) = images.get_mut(&sprite.image) else { return };

    // Create noise sources with offset seeds for independence
    let elev_noise = OpenSimplex::new(params.seed);
    let moist_noise = OpenSimplex::new(params.seed.wrapping_add(137));
    let drain_noise = OpenSimplex::new(params.seed.wrapping_add(293));
    let detail_noise = OpenSimplex::new(params.seed.wrapping_add(500));

    // Generate fields
    let size = (TEX_W * TEX_H) as usize;
    let mut elevation = vec![0.0f64; size];
    let mut moisture = vec![0.0f64; size];
    let mut drainage = vec![0.0f64; size];

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let idx = (py * TEX_W + px) as usize;
            let ux = px as f64 / TEX_W as f64;
            let uy = py as f64 / TEX_H as f64;

            elevation[idx] = fbm(&elev_noise, ux * params.elevation_freq, uy * params.elevation_freq,
                params.octaves, params.lacunarity, params.gain) * 0.5 + 0.5;
            moisture[idx] = fbm(&moist_noise, ux * params.moisture_freq, uy * params.moisture_freq,
                params.octaves, params.lacunarity, params.gain) * 0.5 + 0.5;
            drainage[idx] = fbm(&drain_noise, ux * params.drainage_freq, uy * params.drainage_freq,
                params.octaves, params.lacunarity, params.gain) * 0.5 + 0.5;
        }
    }

    // Compute normals from elevation for lit mode
    let normals = if matches!(params.view_mode, ViewMode::BiomesLit) {
        Some(compute_normals(&elevation, params.height_scale))
    } else {
        None
    };

    let light = light_direction(params.light_azimuth, params.light_elevation);

    let mut pixels = vec![0u8; size * 4];

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let idx = (py * TEX_W + px) as usize;
            let pidx = idx * 4;

            let e = elevation[idx];
            let m = moisture[idx];
            let d = drainage[idx];

            let ux = px as f64 / TEX_W as f64;
            let uy = py as f64 / TEX_H as f64;

            let [r, g, b] = match params.view_mode {
                ViewMode::Elevation => {
                    let v = (e * 255.0) as u8;
                    [v, v, v]
                }
                ViewMode::Moisture => {
                    let v = (m * 255.0) as u8;
                    [0, 0, v]
                }
                ViewMode::Drainage => {
                    let v = (d * 255.0) as u8;
                    [0, v, 0]
                }
                ViewMode::Weights => {
                    // Show the three axes as RGB
                    [(e * 255.0) as u8, (m * 255.0) as u8, (d * 255.0) as u8]
                }
                ViewMode::Biomes | ViewMode::BiomesLit => {
                    let color = blended_biome_color(
                        e, m, d, ux, uy,
                        &detail_noise, &params,
                    );

                    if let (ViewMode::BiomesLit, Some(normals)) = (params.view_mode, &normals) {
                        let normal = normals[idx];
                        let brightness = shade(normal, light, params.ambient);
                        [
                            (color[0] * brightness * 255.0).min(255.0) as u8,
                            (color[1] * brightness * 255.0).min(255.0) as u8,
                            (color[2] * brightness * 255.0).min(255.0) as u8,
                        ]
                    } else {
                        [
                            (color[0] * 255.0) as u8,
                            (color[1] * 255.0) as u8,
                            (color[2] * 255.0) as u8,
                        ]
                    }
                }
            };

            pixels[pidx] = r;
            pixels[pidx + 1] = g;
            pixels[pidx + 2] = b;
            pixels[pidx + 3] = 255;
        }
    }

    image.data = Some(pixels);
}

/// Sample biome color with stochastic blending at biome boundaries.
fn blended_biome_color(
    e: f64, m: f64, d: f64,
    ux: f64, uy: f64,
    detail_noise: &OpenSimplex,
    params: &BlenderParams,
) -> [f64; 3] {
    let detail = detail_noise.get([ux * params.detail_freq, uy * params.detail_freq]) * params.detail_strength;

    if params.blend_width <= 0.001 || params.blend_samples <= 1 {
        // No blending — hard biome boundaries
        let biome = biome_lookup(e, m, d);
        return lerp3(biome.base, biome.detail, detail * 0.5 + 0.5);
    }

    // Sample nearby parameter values and average the biome colors.
    // This creates smooth transitions at biome boundaries by sampling
    // the biome function at jittered offsets within the blend radius.
    let mut accum = [0.0f64; 3];
    let n = params.blend_samples;
    let bw = params.blend_width;

    for i in 0..n {
        let angle = (i as f64 / n as f64) * std::f64::consts::TAU;
        let r = bw * ((i as f64 * 0.618034).fract()); // golden ratio jitter
        let oe = (e + angle.cos() * r).clamp(0.0, 1.0);
        let om = (m + angle.sin() * r).clamp(0.0, 1.0);
        let od = (d + (angle + 1.0).cos() * r * 0.7).clamp(0.0, 1.0);

        let biome = biome_lookup(oe, om, od);
        let c = lerp3(biome.base, biome.detail, detail * 0.5 + 0.5);
        accum[0] += c[0];
        accum[1] += c[1];
        accum[2] += c[2];
    }

    let inv = 1.0 / n as f64;
    [accum[0] * inv, accum[1] * inv, accum[2] * inv]
}

// =====================================================================
// Normal computation (from elevation)
// =====================================================================

fn compute_normals(heights: &[f64], height_scale: f64) -> Vec<[f64; 3]> {
    let mut normals = vec![[0.0, 0.0, 1.0]; (TEX_W * TEX_H) as usize];
    let step = 1.0 / TEX_W as f64;

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let idx = (py * TEX_W + px) as usize;

            let left = if px > 0 { heights[idx - 1] } else { heights[idx] };
            let right = if px < TEX_W - 1 { heights[idx + 1] } else { heights[idx] };
            let up = if py > 0 { heights[idx - TEX_W as usize] } else { heights[idx] };
            let down = if py < TEX_H - 1 { heights[idx + TEX_W as usize] } else { heights[idx] };

            let dhdx = (right - left) / (2.0 * step) * height_scale;
            let dhdy = (up - down) / (2.0 * step) * height_scale;

            let nx = -dhdx;
            let ny = -dhdy;
            let nz = 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            normals[idx] = [nx / len, ny / len, nz / len];
        }
    }
    normals
}

// =====================================================================
// Lighting
// =====================================================================

fn light_direction(azimuth_deg: f64, elevation_deg: f64) -> [f64; 3] {
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();
    [az.cos() * el.cos(), az.sin() * el.cos(), el.sin()]
}

fn shade(normal: [f64; 3], light: [f64; 3], ambient: f64) -> f64 {
    let ndotl = normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2];
    (ambient + ndotl.max(0.0)).min(1.0)
}

// =====================================================================
// Noise
// =====================================================================

fn fbm<F: NoiseFn<f64, 2>>(noise: &F, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amp = 0.0;
    for _ in 0..octaves {
        value += noise.get([x * frequency, y * frequency]) * amplitude;
        max_amp += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    value / max_amp
}

// =====================================================================
// Utilities
// =====================================================================

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

// =====================================================================
// Camera controls
// =====================================================================

fn camera_zoom(
    mut query: Query<&mut Projection, With<Camera2d>>,
    mut scroll: MessageReader<MouseWheel>,
) {
    for ev in scroll.read() {
        for mut projection in &mut query {
            if let Projection::Orthographic(ortho) = projection.as_mut() {
                let factor = 1.0 - ev.y * 0.1;
                ortho.scale = (ortho.scale * factor).clamp(0.1, 10.0);
            }
        }
    }
}

fn camera_pan(
    mut query: Query<(&mut Transform, &Projection), With<Camera2d>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    time: Res<Time>,
) {
    let Ok((mut transform, projection)) = query.single_mut() else { return };
    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };

    if mouse.pressed(MouseButton::Middle) {
        for ev in motion.read() {
            transform.translation.x -= ev.delta.x * scale;
            transform.translation.y += ev.delta.y * scale;
        }
    } else {
        motion.clear();
    }

    let speed = 200.0 * scale * time.delta_secs();
    if keys.pressed(KeyCode::ArrowLeft) { transform.translation.x -= speed; }
    if keys.pressed(KeyCode::ArrowRight) { transform.translation.x += speed; }
    if keys.pressed(KeyCode::ArrowUp) { transform.translation.y += speed; }
    if keys.pressed(KeyCode::ArrowDown) { transform.translation.y -= speed; }
}
