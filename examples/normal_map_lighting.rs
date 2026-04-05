//! Interactive normal map lighting prototype.
//!
//! Generates a height field from noise or SDF shapes, computes normal maps
//! analytically, and applies real-time directional lighting. Demonstrates
//! the pseudo-3D effect used for top-down terrain rendering.
//!
//! Usage:
//!   cargo run --example normal_map_lighting
//!
//! Controls:
//!   Left panel — select height source, tweak lighting parameters
//!   Left mouse drag — move light direction (on canvas)
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
// Height source
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum HeightSource {
    FbmNoise,
    RidgedNoise,
    DomainWarp,
    SdfShapes,
    Voronoi,
}

impl HeightSource {
    const ALL: &[HeightSource] = &[
        HeightSource::FbmNoise,
        HeightSource::RidgedNoise,
        HeightSource::DomainWarp,
        HeightSource::SdfShapes,
        HeightSource::Voronoi,
    ];

    fn label(self) -> &'static str {
        match self {
            HeightSource::FbmNoise => "fBM Noise",
            HeightSource::RidgedNoise => "Ridged Multifractal",
            HeightSource::DomainWarp => "Domain Warp",
            HeightSource::SdfShapes => "SDF Shapes",
            HeightSource::Voronoi => "Voronoi Cells",
        }
    }
}

// =====================================================================
// Visualization mode
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Lit,
    HeightMap,
    NormalMap,
    SplitCompare,
}

impl ViewMode {
    const ALL: &[ViewMode] = &[
        ViewMode::Lit,
        ViewMode::HeightMap,
        ViewMode::NormalMap,
        ViewMode::SplitCompare,
    ];

    fn label(self) -> &'static str {
        match self {
            ViewMode::Lit => "Lit (final)",
            ViewMode::HeightMap => "Height Map",
            ViewMode::NormalMap => "Normal Map",
            ViewMode::SplitCompare => "Split Compare",
        }
    }
}

// =====================================================================
// Parameters
// =====================================================================

#[derive(Resource, Clone)]
struct LightParams {
    height_source: HeightSource,
    view_mode: ViewMode,
    seed: u32,
    frequency: f64,
    octaves: u32,
    lacunarity: f64,
    gain: f64,
    warp_strength: f64,
    height_scale: f64,
    // Lighting
    light_azimuth: f64,   // horizontal angle in degrees (0 = right, 90 = up)
    light_elevation: f64, // angle above horizon in degrees
    ambient: f64,
    specular_strength: f64,
    specular_power: f64,
    // Color
    base_color: [f64; 3],
    use_terrain_colors: bool,
    // AO
    ao_strength: f64,
    ao_radius: u32,
}

impl Default for LightParams {
    fn default() -> Self {
        Self {
            height_source: HeightSource::FbmNoise,
            view_mode: ViewMode::Lit,
            seed: 42,
            frequency: 6.0,
            octaves: 6,
            lacunarity: 2.0,
            gain: 0.5,
            warp_strength: 4.0,
            height_scale: 1.0,
            light_azimuth: 225.0,
            light_elevation: 45.0,
            ambient: 0.15,
            specular_strength: 0.3,
            specular_power: 16.0,
            base_color: [0.45, 0.55, 0.35],
            use_terrain_colors: true,
            ao_strength: 0.4,
            ao_radius: 2,
        }
    }
}

#[derive(Resource)]
struct RenderDirty(bool);

#[derive(Resource, Default)]
struct DragState {
    dragging_light: bool,
}

#[derive(Component)]
struct DisplaySprite;

