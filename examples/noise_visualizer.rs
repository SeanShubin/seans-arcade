//! Interactive noise visualizer for exploring procedural generation primitives.
//!
//! Renders noise functions to a full-screen texture with real-time parameter
//! tweaking via egui. Supports all core noise types needed for terrain and
//! biome generation.
//!
//! Usage:
//!   cargo run --example noise_visualizer
//!
//! Controls:
//!   Left panel — select noise type and tweak parameters
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
use noise::{NoiseFn, OpenSimplex, Perlin};

// =====================================================================
// Noise types
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NoiseType {
    Simplex,
    Perlin,
    Fbm,
    RidgedMulti,
    Turbulence,
    DomainWarp,
    Voronoi,
    Marble,
    Combined,
}

impl NoiseType {
    const ALL: &[NoiseType] = &[
        NoiseType::Simplex,
        NoiseType::Perlin,
        NoiseType::Fbm,
        NoiseType::RidgedMulti,
        NoiseType::Turbulence,
        NoiseType::DomainWarp,
        NoiseType::Voronoi,
        NoiseType::Marble,
        NoiseType::Combined,
    ];

    fn label(self) -> &'static str {
        match self {
            NoiseType::Simplex => "Simplex",
            NoiseType::Perlin => "Perlin",
            NoiseType::Fbm => "fBM (Fractal Brownian Motion)",
            NoiseType::RidgedMulti => "Ridged Multifractal",
            NoiseType::Turbulence => "Turbulence",
            NoiseType::DomainWarp => "Domain Warp",
            NoiseType::Voronoi => "Voronoi / Worley",
            NoiseType::Marble => "Marble (Sine + Warp)",
            NoiseType::Combined => "Combined (Biome-like)",
        }
    }
}

// =====================================================================
// Parameters
// =====================================================================

#[derive(Resource, Clone)]
struct NoiseParams {
    noise_type: NoiseType,
    seed: u32,
    frequency: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f64,
    warp_strength: f64,
    voronoi_metric: VoronoiMetric,
    voronoi_output: VoronoiOutput,
    color_mode: ColorMode,
    // Combined mode weights
    height_freq: f64,
    moisture_freq: f64,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            noise_type: NoiseType::Fbm,
            seed: 0,
            frequency: 4.0,
            octaves: 6,
            lacunarity: 2.0,
            gain: 0.5,
            warp_strength: 4.0,
            voronoi_metric: VoronoiMetric::Euclidean,
            voronoi_output: VoronoiOutput::F1,
            color_mode: ColorMode::Grayscale,
            height_freq: 3.0,
            moisture_freq: 5.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VoronoiMetric {
    Euclidean,
    Manhattan,
    Chebyshev,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VoronoiOutput {
    F1,
    F2,
    F2MinusF1,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Grayscale,
    Terrain,
    HeatMap,
}

#[derive(Resource)]
struct NoiseDirty(bool);

#[derive(Component)]
struct NoiseSprite;

// =====================================================================
// Texture dimensions
// =====================================================================

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
                    title: "Noise Visualizer".into(),
                    resolution: bevy::window::WindowResolution::new(1024, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(NoiseParams::default())
        .insert_resource(NoiseDirty(true))
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(Update, (regenerate_texture, camera_zoom, camera_pan))
        .run();
}

fn setup(mut commands: Commands, images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);
    spawn_noise_sprite(commands, images);
}

fn spawn_noise_sprite(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = Image::new_fill(
        Extent3d {
            width: TEX_W,
            height: TEX_H,
            depth_or_array_layers: 1,
        },
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
        NoiseSprite,
    ));
}

// =====================================================================
// UI
// =====================================================================

