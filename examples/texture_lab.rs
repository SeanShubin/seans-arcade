//! Interactive 47-blob texture tweaker with map preview.
//!
//! Usage:
//!   cargo run --example texture_lab                    # GUI mode
//!   cargo run --example texture_lab -- --render        # render to PNG and exit
//!   cargo run --example texture_lab -- --render out.png
//!   cargo run --example texture_lab -- --preset "Dark Stone" --render
//!
//! GUI Controls:
//!   - Left panel: tweak material parameters
//!   - Scroll wheel: zoom in/out
//!   - Click + drag: pan camera
//!   - Presets dropdown to switch between starting points
//!   - "Save PNG" button to export the current tileset

#[allow(unused_imports)]
use bevy::asset::RenderAssetUsages;
#[allow(unused_imports)]
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
#[allow(unused_imports)]
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
#[allow(unused_imports)]
use bevy_egui::{EguiContexts, egui};
use noise::{NoiseFn, OpenSimplex, Perlin};

const TILE_SIZE: u32 = 64;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let render_mode = args.iter().any(|a| a == "--render");
    let preset_name = args.iter().position(|a| a == "--preset").map(|i| {
        args.get(i + 1).expect("--preset requires a name").clone()
    });
    let output_path = if render_mode {
        // Check for a path arg after --render (that isn't another flag)
        let ri = args.iter().position(|a| a == "--render").unwrap();
        args.get(ri + 1)
            .filter(|a| !a.starts_with("--"))
            .cloned()
            .unwrap_or_else(|| "assets/generated/wall/_preview.png".to_string())
    } else {
        String::new()
    };

    // Apply preset if specified
    let params = if let Some(name) = &preset_name {
        PRESETS
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                let names: Vec<_> = PRESETS.iter().map(|(n, _)| *n).collect();
                panic!("Unknown preset '{}'. Available: {:?}", name, names);
            })
            .1()
    } else {
        TexParams::default()
    };

    if render_mode {
        render_and_exit(&params, &output_path);
    } else {
        run_gui(params);
    }
}

fn render_and_exit(params: &TexParams, output_path: &str) {
    let grid = MapGrid::load();
    let pixels = generate_map_pixels(params, &grid);
    let img = image::RgbaImage::from_raw(grid.pixel_width(), grid.pixel_height(), pixels)
        .expect("Failed to create image buffer");
    let dir = std::path::Path::new(output_path).parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir).ok();
    img.save(output_path).expect("Failed to save image");
    println!("Rendered: {}", output_path);
}

fn run_gui(params: TexParams) {
    use bevy_egui::EguiPlugin;

    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Texture Lab".into(),
                    resolution: bevy::window::WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(params)
        .init_resource::<TexDirty>()
        .add_systems(Startup, setup)
        .add_systems(bevy_egui::EguiPrimaryContextPass, ui_system)
        .add_systems(Update, (regenerate_system, camera_zoom, camera_pan))
        .run();
}

// --- Resources ---

#[derive(Resource)]
struct TexParams {
    base_color: [f32; 3],
    color_variation: f32,
    noise_scale: f32,
    noise_octaves: u32,
    pattern: usize, // index into PATTERN_NAMES
    bevel_width: f32,
    shadow_strength: f32,
    highlight_strength: f32,
    light_angle: f32,
    speckle_density: f32,
    speckle_color: [f32; 3],
    use_secondary: bool,
    secondary_color: [f32; 3],
    stripe_angle: f32,
    seed: u32,
    floor_color: [f32; 3],
}

impl Default for TexParams {
    fn default() -> Self {
        Self {
            base_color: [0.62, 0.62, 0.62],
            color_variation: 0.06,
            noise_scale: 0.08,
            noise_octaves: 3,
            pattern: 0,
            bevel_width: 14.0,
            shadow_strength: 0.7,
            highlight_strength: 0.4,
            light_angle: 135.0,
            speckle_density: 0.0,
            speckle_color: [1.0, 1.0, 1.0],
            use_secondary: false,
            secondary_color: [0.3, 0.3, 0.3],
            stripe_angle: 0.0,
            seed: 42,
            floor_color: [0.412, 0.416, 0.475],
        }
    }
}

const PATTERN_NAMES: &[&str] = &["Perlin", "Cellular", "Brick", "Stripe", "Marble"];