// =====================================================================
// Constants
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
                    title: "Normal Map Lighting".into(),
                    resolution: bevy::window::WindowResolution::new(1100, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(LightParams::default())
        .insert_resource(RenderDirty(true))
        .insert_resource(DragState::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(Update, (regenerate_texture, camera_zoom, camera_pan, drag_light))
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

fn ui_panel(mut contexts: EguiContexts, mut params: ResMut<LightParams>, mut dirty: ResMut<RenderDirty>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("light_params").min_width(280.0).show(ctx, |ui| {
        ui.heading("Normal Map Lighting");
        ui.separator();

        // Height source
        ui.label("Height Source");
        for &hs in HeightSource::ALL {
            if ui.radio_value(&mut params.height_source, hs, hs.label()).changed() { dirty.0 = true; }
        }
        ui.separator();

        // View mode
        ui.label("View Mode");
        for &vm in ViewMode::ALL {
            if ui.radio_value(&mut params.view_mode, vm, vm.label()).changed() { dirty.0 = true; }
        }
        ui.separator();

        // Height params
        ui.label("Seed");
        if ui.add(egui::DragValue::new(&mut params.seed).range(0..=9999)).changed() { dirty.0 = true; }

        ui.label("Frequency");
        if ui.add(egui::Slider::new(&mut params.frequency, 0.5..=30.0).logarithmic(true)).changed() { dirty.0 = true; }

        let uses_octaves = matches!(
            params.height_source,
            HeightSource::FbmNoise | HeightSource::RidgedNoise | HeightSource::DomainWarp
        );
        if uses_octaves {
            ui.label("Octaves");
            if ui.add(egui::Slider::new(&mut params.octaves, 1..=10)).changed() { dirty.0 = true; }
            ui.label("Lacunarity");
            if ui.add(egui::Slider::new(&mut params.lacunarity, 1.0..=4.0)).changed() { dirty.0 = true; }
            ui.label("Gain");
            if ui.add(egui::Slider::new(&mut params.gain, 0.1..=0.9)).changed() { dirty.0 = true; }
        }

        if params.height_source == HeightSource::DomainWarp {
            ui.label("Warp Strength");
            if ui.add(egui::Slider::new(&mut params.warp_strength, 0.1..=10.0)).changed() { dirty.0 = true; }
        }

        ui.label("Height Scale");
        if ui.add(egui::Slider::new(&mut params.height_scale, 0.1..=5.0)).changed() { dirty.0 = true; }

        ui.separator();

        // Lighting
        ui.heading("Lighting");
        ui.label("Light Azimuth");
        if ui.add(egui::Slider::new(&mut params.light_azimuth, 0.0..=360.0).suffix("°")).changed() { dirty.0 = true; }
        ui.label("Light Elevation");
        if ui.add(egui::Slider::new(&mut params.light_elevation, 5.0..=85.0).suffix("°")).changed() { dirty.0 = true; }
        ui.label("Ambient");
        if ui.add(egui::Slider::new(&mut params.ambient, 0.0..=0.5)).changed() { dirty.0 = true; }
        ui.label("Specular Strength");
        if ui.add(egui::Slider::new(&mut params.specular_strength, 0.0..=1.0)).changed() { dirty.0 = true; }
        ui.label("Specular Power");
        if ui.add(egui::Slider::new(&mut params.specular_power, 2.0..=64.0)).changed() { dirty.0 = true; }

        ui.separator();

        // AO
        ui.heading("Ambient Occlusion");
        ui.label("AO Strength");
        if ui.add(egui::Slider::new(&mut params.ao_strength, 0.0..=1.0)).changed() { dirty.0 = true; }
        ui.label("AO Radius");
        let mut r = params.ao_radius as i32;
        if ui.add(egui::Slider::new(&mut r, 0..=5)).changed() {
            params.ao_radius = r as u32;
            dirty.0 = true;
        }

        ui.separator();

        // Color
        ui.heading("Color");
        if ui.checkbox(&mut params.use_terrain_colors, "Terrain colors (height-based)").changed() { dirty.0 = true; }
        if !params.use_terrain_colors {
            let mut c = [params.base_color[0] as f32, params.base_color[1] as f32, params.base_color[2] as f32];
            if ui.color_edit_button_rgb(&mut c).changed() {
                params.base_color = [c[0] as f64, c[1] as f64, c[2] as f64];
                dirty.0 = true;
            }
        }

        ui.separator();
        ui.small("Left-click drag on canvas to move light direction");
    });
}

// =====================================================================
// Height field generation
// =====================================================================

fn generate_height_field(params: &LightParams) -> Vec<f64> {
    let simplex = OpenSimplex::new(params.seed);
    let simplex_b = OpenSimplex::new(params.seed.wrapping_add(293));

    let mut heights = vec![0.0f64; (TEX_W * TEX_H) as usize];

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let nx = px as f64 / TEX_W as f64 * params.frequency;
            let ny = py as f64 / TEX_H as f64 * params.frequency;

            let h = match params.height_source {
                HeightSource::FbmNoise => fbm(&simplex, nx, ny, params.octaves, params.lacunarity, params.gain),
                HeightSource::RidgedNoise => ridged_multi(&simplex, nx, ny, params.octaves, params.lacunarity, params.gain),
                HeightSource::DomainWarp => domain_warp(&simplex, &simplex_b, nx, ny, params.octaves, params.lacunarity, params.gain, params.warp_strength),
                HeightSource::SdfShapes => sdf_height(nx / params.frequency, ny / params.frequency),
                HeightSource::Voronoi => voronoi_height(nx, ny, params.seed),
            };

            heights[(py * TEX_W + px) as usize] = h * params.height_scale;
        }
    }
    heights
}