fn ui_panel(mut contexts: EguiContexts, mut params: ResMut<NoiseParams>, mut dirty: ResMut<NoiseDirty>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    egui::SidePanel::left("noise_params").min_width(280.0).show(ctx, |ui| {
        ui.heading("Noise Visualizer");
        ui.separator();

        // Noise type selector
        ui.label("Noise Type");
        let prev_type = params.noise_type;
        for &nt in NoiseType::ALL {
            if ui.radio_value(&mut params.noise_type, nt, nt.label()).changed() {
                dirty.0 = true;
            }
        }
        if params.noise_type != prev_type {
            dirty.0 = true;
        }
        ui.separator();

        // Common parameters
        ui.label("Seed");
        if ui.add(egui::DragValue::new(&mut params.seed).range(0..=9999)).changed() {
            dirty.0 = true;
        }

        ui.label("Frequency");
        if ui.add(egui::Slider::new(&mut params.frequency, 0.5..=50.0).logarithmic(true)).changed() {
            dirty.0 = true;
        }

        // Octave-based params
        let uses_octaves = matches!(
            params.noise_type,
            NoiseType::Fbm | NoiseType::RidgedMulti | NoiseType::Turbulence | NoiseType::DomainWarp | NoiseType::Marble | NoiseType::Combined
        );
        if uses_octaves {
            ui.separator();
            ui.label("Octaves");
            if ui.add(egui::Slider::new(&mut params.octaves, 1..=10)).changed() {
                dirty.0 = true;
            }
            ui.label("Lacunarity");
            if ui.add(egui::Slider::new(&mut params.lacunarity, 1.0..=4.0)).changed() {
                dirty.0 = true;
            }
            ui.label("Gain (Persistence)");
            if ui.add(egui::Slider::new(&mut params.gain, 0.1..=0.9)).changed() {
                dirty.0 = true;
            }
        }

        // Warp strength
        if matches!(params.noise_type, NoiseType::DomainWarp | NoiseType::Marble) {
            ui.separator();
            ui.label("Warp Strength");
            if ui.add(egui::Slider::new(&mut params.warp_strength, 0.1..=10.0)).changed() {
                dirty.0 = true;
            }
        }

        // Voronoi params
        if params.noise_type == NoiseType::Voronoi {
            ui.separator();
            ui.label("Distance Metric");
            if ui.radio_value(&mut params.voronoi_metric, VoronoiMetric::Euclidean, "Euclidean").changed() { dirty.0 = true; }
            if ui.radio_value(&mut params.voronoi_metric, VoronoiMetric::Manhattan, "Manhattan").changed() { dirty.0 = true; }
            if ui.radio_value(&mut params.voronoi_metric, VoronoiMetric::Chebyshev, "Chebyshev").changed() { dirty.0 = true; }

            ui.separator();
            ui.label("Output");
            if ui.radio_value(&mut params.voronoi_output, VoronoiOutput::F1, "F1 (nearest)").changed() { dirty.0 = true; }
            if ui.radio_value(&mut params.voronoi_output, VoronoiOutput::F2, "F2 (second nearest)").changed() { dirty.0 = true; }
            if ui.radio_value(&mut params.voronoi_output, VoronoiOutput::F2MinusF1, "F2 - F1 (cell edges)").changed() { dirty.0 = true; }
        }

        // Combined mode params
        if params.noise_type == NoiseType::Combined {
            ui.separator();
            ui.label("Height Frequency");
            if ui.add(egui::Slider::new(&mut params.height_freq, 0.5..=20.0).logarithmic(true)).changed() { dirty.0 = true; }
            ui.label("Moisture Frequency");
            if ui.add(egui::Slider::new(&mut params.moisture_freq, 0.5..=20.0).logarithmic(true)).changed() { dirty.0 = true; }
        }

        // Color mode
        ui.separator();
        ui.label("Color Mode");
        if ui.radio_value(&mut params.color_mode, ColorMode::Grayscale, "Grayscale").changed() { dirty.0 = true; }
        if ui.radio_value(&mut params.color_mode, ColorMode::Terrain, "Terrain").changed() { dirty.0 = true; }
        if ui.radio_value(&mut params.color_mode, ColorMode::HeatMap, "Heat Map").changed() { dirty.0 = true; }
    });
}

// =====================================================================
// Texture regeneration
// =====================================================================