const PRESETS: &[(&str, fn() -> TexParams)] = &[
    ("Concrete", || TexParams {
        base_color: [0.62, 0.62, 0.62],
        color_variation: 0.06,
        noise_scale: 0.08,
        noise_octaves: 3,
        pattern: 0,
        bevel_width: 14.0,
        shadow_strength: 0.7,
        highlight_strength: 0.4,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Red Brick", || TexParams {
        base_color: [0.6, 0.25, 0.18],
        color_variation: 0.06,
        noise_scale: 0.1,
        noise_octaves: 2,
        pattern: 2,
        bevel_width: 4.0,
        shadow_strength: 0.4,
        highlight_strength: 0.2,
        light_angle: 135.0,
        speckle_density: 0.08,
        speckle_color: [0.85, 0.85, 0.8],
        use_secondary: true,
        secondary_color: [0.78, 0.75, 0.65],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Dark Stone", || TexParams {
        base_color: [0.3, 0.3, 0.32],
        color_variation: 0.1,
        noise_scale: 0.04,
        noise_octaves: 4,
        pattern: 1,
        bevel_width: 6.0,
        shadow_strength: 0.5,
        highlight_strength: 0.2,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.18, 0.18, 0.2],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Marble", || TexParams {
        base_color: [0.88, 0.86, 0.82],
        color_variation: 0.15,
        noise_scale: 0.03,
        noise_octaves: 4,
        pattern: 4,
        bevel_width: 4.0,
        shadow_strength: 0.3,
        highlight_strength: 0.5,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.4, 0.35, 0.3],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Wood Plank", || TexParams {
        base_color: [0.55, 0.38, 0.22],
        color_variation: 0.12,
        noise_scale: 0.06,
        noise_octaves: 3,
        pattern: 3,
        bevel_width: 5.0,
        shadow_strength: 0.35,
        highlight_strength: 0.15,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.42, 0.28, 0.15],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Blue Tile", || TexParams {
        base_color: [0.2, 0.35, 0.6],
        color_variation: 0.03,
        noise_scale: 0.15,
        noise_octaves: 1,
        pattern: 0,
        bevel_width: 4.0,
        shadow_strength: 0.5,
        highlight_strength: 0.7,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Sandstone", || TexParams {
        base_color: [0.72, 0.62, 0.45],
        color_variation: 0.1,
        noise_scale: 0.07,
        noise_octaves: 3,
        pattern: 0,
        bevel_width: 5.0,
        shadow_strength: 0.35,
        highlight_strength: 0.15,
        light_angle: 135.0,
        speckle_density: 0.04,
        speckle_color: [0.85, 0.78, 0.6],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
    ("Metal Plate", || TexParams {
        base_color: [0.5, 0.52, 0.55],
        color_variation: 0.02,
        noise_scale: 0.2,
        noise_octaves: 1,
        pattern: 0,
        bevel_width: 3.0,
        shadow_strength: 0.5,
        highlight_strength: 0.8,
        light_angle: 135.0,
        speckle_density: 0.02,
        speckle_color: [0.7, 0.72, 0.75],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        floor_color: [0.412, 0.416, 0.475],
    }),
];

#[derive(Resource, Default)]
struct TexDirty(bool);

#[derive(Resource)]
struct TexImage(Handle<Image>);

/// The IntGrid loaded from reference.ldtk.
#[derive(Resource)]
struct MapGrid {
    cells: Vec<u8>, // 0 = empty, 1 = wall
    cols: usize,
    rows: usize,
}

impl MapGrid {
    fn load() -> Self {
        let data = std::fs::read_to_string("maps/reference.ldtk")
            .expect("Failed to read maps/reference.ldtk");
        let json: serde_json::Value =
            serde_json::from_str(&data).expect("Failed to parse LDtk JSON");

        let level = &json["levels"][0];
        // Find the IntGrid layer
        let layers = level["layerInstances"].as_array().unwrap();
        let intgrid_layer = layers
            .iter()
            .find(|l| l["__type"].as_str() == Some("IntGrid"))
            .expect("No IntGrid layer found");

        let cols = intgrid_layer["__cWid"].as_u64().unwrap() as usize;
        let rows = intgrid_layer["__cHei"].as_u64().unwrap() as usize;
        let csv = intgrid_layer["intGridCsv"].as_array().unwrap();
        let cells: Vec<u8> = csv.iter().map(|v| v.as_u64().unwrap() as u8).collect();

        assert_eq!(cells.len(), cols * rows);
        Self { cells, cols, rows }
    }