fn sdf_height(nx: f64, ny: f64) -> f64 {
    // A few SDF shapes composed together to demonstrate normal mapping on geometry
    let cx = nx - 0.5;
    let cy = ny - 0.5;

    let circle1 = ((cx + 0.1) * (cx + 0.1) + cy * cy).sqrt() - 0.15;
    let circle2 = ((cx - 0.15) * (cx - 0.15) + (cy - 0.05) * (cy - 0.05)).sqrt() - 0.12;
    let box1 = {
        let dx = (cx - 0.0).abs() - 0.25;
        let dy = (cy + 0.15).abs() - 0.06;
        let outside = (dx.max(0.0) * dx.max(0.0) + dy.max(0.0) * dy.max(0.0)).sqrt();
        let inside = dx.max(dy).min(0.0);
        outside + inside
    };

    let d = smooth_min(circle1, circle2, 0.05);
    let d = smooth_min(d, box1, 0.08);

    // Convert distance to height: inside = high, outside = 0, smooth falloff
    (-d * 8.0).clamp(0.0, 1.0)
}

fn voronoi_height(x: f64, y: f64, seed: u32) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - x.floor();
    let fy = y - y.floor();

    let mut d1 = f64::MAX;
    let mut d2 = f64::MAX;

    for dy in -1..=1 {
        for dx in -1..=1 {
            let (px, py) = cell_point(ix + dx, iy + dy, seed);
            let diff_x = (dx as f64 + px) - fx;
            let diff_y = (dy as f64 + py) - fy;
            let dist = (diff_x * diff_x + diff_y * diff_y).sqrt();
            if dist < d1 { d2 = d1; d1 = dist; }
            else if dist < d2 { d2 = dist; }
        }
    }
    // F2 - F1 gives cell edges (ridges)
    (d2 - d1) * 2.0 - 0.5
}

fn smooth_min(a: f64, b: f64, k: f64) -> f64 {
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

// =====================================================================
// Normal computation
// =====================================================================

fn compute_normals(heights: &[f64]) -> Vec<[f64; 3]> {
    let mut normals = vec![[0.0, 0.0, 1.0]; (TEX_W * TEX_H) as usize];
    let step = 1.0 / TEX_W as f64;

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let idx = (py * TEX_W + px) as usize;

            // Central differences for partial derivatives
            let left = if px > 0 { heights[idx - 1] } else { heights[idx] };
            let right = if px < TEX_W - 1 { heights[idx + 1] } else { heights[idx] };
            let up = if py > 0 { heights[idx - TEX_W as usize] } else { heights[idx] };
            let down = if py < TEX_H - 1 { heights[idx + TEX_W as usize] } else { heights[idx] };

            let dhdx = (right - left) / (2.0 * step);
            let dhdy = (up - down) / (2.0 * step); // up-down: pixel Y is flipped vs world Y

            // Normal = normalize(cross(tangent_x, tangent_y))
            // tangent_x = (1, 0, dhdx), tangent_y = (0, 1, dhdy)
            // cross = (-dhdx, -dhdy, 1)
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
// Ambient occlusion
// =====================================================================

fn compute_ao(heights: &[f64], radius: u32) -> Vec<f64> {
    if radius == 0 {
        return vec![1.0; (TEX_W * TEX_H) as usize];
    }

    let mut ao = vec![1.0f64; (TEX_W * TEX_H) as usize];
    let r = radius as i32;

    for py in 0..TEX_H as i32 {
        for px in 0..TEX_W as i32 {
            let idx = (py * TEX_W as i32 + px) as usize;
            let center_h = heights[idx];
            let mut occlusion = 0.0;
            let mut samples = 0.0;

            for dy in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dy == 0 { continue; }
                    let sx = (px + dx).clamp(0, TEX_W as i32 - 1);
                    let sy = (py + dy).clamp(0, TEX_H as i32 - 1);
                    let si = (sy * TEX_W as i32 + sx) as usize;

                    let diff = heights[si] - center_h;
                    if diff > 0.0 {
                        let dist = ((dx * dx + dy * dy) as f64).sqrt();
                        occlusion += (diff / dist).min(1.0);
                    }
                    samples += 1.0;
                }
            }

            ao[idx] = 1.0 - (occlusion / samples).min(1.0);
        }
    }
    ao
}