fn regenerate_texture(
    mut dirty: ResMut<NoiseDirty>,
    params: Res<NoiseParams>,
    mut images: ResMut<Assets<Image>>,
    sprites: Query<&Sprite, With<NoiseSprite>>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let Ok(sprite) = sprites.single() else { return };
    let Some(image) = images.get_mut(&sprite.image) else { return };

    let perlin = Perlin::new(params.seed);
    let simplex = OpenSimplex::new(params.seed);
    let perlin_b = Perlin::new(params.seed.wrapping_add(137));
    let simplex_b = OpenSimplex::new(params.seed.wrapping_add(293));

    let num_pixels = (TEX_W * TEX_H * 4) as usize;
    let mut pixels = vec![0u8; num_pixels];

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let nx = px as f64 / TEX_W as f64 * params.frequency;
            let ny = py as f64 / TEX_H as f64 * params.frequency;

            let idx = ((py * TEX_W + px) * 4) as usize;

            if params.noise_type == NoiseType::Combined {
                let height = fbm(&simplex, nx / params.frequency * params.height_freq, ny / params.frequency * params.height_freq, params.octaves, params.lacunarity, params.gain);
                let moisture = fbm(&perlin, nx / params.frequency * params.moisture_freq, ny / params.frequency * params.moisture_freq, params.octaves, params.lacunarity, params.gain);
                let h = (height * 0.5 + 0.5).clamp(0.0, 1.0);
                let m = (moisture * 0.5 + 0.5).clamp(0.0, 1.0);
                let [r, g, b] = biome_color(h, m);
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255;
            } else {
                let value = match params.noise_type {
                    NoiseType::Simplex => simplex.get([nx, ny]),
                    NoiseType::Perlin => perlin.get([nx, ny]),
                    NoiseType::Fbm => fbm(&simplex, nx, ny, params.octaves, params.lacunarity, params.gain),
                    NoiseType::RidgedMulti => ridged_multi(&simplex, nx, ny, params.octaves, params.lacunarity, params.gain),
                    NoiseType::Turbulence => turbulence(&simplex, nx, ny, params.octaves, params.lacunarity, params.gain),
                    NoiseType::DomainWarp => domain_warp(&simplex, &simplex_b, nx, ny, params.octaves, params.lacunarity, params.gain, params.warp_strength),
                    NoiseType::Voronoi => voronoi(nx, ny, params.seed, params.voronoi_metric, params.voronoi_output),
                    NoiseType::Marble => marble(&perlin, &perlin_b, nx, ny, params.octaves, params.lacunarity, params.gain, params.warp_strength),
                    NoiseType::Combined => unreachable!(),
                };
                let normalized = (value * 0.5 + 0.5).clamp(0.0, 1.0);
                let [r, g, b] = colorize(normalized, params.color_mode);
                pixels[idx] = r;
                pixels[idx + 1] = g;
                pixels[idx + 2] = b;
                pixels[idx + 3] = 255;
            }
        }
    }

    image.data = Some(pixels);
}

// =====================================================================
// Noise functions
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

fn ridged_multi<F: NoiseFn<f64, 2>>(noise: &F, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut weight = 1.0;

    for _ in 0..octaves {
        let signal = (1.0 - noise.get([x * frequency, y * frequency]).abs()) * amplitude;
        let signal = signal * weight;
        weight = (signal * 2.0).clamp(0.0, 1.0);
        value += signal;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    value * 0.5 - 0.5
}

fn turbulence<F: NoiseFn<f64, 2>>(noise: &F, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amp = 0.0;

    for _ in 0..octaves {
        value += noise.get([x * frequency, y * frequency]).abs() * amplitude;
        max_amp += amplitude;
        amplitude *= gain;
        frequency *= lacunarity;
    }
    value / max_amp * 2.0 - 1.0
}

fn domain_warp<F: NoiseFn<f64, 2>, G: NoiseFn<f64, 2>>(
    noise_a: &F, noise_b: &G,
    x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64, warp_strength: f64,
) -> f64 {
    let wx = fbm(noise_a, x, y, octaves, lacunarity, gain) * warp_strength;
    let wy = fbm(noise_b, x + 5.2, y + 1.3, octaves, lacunarity, gain) * warp_strength;
    let wx2 = fbm(noise_a, x + wx + 1.7, y + wy + 9.2, octaves, lacunarity, gain) * warp_strength;
    let wy2 = fbm(noise_b, x + wx + 8.3, y + wy + 2.8, octaves, lacunarity, gain) * warp_strength;
    fbm(noise_a, x + wx2, y + wy2, octaves, lacunarity, gain)
}

fn marble<F: NoiseFn<f64, 2>, G: NoiseFn<f64, 2>>(
    _noise_a: &F, noise_b: &G,
    x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64, warp_strength: f64,
) -> f64 {
    let warp = fbm(noise_b, x, y, octaves, lacunarity, gain) * warp_strength;
    ((x + warp) * std::f64::consts::PI).sin()
}

fn voronoi(x: f64, y: f64, seed: u32, metric: VoronoiMetric, output: VoronoiOutput) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let mut d1 = f64::MAX;
    let mut d2 = f64::MAX;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = ix + dx;
            let cy = iy + dy;
            // Hash cell to get point position
            let (px, py) = cell_point(cx, cy, seed);
            let diff_x = (dx as f64 + px) - fx;
            let diff_y = (dy as f64 + py) - fy;

            let dist = match metric {
                VoronoiMetric::Euclidean => (diff_x * diff_x + diff_y * diff_y).sqrt(),
                VoronoiMetric::Manhattan => diff_x.abs() + diff_y.abs(),
                VoronoiMetric::Chebyshev => diff_x.abs().max(diff_y.abs()),
            };

            if dist < d1 {
                d2 = d1;
                d1 = dist;
            } else if dist < d2 {
                d2 = dist;
            }
        }
    }

    let raw = match output {
        VoronoiOutput::F1 => d1,
        VoronoiOutput::F2 => d2,
        VoronoiOutput::F2MinusF1 => d2 - d1,
    };
    // Normalize to roughly -1..1
    raw * 2.0 - 1.0
}