    fn get(&self, col: usize, row: usize) -> u8 {
        self.cells[row * self.cols + col]
    }

    fn pixel_width(&self) -> u32 {
        (self.cols as u32) * TILE_SIZE
    }

    fn pixel_height(&self) -> u32 {
        (self.rows as u32) * TILE_SIZE
    }
}

// --- Setup ---

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, params: Res<TexParams>) {
    commands.spawn(Camera2d);

    let grid = MapGrid::load();
    let pixels = generate_map_pixels(&params, &grid);

    let image = Image::new(
        Extent3d {
            width: grid.pixel_width(),
            height: grid.pixel_height(),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    let handle = images.add(image);
    commands.spawn(Sprite {
        image: handle.clone(),
        ..default()
    });
    commands.insert_resource(TexImage(handle));
    commands.insert_resource(grid);
}

// --- UI ---

fn ui_system(
    mut contexts: EguiContexts,
    mut params: ResMut<TexParams>,
    mut dirty: ResMut<TexDirty>,
    tex_image: Res<TexImage>,
    images: Res<Assets<Image>>,
    grid: Res<MapGrid>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("controls").min_width(260.0).show(ctx, |ui| {
        ui.heading("Texture Lab");
        ui.separator();

        // Presets
        ui.label("Presets");
        ui.horizontal_wrapped(|ui| {
            for &(name, make_fn) in PRESETS {
                if ui.button(name).clicked() {
                    *params = make_fn();
                    dirty.0 = true;
                }
            }
        });
        ui.separator();

        // Pattern type
        let mut changed = false;
        ui.label("Pattern");
        egui::ComboBox::from_id_salt("pattern")
            .selected_text(PATTERN_NAMES[params.pattern])
            .show_ui(ui, |ui| {
                for (i, name) in PATTERN_NAMES.iter().enumerate() {
                    if ui.selectable_value(&mut params.pattern, i, *name).changed() {
                        changed = true;
                    }
                }
            });

        if params.pattern == 3 {
            changed |= ui
                .add(egui::Slider::new(&mut params.stripe_angle, 0.0..=360.0).text("Stripe angle"))
                .changed();
        }

        ui.separator();

        // Colors
        ui.label("Base color");
        changed |= ui.color_edit_button_rgb(&mut params.base_color).changed();

        changed |= ui
            .add(egui::Slider::new(&mut params.color_variation, 0.0..=0.3).text("Color variation"))
            .changed();

        changed |= ui.checkbox(&mut params.use_secondary, "Secondary color").changed();
        if params.use_secondary {
            changed |= ui.color_edit_button_rgb(&mut params.secondary_color).changed();
        }

        ui.separator();

        // Noise
        ui.label("Noise");
        changed |= ui
            .add(egui::Slider::new(&mut params.noise_scale, 0.01..=0.5).text("Scale"))
            .changed();

        let mut octaves = params.noise_octaves as i32;
        if ui
            .add(egui::Slider::new(&mut octaves, 1..=6).text("Octaves"))
            .changed()
        {
            params.noise_octaves = octaves as u32;
            changed = true;
        }

        let mut seed = params.seed as i32;
        if ui
            .add(egui::Slider::new(&mut seed, 0..=999).text("Seed"))
            .changed()
        {
            params.seed = seed as u32;
            changed = true;
        }

        ui.separator();

        // Bevel
        ui.label("Bevel");
        changed |= ui
            .add(egui::Slider::new(&mut params.bevel_width, 0.0..=20.0).text("Width"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut params.shadow_strength, 0.0..=1.0).text("Shadow"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut params.highlight_strength, 0.0..=1.0).text("Highlight"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut params.light_angle, 0.0..=360.0).text("Light angle°"))
            .changed();

        ui.separator();

        // Speckle
        ui.label("Speckle");
        changed |= ui
            .add(egui::Slider::new(&mut params.speckle_density, 0.0..=0.3).text("Density"))
            .changed();
        if params.speckle_density > 0.0 {
            changed |= ui.color_edit_button_rgb(&mut params.speckle_color).changed();
        }

        ui.separator();

        // Floor
        ui.label("Floor color");
        changed |= ui.color_edit_button_rgb(&mut params.floor_color).changed();

        if changed {
            dirty.0 = true;
        }

        ui.separator();

        // Save button
        if ui.button("Save PNG").clicked() {
            if let Some(image) = images.get(&tex_image.0) {
                save_image(image, &grid);
            }
        }

        ui.separator();
        ui.label("Scroll to zoom, drag to pan");
    });
}

fn save_image(image: &Image, grid: &MapGrid) {
    let dir = std::path::Path::new("assets/generated/wall");
    std::fs::create_dir_all(dir).ok();

    // Find next available name
    let mut i = 1;
    let path = loop {
        let p = dir.join(format!("Wall-custom_{}-64x64.png", i));
        if !p.exists() {
            break p;
        }
        i += 1;
    };

    let img = image::RgbaImage::from_raw(
        grid.pixel_width(),
        grid.pixel_height(),
        image.data.clone().unwrap(),
    )
    .expect("Failed to create image buffer");
    img.save(&path).expect("Failed to save");
    println!("Saved: {}", path.display());
}

// --- Regenerate texture when dirty ---

fn regenerate_system(
    mut dirty: ResMut<TexDirty>,
    params: Res<TexParams>,
    tex_image: Res<TexImage>,
    mut images: ResMut<Assets<Image>>,
    grid: Res<MapGrid>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    if let Some(image) = images.get_mut(&tex_image.0) {
        image.data = Some(generate_map_pixels(&params, &grid));
    }
}

// --- Camera controls ---

fn camera_zoom(
    mut scroll_events: MessageReader<MouseWheel>,
    mut proj_q: Query<&mut Projection, With<Camera2d>>,
) {
    let mut zoom_delta = 0.0;
    for ev in scroll_events.read() {
        zoom_delta += match ev.unit {
            MouseScrollUnit::Line => -ev.y * 0.15,
            MouseScrollUnit::Pixel => -ev.y * 0.002,
        };
    }
    if zoom_delta != 0.0 {
        if let Ok(mut proj) = proj_q.single_mut() {
            let Projection::Orthographic(ref mut ortho) = *proj else { return };
            ortho.scale = (ortho.scale * (1.0 + zoom_delta)).clamp(0.1, 10.0);
        }
    }
}

fn camera_pan(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<bevy::input::mouse::MouseMotion>,
    mut camera: Query<(&mut Transform, &Projection), With<Camera2d>>,
) {
    if !buttons.pressed(MouseButton::Left) {
        motion.read(); // drain
        return;
    }
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta != Vec2::ZERO {
        if let Ok((mut transform, proj)) = camera.single_mut() {
            let Projection::Orthographic(ref ortho) = *proj else { return };
            transform.translation.x -= delta.x * ortho.scale;
            transform.translation.y += delta.y * ortho.scale;
        }
    }
}

// =====================================================================
// Map-based texture generation
// =====================================================================

struct Edges {
    n: bool, s: bool, e: bool, w: bool,
    inner_nw: bool, inner_ne: bool, inner_sw: bool, inner_se: bool,
}

/// Compute the neighbor pattern for a cell in the map grid.
/// Returns [NW, N, NE, W, center(1), E, SW, S, SE].
/// Out-of-bounds cells are treated as empty (-1).
fn neighbor_pattern(grid: &MapGrid, col: usize, row: usize) -> [i8; 9] {
    let same = |c: i32, r: i32| -> i8 {
        if c < 0 || r < 0 || c >= grid.cols as i32 || r >= grid.rows as i32 {
            -1
        } else if grid.get(c as usize, r as usize) == 1 {
            1
        } else {
            -1
        }
    };
    let c = col as i32;
    let r = row as i32;
    [
        same(c - 1, r - 1), same(c, r - 1), same(c + 1, r - 1),
        same(c - 1, r),     1,               same(c + 1, r),
        same(c - 1, r + 1), same(c, r + 1), same(c + 1, r + 1),
    ]
}

/// Generate map-sized pixel buffer using the IntGrid and autotile rules.
fn generate_map_pixels(params: &TexParams, grid: &MapGrid) -> Vec<u8> {
    let img_w = grid.pixel_width();
    let img_h = grid.pixel_height();
    let mut pixels = vec![0u8; (img_w * img_h * 4) as usize];

    // Fill with floor color for empty cells
    let fr = (params.floor_color[0].clamp(0.0, 1.0) * 255.0) as u8;
    let fg = (params.floor_color[1].clamp(0.0, 1.0) * 255.0) as u8;
    let fb = (params.floor_color[2].clamp(0.0, 1.0) * 255.0) as u8;
    for i in 0..(img_w * img_h) as usize {
        pixels[i * 4] = fr;
        pixels[i * 4 + 1] = fg;
        pixels[i * 4 + 2] = fb;
        pixels[i * 4 + 3] = 255;
    }

    let perlin = Perlin::new(params.seed);
    let simplex = OpenSimplex::new(params.seed.wrapping_add(81));
    let light_rad = params.light_angle.to_radians() as f64;

    for row in 0..grid.rows {
        for col in 0..grid.cols {
            if grid.get(col, row) == 0 {
                continue; // empty cell
            }

            // Use actual neighbor data (not matched rule) for edge detection.
            // The rule pattern has 0="don't care" which would hide real edges.
            let pattern = neighbor_pattern(grid, col, row);

            let edges = Edges {
                n: pattern[1] == -1,
                s: pattern[7] == -1,
                w: pattern[3] == -1,
                e: pattern[5] == -1,
                inner_nw: pattern[0] == -1 && pattern[1] != -1 && pattern[3] != -1,
                inner_ne: pattern[2] == -1 && pattern[1] != -1 && pattern[5] != -1,
                inner_sw: pattern[6] == -1 && pattern[7] != -1 && pattern[3] != -1,
                inner_se: pattern[8] == -1 && pattern[7] != -1 && pattern[5] != -1,
            };

            let tile_x = (col as u32) * TILE_SIZE;
            let tile_y = (row as u32) * TILE_SIZE;

            for py in 0..TILE_SIZE {
                for px in 0..TILE_SIZE {
                    let wx = (tile_x + px) as f64;
                    let wy = (tile_y + py) as f64;
                    let lx = px as f64;
                    let ly = py as f64;

                    let base = sample_pattern(params, &perlin, &simplex, wx, wy);
                    let speckled = apply_speckle(params, &perlin, base, wx, wy);
                    let bevel = compute_bevel(&edges, lx, ly, params, light_rad);
                    let final_color = apply_bevel(speckled, bevel, params);

                    let idx = ((tile_y + py) * img_w + (tile_x + px)) as usize * 4;
                    pixels[idx] = (final_color[0].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 1] = (final_color[1].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 2] = (final_color[2].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 3] = 255;
                }
            }
        }
    }

    pixels
}

// =====================================================================
// Material sampling & bevel (same math as generate_textures.rs)
// =====================================================================

fn sample_pattern(
    params: &TexParams,
    perlin: &Perlin,
    simplex: &OpenSimplex,
    x: f64,
    y: f64,
) -> [f64; 3] {
    let scale = params.noise_scale as f64;
    let octaves = params.noise_octaves;

    let noise_val = match params.pattern {
        0 => fbm(perlin, x * scale, y * scale, octaves), // Perlin
        1 => cellular_noise(x * scale, y * scale),         // Cellular
        2 => brick_pattern(x, y, params, perlin),          // Brick
        3 => {                                              // Stripe
            let angle = (params.stripe_angle as f64).to_radians();
            let rotated = x * angle.cos() + y * angle.sin();
            let stripe = fbm(perlin, rotated * scale * 3.0, 0.3, octaves);
            let detail = fbm(simplex, x * scale * 0.5, y * scale * 2.0, 2);
            stripe * 0.8 + detail * 0.2
        }
        4 => {                                              // Marble
            let warp_x = fbm(perlin, x * scale, y * scale, 3) * 8.0;
            let warp_y = fbm(simplex, x * scale + 5.3, y * scale + 1.7, 3) * 8.0;
            let v = (x * scale + warp_x).sin() * 0.5 + 0.5;
            let detail = fbm(perlin, x * scale * 2.0 + warp_y, y * scale * 2.0, 2);
            v * 0.7 + detail * 0.3
        }
        _ => 0.0,
    };

    let var = noise_val * params.color_variation as f64;
    let base = [
        params.base_color[0] as f64,
        params.base_color[1] as f64,
        params.base_color[2] as f64,
    ];

    if params.use_secondary {
        let sec = [
            params.secondary_color[0] as f64,
            params.secondary_color[1] as f64,
            params.secondary_color[2] as f64,
        ];
        let t = (noise_val * 0.5 + 0.5).clamp(0.0, 1.0);
        [
            lerp(base[0], sec[0], t) + var,
            lerp(base[1], sec[1], t) + var,
            lerp(base[2], sec[2], t) + var,
        ]
    } else {
        [base[0] + var, base[1] + var, base[2] + var]
    }
}

fn brick_pattern(x: f64, y: f64, params: &TexParams, perlin: &Perlin) -> f64 {
    let brick_w = 16.0;
    let brick_h = 8.0;
    let mortar = 1.0;
    let row = (y / brick_h).floor() as i32;
    let offset = if row % 2 == 0 { 0.0 } else { brick_w * 0.5 };
    let bx = ((x + offset) % brick_w) / brick_w;
    let by = (y % brick_h) / brick_h;
    let mortar_x = mortar / brick_w;
    let mortar_y = mortar / brick_h;

    if bx < mortar_x || bx > (1.0 - mortar_x) || by < mortar_y || by > (1.0 - mortar_y) {
        -1.0
    } else {
        let scale = params.noise_scale as f64;
        let brick_id = (row as f64) * 100.0 + ((x + offset) / brick_w).floor();
        let base_variation = perlin.get([brick_id * 0.1, 0.0]) * 0.5;
        let surface = fbm(perlin, x * scale, y * scale, params.noise_octaves);
        base_variation + surface * 0.5
    }
}

fn apply_speckle(params: &TexParams, perlin: &Perlin, base: [f64; 3], x: f64, y: f64) -> [f64; 3] {
    if params.speckle_density <= 0.0 {
        return base;
    }
    let hash = perlin.get([x * 1.731, y * 2.399]);
    if hash > 1.0 - params.speckle_density as f64 * 2.0 {
        [
            params.speckle_color[0] as f64,
            params.speckle_color[1] as f64,
            params.speckle_color[2] as f64,
        ]
    } else {
        base
    }
}

/// Compute bevel using minimum-distance-to-void with smooth normal blending.
///
/// Instead of summing independent edge contributions (which breaks at corners),
/// we find the nearest void boundary and compute a single surface normal there.
/// At outer corners, normals blend smoothly to create proper diagonal mitering.
fn compute_bevel(edges: &Edges, x: f64, y: f64, params: &TexParams, light_rad: f64) -> f64 {
    let w = params.bevel_width as f64;
    let size = TILE_SIZE as f64;
    let light_x = light_rad.cos();
    let light_y = -light_rad.sin();

    // Collect distances to each exposed edge/corner, with outward normals.
    // We'll blend the normals weighted by proximity.
    let mut total_weight = 0.0;
    let mut nx = 0.0;
    let mut ny = 0.0;
    let mut min_dist = f64::MAX;

    // Cardinal edges
    if edges.n && y < w {
        let d = y;
        if d < min_dist { min_dist = d; }
        // Weight: stronger when closer to this edge (inverse distance, softened)
        let weight = 1.0 / (d + 0.5);
        nx += 0.0 * weight;
        ny += -1.0 * weight;
        total_weight += weight;
    }
    if edges.s && (size - 1.0 - y) < w {
        let d = size - 1.0 - y;
        if d < min_dist { min_dist = d; }
        let weight = 1.0 / (d + 0.5);
        nx += 0.0 * weight;
        ny += 1.0 * weight;
        total_weight += weight;
    }
    if edges.w && x < w {
        let d = x;
        if d < min_dist { min_dist = d; }
        let weight = 1.0 / (d + 0.5);
        nx += -1.0 * weight;
        ny += 0.0 * weight;
        total_weight += weight;
    }
    if edges.e && (size - 1.0 - x) < w {
        let d = size - 1.0 - x;
        if d < min_dist { min_dist = d; }
        let weight = 1.0 / (d + 0.5);
        nx += 1.0 * weight;
        ny += 0.0 * weight;
        total_weight += weight;
    }

    // Inner corners (concave): diagonal neighbor is void but both adjacent cardinals are wall.
    // The void is at the corner point — distance is to the corner.
    let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
    if edges.inner_nw {
        let d = (x * x + y * y).sqrt();
        if d < w * 1.4 {
            if d < min_dist { min_dist = d; }
            let weight = 1.0 / (d + 0.5);
            nx += -inv_sqrt2 * weight;
            ny += -inv_sqrt2 * weight;
            total_weight += weight;
        }
    }
    if edges.inner_ne {
        let dx = size - 1.0 - x;
        let d = (dx * dx + y * y).sqrt();
        if d < w * 1.4 {
            if d < min_dist { min_dist = d; }
            let weight = 1.0 / (d + 0.5);
            nx += inv_sqrt2 * weight;
            ny += -inv_sqrt2 * weight;
            total_weight += weight;
        }
    }
    if edges.inner_sw {
        let dy = size - 1.0 - y;
        let d = (x * x + dy * dy).sqrt();
        if d < w * 1.4 {
            if d < min_dist { min_dist = d; }
            let weight = 1.0 / (d + 0.5);
            nx += -inv_sqrt2 * weight;
            ny += inv_sqrt2 * weight;
            total_weight += weight;
        }
    }
    if edges.inner_se {
        let dx = size - 1.0 - x;
        let dy = size - 1.0 - y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < w * 1.4 {
            if d < min_dist { min_dist = d; }
            let weight = 1.0 / (d + 0.5);
            nx += inv_sqrt2 * weight;
            ny += inv_sqrt2 * weight;
            total_weight += weight;
        }
    }

    if total_weight == 0.0 || min_dist >= w {
        return 0.0;
    }

    // Normalize the blended normal
    nx /= total_weight;
    ny /= total_weight;
    let len = (nx * nx + ny * ny).sqrt();
    if len < 0.001 {
        return 0.0;
    }
    nx /= len;
    ny /= len;

    // Bevel strength from distance (sqrt falloff = stays strong, drops near inner edge)
    let t = (1.0 - min_dist / w).sqrt();

    // Lighting: dot product of surface normal with light direction
    let lighting = nx * light_x + ny * light_y;

    (t * lighting).clamp(-1.0, 1.0)
}

fn apply_bevel(color: [f64; 3], bevel: f64, params: &TexParams) -> [f64; 3] {
    if bevel > 0.0 {
        // Highlight: lerp toward white
        let f = bevel * params.highlight_strength as f64;
        [
            lerp(color[0], 1.0, f),
            lerp(color[1], 1.0, f),
            lerp(color[2], 1.0, f),
        ]
    } else {
        // Shadow: lerp toward black
        let f = -bevel * params.shadow_strength as f64;
        [
            lerp(color[0], 0.0, f),
            lerp(color[1], 0.0, f),
            lerp(color[2], 0.0, f),
        ]
    }
}

// --- Math primitives ---

fn fbm<N: NoiseFn<f64, 2>>(noise: &N, x: f64, y: f64, octaves: u32) -> f64 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_amp = 0.0;
    for _ in 0..octaves {
        value += noise.get([x * frequency, y * frequency]) * amplitude;
        max_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    value / max_amp
}

fn cellular_noise(x: f64, y: f64) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let mut min_dist = f64::MAX;
    let mut second_dist = f64::MAX;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cx = ix + dx;
            let cy = iy + dy;
            let px = cx as f64 + hash2d(cx, cy, 0) * 0.8 + 0.1;
            let py = cy as f64 + hash2d(cx, cy, 1) * 0.8 + 0.1;
            let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if dist < min_dist {
                second_dist = min_dist;
                min_dist = dist;
            } else if dist < second_dist {
                second_dist = dist;
            }
        }
    }
    (second_dist - min_dist).clamp(0.0, 1.0) * 2.0 - 1.0
}

fn hash2d(x: i32, y: i32, seed: i32) -> f64 {
    let mut h = (x.wrapping_mul(374761393))
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(seed.wrapping_mul(1274126177));
    h = (h ^ (h >> 13)).wrapping_mul(1103515245);
    h = h ^ (h >> 16);
    (h & 0x7FFFFFFF) as f64 / 0x7FFFFFFF as f64
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}