// =====================================================================
// Lighting
// =====================================================================

fn light_direction(azimuth_deg: f64, elevation_deg: f64) -> [f64; 3] {
    let az = azimuth_deg.to_radians();
    let el = elevation_deg.to_radians();
    let x = az.cos() * el.cos();
    let y = az.sin() * el.cos();
    let z = el.sin();
    [x, y, z]
}

fn shade_pixel(
    normal: [f64; 3],
    light: [f64; 3],
    ambient: f64,
    specular_strength: f64,
    specular_power: f64,
    ao: f64,
    ao_strength: f64,
) -> f64 {
    // Diffuse (Lambertian)
    let ndotl = normal[0] * light[0] + normal[1] * light[1] + normal[2] * light[2];
    let diffuse = ndotl.max(0.0);

    // Specular (Blinn-Phong, view direction = straight down = [0, 0, 1])
    let hx = light[0];
    let hy = light[1];
    let hz = light[2] + 1.0;
    let hlen = (hx * hx + hy * hy + hz * hz).sqrt();
    let ndoth = normal[0] * (hx / hlen) + normal[1] * (hy / hlen) + normal[2] * (hz / hlen);
    let specular = ndoth.max(0.0).powf(specular_power) * specular_strength;

    // Combine
    let ao_factor = 1.0 - ao_strength * (1.0 - ao);
    (ambient + diffuse * ao_factor + specular).min(1.0)
}

// =====================================================================
// Color mapping
// =====================================================================

fn terrain_color(height: f64) -> [f64; 3] {
    let h = (height * 0.5 + 0.5).clamp(0.0, 1.0);
    if h < 0.3 {
        lerp3([0.08, 0.15, 0.45], [0.2, 0.35, 0.6], h / 0.3)
    } else if h < 0.4 {
        lerp3([0.2, 0.35, 0.6], [0.75, 0.72, 0.55], (h - 0.3) / 0.1)
    } else if h < 0.55 {
        lerp3([0.3, 0.55, 0.2], [0.2, 0.45, 0.15], (h - 0.4) / 0.15)
    } else if h < 0.75 {
        lerp3([0.2, 0.45, 0.15], [0.35, 0.3, 0.25], (h - 0.55) / 0.2)
    } else {
        lerp3([0.35, 0.3, 0.25], [0.9, 0.9, 0.92], (h - 0.75) / 0.25)
    }
}