/// Hash a cell coordinate to a pseudo-random point in [0, 1)^2.
fn cell_point(x: i32, y: i32, seed: u32) -> (f64, f64) {
    let h = hash2d(x, y, seed);
    let px = (h & 0xFFFF) as f64 / 65536.0;
    let py = ((h >> 16) & 0xFFFF) as f64 / 65536.0;
    (px, py)
}

fn hash2d(x: i32, y: i32, seed: u32) -> u32 {
    let mut h = seed;
    h = h.wrapping_add(x as u32).wrapping_mul(0x9E3779B9);
    h = h.wrapping_add(y as u32).wrapping_mul(0x517CC1B7);
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EBCA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2AE35);
    h ^= h >> 16;
    h
}

// =====================================================================
// Color mapping
// =====================================================================

fn colorize(t: f64, mode: ColorMode) -> [u8; 3] {
    match mode {
        ColorMode::Grayscale => {
            let v = (t * 255.0) as u8;
            [v, v, v]
        }
        ColorMode::Terrain => terrain_ramp(t),
        ColorMode::HeatMap => heat_ramp(t),
    }
}

fn terrain_ramp(t: f64) -> [u8; 3] {
    // Deep water → shallow water → sand → grass → forest → rock → snow
    if t < 0.3 {
        lerp_color([20, 40, 120], [60, 100, 180], t / 0.3)
    } else if t < 0.4 {
        lerp_color([60, 100, 180], [210, 200, 150], (t - 0.3) / 0.1)
    } else if t < 0.5 {
        lerp_color([210, 200, 150], [80, 160, 50], (t - 0.4) / 0.1)
    } else if t < 0.7 {
        lerp_color([80, 160, 50], [30, 100, 30], (t - 0.5) / 0.2)
    } else if t < 0.85 {
        lerp_color([30, 100, 30], [120, 110, 100], (t - 0.7) / 0.15)
    } else {
        lerp_color([120, 110, 100], [240, 240, 245], (t - 0.85) / 0.15)
    }
}

fn heat_ramp(t: f64) -> [u8; 3] {
    if t < 0.25 {
        lerp_color([0, 0, 80], [0, 80, 200], t / 0.25)
    } else if t < 0.5 {
        lerp_color([0, 80, 200], [0, 200, 100], (t - 0.25) / 0.25)
    } else if t < 0.75 {
        lerp_color([0, 200, 100], [230, 200, 0], (t - 0.5) / 0.25)
    } else {
        lerp_color([230, 200, 0], [200, 30, 0], (t - 0.75) / 0.25)
    }
}

/// Map height + moisture to a biome color (for Combined mode).
fn biome_color(height: f64, moisture: f64) -> [u8; 3] {
    if height < 0.35 {
        // Water
        lerp_color([20, 40, 140], [60, 100, 180], height / 0.35)
    } else if height < 0.42 {
        // Beach / sand
        [210, 195, 150]
    } else if height < 0.75 {
        // Land — moisture determines biome
        if moisture < 0.3 {
            // Desert / savanna
            lerp_color([190, 170, 100], [170, 160, 80], (height - 0.42) / 0.33)
        } else if moisture < 0.6 {
            // Grassland / temperate
            lerp_color([80, 160, 50], [50, 130, 40], (height - 0.42) / 0.33)
        } else {
            // Forest / jungle
            lerp_color([30, 110, 40], [20, 70, 30], (height - 0.42) / 0.33)
        }
    } else if height < 0.88 {
        // Mountain rock
        lerp_color([110, 100, 90], [140, 135, 125], (height - 0.75) / 0.13)
    } else {
        // Snow
        lerp_color([200, 200, 210], [245, 245, 250], (height - 0.88) / 0.12)
    }
}

fn lerp_color(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f64 + (b[0] as f64 - a[0] as f64) * t) as u8,
        (a[1] as f64 + (b[1] as f64 - a[1] as f64) * t) as u8,
        (a[2] as f64 + (b[2] as f64 - a[2] as f64) * t) as u8,
    ]
}

// =====================================================================
// Camera controls (matching texture_lab pattern)
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