fn lerp3(a: [f64; 3], b: [f64; 3], t: f64) -> [f64; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn normal_to_color(n: [f64; 3]) -> [u8; 3] {
    // Standard normal map encoding: [-1,1] -> [0,255]
    [
        ((n[0] * 0.5 + 0.5) * 255.0) as u8,
        ((n[1] * 0.5 + 0.5) * 255.0) as u8,
        ((n[2] * 0.5 + 0.5) * 255.0) as u8,
    ]
}

// =====================================================================
// Texture regeneration
// =====================================================================

fn regenerate_texture(
    mut dirty: ResMut<RenderDirty>,
    params: Res<LightParams>,
    mut images: ResMut<Assets<Image>>,
    sprites: Query<&Sprite, With<DisplaySprite>>,
) {
    if !dirty.0 { return; }
    dirty.0 = false;

    let Ok(sprite) = sprites.single() else { return };
    let Some(image) = images.get_mut(&sprite.image) else { return };

    let heights = generate_height_field(&params);
    let normals = compute_normals(&heights);
    let ao = compute_ao(&heights, params.ao_radius);
    let light = light_direction(params.light_azimuth, params.light_elevation);

    let num_pixels = (TEX_W * TEX_H * 4) as usize;
    let mut pixels = vec![0u8; num_pixels];

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            let idx = (py * TEX_W + px) as usize;
            let pidx = idx * 4;
            let h = heights[idx];

            let [r, g, b] = match params.view_mode {
                ViewMode::HeightMap => {
                    let v = ((h * 0.5 + 0.5).clamp(0.0, 1.0) * 255.0) as u8;
                    [v, v, v]
                }
                ViewMode::NormalMap => {
                    normal_to_color(normals[idx])
                }
                ViewMode::Lit => {
                    let brightness = shade_pixel(
                        normals[idx], light,
                        params.ambient, params.specular_strength, params.specular_power,
                        ao[idx], params.ao_strength,
                    );
                    let base = if params.use_terrain_colors {
                        terrain_color(h)
                    } else {
                        params.base_color
                    };
                    [
                        (base[0] * brightness * 255.0).min(255.0) as u8,
                        (base[1] * brightness * 255.0).min(255.0) as u8,
                        (base[2] * brightness * 255.0).min(255.0) as u8,
                    ]
                }
                ViewMode::SplitCompare => {
                    // Left half: flat (no lighting), right half: lit
                    if px < TEX_W / 2 {
                        // Flat — just base color
                        let base = if params.use_terrain_colors {
                            terrain_color(h)
                        } else {
                            params.base_color
                        };
                        [
                            (base[0] * 255.0) as u8,
                            (base[1] * 255.0) as u8,
                            (base[2] * 255.0) as u8,
                        ]
                    } else {
                        // Lit
                        let brightness = shade_pixel(
                            normals[idx], light,
                            params.ambient, params.specular_strength, params.specular_power,
                            ao[idx], params.ao_strength,
                        );
                        let base = if params.use_terrain_colors {
                            terrain_color(h)
                        } else {
                            params.base_color
                        };
                        [
                            (base[0] * brightness * 255.0).min(255.0) as u8,
                            (base[1] * brightness * 255.0).min(255.0) as u8,
                            (base[2] * brightness * 255.0).min(255.0) as u8,
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

// =====================================================================
// Noise functions (same as noise_visualizer)
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

// =====================================================================
// Drag light direction with left mouse
// =====================================================================

fn drag_light(
    mut params: ResMut<LightParams>,
    mut dirty: ResMut<RenderDirty>,
    mut drag: ResMut<DragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&Sprite, &GlobalTransform), With<DisplaySprite>>,
    mut contexts: EguiContexts,
) {
    let egui_wants = contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_pointer_input());

    if mouse.just_released(MouseButton::Left) {
        drag.dragging_light = false;
        return;
    }

    if mouse.just_pressed(MouseButton::Left) && !egui_wants {
        drag.dragging_light = true;
    }

    if !drag.dragging_light || !mouse.pressed(MouseButton::Left) { return; }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_tf)) = camera.single() else { return };
    let Ok((sprite, sprite_tf)) = sprites.single() else { return };

    let Some(cursor) = window.cursor_position() else { return };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_tf, cursor) else { return };

    let sprite_size = sprite.custom_size.unwrap_or(Vec2::new(TEX_W as f32, TEX_H as f32));
    let sprite_pos = sprite_tf.translation().truncate();
    let local = (world_pos - sprite_pos) / sprite_size;

    // Map cursor position to light azimuth (angle from center)
    let azimuth = (local.y as f64).atan2(local.x as f64).to_degrees();
    params.light_azimuth = (azimuth + 360.0) % 360.0;

    // Map distance from center to elevation (center = overhead, edge = low angle)
    let dist = (local.x * local.x + local.y * local.y).sqrt() as f64;
    params.light_elevation = (1.0 - (dist * 2.0).min(1.0)) * 80.0 + 5.0;

    dirty.0 = true;
}
