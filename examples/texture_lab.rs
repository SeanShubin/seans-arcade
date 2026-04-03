//! Interactive 47-blob texture tweaker.
//!
//! Renders all 47 blob tiles using procedural textures with adjustable parameters.
//! Uses the standard LDtk blob layout (12×5 grid) with 2px gaps between tiles.
//!
//! Usage:
//!   cargo run --example texture_lab                        # interactive
//!   cargo run --example texture_lab -- --render            # export PNG
//!   cargo run --example texture_lab -- --render out.png    # export to path
//!   cargo run --example texture_lab -- --preset Concrete --render
//!   cargo run --example texture_lab -- --render --base-color 0.5,0.5,0.55 \
//!     --bevel-width 16 --3d-lighting --edge-lines
//!
//! Controls:
//!   Scroll wheel — zoom in/out
//!   Middle mouse drag — pan
//!   Arrow keys — pan
//!   Left panel — tweak material parameters

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use noise::{NoiseFn, OpenSimplex, Perlin};

// =====================================================================
// Blob layout — the 47 standard autotile masks in a 12×5 grid
// =====================================================================

const N: u8 = 1;
const NE: u8 = 2;
const E: u8 = 4;
const SE: u8 = 8;
const S: u8 = 16;
const SW: u8 = 32;
const W: u8 = 64;
const NW: u8 = 128;

const GRID_COLS: u32 = 12;
const GRID_ROWS: u32 = 5;
const TILE_SIZE: u32 = 64;
const TILE_GAP: u32 = 2;

const BLOB_LAYOUT: [(u32, u32, u8); 47] = [
    (0, 0, 28),  (1, 0, 124), (2, 0, 112), (3, 0, 4),   (4, 0, 68),
    (0, 1, 31),  (1, 1, 255), (2, 1, 241), (3, 1, 0),   (4, 1, 16),
    (0, 2, 7),   (1, 2, 199), (2, 2, 193), (3, 2, 85),  (4, 2, 17),
    (0, 3, 127), (1, 3, 253), (2, 3, 20),  (3, 3, 80),  (4, 3, 1),
    (0, 4, 223), (1, 4, 247), (2, 4, 5),   (3, 4, 65),
    (5, 0, 64),  (6, 0, 92),  (7, 0, 116), (8, 0, 95),  (9, 0, 245),
    (10, 0, 93), (11, 0, 117),
    (6, 1, 71),  (7, 1, 197), (8, 1, 215), (9, 1, 125), (10, 1, 87),
    (11, 1, 213),
    (6, 2, 29),  (7, 2, 113), (8, 2, 21),  (9, 2, 84),  (10, 2, 119),
    (6, 3, 23),  (7, 3, 209), (8, 3, 69),  (9, 3, 81),  (10, 3, 221),
];

// =====================================================================
// 3D lighting constants (matching beveled_block)
// =====================================================================

const BEVEL_DEPTH: f64 = 10.0;
const AMBIENT: f64 = 0.25;
const OVERHEAD_LIGHT_Z: f64 = 2.0;
const EDGE_LINE_HALF_WIDTH: f64 = 0.5;

// =====================================================================
// App entry point
// =====================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let render_mode = args.iter().any(|a| a == "--render");
    let params = parse_params_from_args(&args);

    if render_mode {
        let render_idx = args.iter().position(|a| a == "--render").unwrap();
        let output_path = args.get(render_idx + 1)
            .filter(|a| !a.starts_with("--"))
            .map(|s| s.as_str())
            .unwrap_or("assets/generated/texture_lab.png");
        render_and_exit(&params, output_path);
    } else {
        run_interactive(params);
    }
}

fn render_and_exit(params: &TexParams, output_path: &str) {
    let pixels_per_unit = 1.0;
    let pixels = render_all_tiles(params, pixels_per_unit);
    let img = image::RgbaImage::from_raw(
        scaled_image_width(pixels_per_unit),
        scaled_image_height(pixels_per_unit),
        pixels,
    ).expect("Failed to create image buffer");
    let dir = std::path::Path::new(output_path).parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir).ok();
    img.save(output_path).expect("Failed to save image");
    println!("Rendered: {}", output_path);
}

fn run_interactive(params: TexParams) {
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
        .insert_resource(TexDirty(true))
        .add_systems(Startup, spawn_camera_and_tileset)
        .add_systems(bevy_egui::EguiPrimaryContextPass, parameter_ui)
        .add_systems(Update, (regenerate_tileset, camera_zoom, camera_pan))
        .run();
}

// =====================================================================
// CLI argument parsing
// =====================================================================

fn parse_params_from_args(args: &[String]) -> TexParams {
    // Start from preset if specified, otherwise default
    let mut params = if let Some(pos) = args.iter().position(|a| a == "--preset") {
        let name = args.get(pos + 1).expect("--preset requires a name");
        PRESETS.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                let names: Vec<_> = PRESETS.iter().map(|(n, _)| *n).collect();
                panic!("Unknown preset '{}'. Available: {:?}", name, names);
            })
            .1()
    } else {
        TexParams::default()
    };

    fn arg_f32(args: &[String], name: &str) -> Option<f32> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)?.parse().ok())
    }
    fn arg_u32(args: &[String], name: &str) -> Option<u32> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)?.parse().ok())
    }
    fn arg_usize(args: &[String], name: &str) -> Option<usize> {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)?.parse().ok())
    }
    fn arg_rgb(args: &[String], name: &str) -> Option<[f32; 3]> {
        let pos = args.iter().position(|a| a == name)?;
        let val = args.get(pos + 1)?;
        let parts: Vec<f32> = val.split(',').filter_map(|s| s.parse().ok()).collect();
        if parts.len() == 3 { Some([parts[0], parts[1], parts[2]]) } else { None }
    }
    fn arg_flag(args: &[String], name: &str) -> bool {
        args.iter().any(|a| a == name)
    }

    if let Some(v) = arg_rgb(args, "--base-color") { params.base_color = v; }
    if let Some(v) = arg_f32(args, "--color-variation") { params.color_variation = v; }
    if let Some(v) = arg_f32(args, "--noise-scale") { params.noise_scale = v; }
    if let Some(v) = arg_u32(args, "--noise-octaves") { params.noise_octaves = v; }
    if let Some(v) = arg_usize(args, "--pattern") { params.pattern = v; }
    if let Some(v) = arg_f32(args, "--bevel-fraction") { params.bevel_fraction = v; }
    if let Some(v) = arg_f32(args, "--shadow-strength") { params.shadow_strength = v; }
    if let Some(v) = arg_f32(args, "--highlight-strength") { params.highlight_strength = v; }
    if let Some(v) = arg_f32(args, "--light-angle") { params.light_angle = v; }
    if let Some(v) = arg_f32(args, "--speckle-density") { params.speckle_density = v; }
    if let Some(v) = arg_rgb(args, "--speckle-color") { params.speckle_color = v; }
    if let Some(v) = arg_rgb(args, "--secondary-color") { params.secondary_color = v; params.use_secondary = true; }
    if let Some(v) = arg_f32(args, "--stripe-angle") { params.stripe_angle = v; }
    if let Some(v) = arg_u32(args, "--seed") { params.seed = v; }
    if arg_flag(args, "--3d-lighting") { params.use_3d_lighting = true; }
    if arg_flag(args, "--edge-lines") { params.show_edge_lines = true; }
    if arg_flag(args, "--no-textures") { params.procedural_textures = false; }

    params
}

// =====================================================================
// Resources
// =====================================================================

#[derive(Resource)]
struct TexParams {
    base_color: [f32; 3],
    color_variation: f32,
    noise_scale: f32,
    noise_octaves: u32,
    pattern: usize,
    bevel_fraction: f32,
    shadow_strength: f32,
    highlight_strength: f32,
    light_angle: f32,
    speckle_density: f32,
    speckle_color: [f32; 3],
    use_secondary: bool,
    secondary_color: [f32; 3],
    stripe_angle: f32,
    seed: u32,
    use_3d_lighting: bool,
    show_edge_lines: bool,
    procedural_textures: bool,
}

impl Default for TexParams {
    fn default() -> Self {
        Self {
            base_color: [0.62, 0.62, 0.62],
            color_variation: 0.06,
            noise_scale: 0.08,
            noise_octaves: 3,
            pattern: 0,
            bevel_fraction: 0.22,
            shadow_strength: 0.7,
            highlight_strength: 0.4,
            light_angle: 135.0,
            speckle_density: 0.0,
            speckle_color: [1.0, 1.0, 1.0],
            use_secondary: false,
            secondary_color: [0.3, 0.3, 0.3],
            stripe_angle: 0.0,
            seed: 42,
            use_3d_lighting: false,
            show_edge_lines: false,
            procedural_textures: true,
        }
    }
}

const PATTERN_NAMES: &[&str] = &["Perlin", "Cellular", "Brick", "Stripe", "Marble"];

const PRESETS: &[(&str, fn() -> TexParams)] = &[
    ("Beveled Block", || TexParams {
        base_color: [0.50, 0.50, 0.55],
        color_variation: 0.0,
        noise_scale: 0.08,
        noise_octaves: 1,
        pattern: 0,
        bevel_fraction: 0.25,
        shadow_strength: 0.7,
        highlight_strength: 0.4,
        light_angle: 202.5,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: true,
        show_edge_lines: true,
        procedural_textures: false,
    }),
    ("Concrete", || TexParams {
        base_color: [0.62, 0.62, 0.62],
        color_variation: 0.06,
        noise_scale: 0.08,
        noise_octaves: 3,
        pattern: 0,
        bevel_fraction: 0.22,
        shadow_strength: 0.7,
        highlight_strength: 0.4,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Red Brick", || TexParams {
        base_color: [0.6, 0.25, 0.18],
        color_variation: 0.06,
        noise_scale: 0.1,
        noise_octaves: 2,
        pattern: 2,
        bevel_fraction: 0.06,
        shadow_strength: 0.4,
        highlight_strength: 0.2,
        light_angle: 135.0,
        speckle_density: 0.08,
        speckle_color: [0.85, 0.85, 0.8],
        use_secondary: true,
        secondary_color: [0.78, 0.75, 0.65],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Dark Stone", || TexParams {
        base_color: [0.3, 0.3, 0.32],
        color_variation: 0.1,
        noise_scale: 0.04,
        noise_octaves: 4,
        pattern: 1,
        bevel_fraction: 0.09,
        shadow_strength: 0.5,
        highlight_strength: 0.2,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.18, 0.18, 0.2],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Marble", || TexParams {
        base_color: [0.88, 0.86, 0.82],
        color_variation: 0.15,
        noise_scale: 0.03,
        noise_octaves: 4,
        pattern: 4,
        bevel_fraction: 0.06,
        shadow_strength: 0.3,
        highlight_strength: 0.5,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.4, 0.35, 0.3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Wood Plank", || TexParams {
        base_color: [0.55, 0.38, 0.22],
        color_variation: 0.12,
        noise_scale: 0.06,
        noise_octaves: 3,
        pattern: 3,
        bevel_fraction: 0.08,
        shadow_strength: 0.35,
        highlight_strength: 0.15,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: true,
        secondary_color: [0.42, 0.28, 0.15],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Blue Tile", || TexParams {
        base_color: [0.2, 0.35, 0.6],
        color_variation: 0.03,
        noise_scale: 0.15,
        noise_octaves: 1,
        pattern: 0,
        bevel_fraction: 0.06,
        shadow_strength: 0.5,
        highlight_strength: 0.7,
        light_angle: 135.0,
        speckle_density: 0.0,
        speckle_color: [1.0; 3],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Sandstone", || TexParams {
        base_color: [0.72, 0.62, 0.45],
        color_variation: 0.1,
        noise_scale: 0.07,
        noise_octaves: 3,
        pattern: 0,
        bevel_fraction: 0.08,
        shadow_strength: 0.35,
        highlight_strength: 0.15,
        light_angle: 135.0,
        speckle_density: 0.04,
        speckle_color: [0.85, 0.78, 0.6],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
    ("Metal Plate", || TexParams {
        base_color: [0.5, 0.52, 0.55],
        color_variation: 0.02,
        noise_scale: 0.2,
        noise_octaves: 1,
        pattern: 0,
        bevel_fraction: 0.05,
        shadow_strength: 0.5,
        highlight_strength: 0.8,
        light_angle: 135.0,
        speckle_density: 0.02,
        speckle_color: [0.7, 0.72, 0.75],
        use_secondary: false,
        secondary_color: [0.3; 3],
        stripe_angle: 0.0,
        seed: 42,
        use_3d_lighting: false,
        show_edge_lines: false,
        procedural_textures: true,
    }),
];

#[derive(Resource)]
struct TexDirty(bool);

#[derive(Resource)]
struct TilesetImageHandle(Handle<Image>);

#[derive(Resource)]
struct LastRenderScale(f32);

// =====================================================================
// Image dimensions with tile gaps
// =====================================================================

fn image_width() -> u32 {
    GRID_COLS * TILE_SIZE + (GRID_COLS - 1) * TILE_GAP
}

fn image_height() -> u32 {
    GRID_ROWS * TILE_SIZE + (GRID_ROWS - 1) * TILE_GAP
}

// =====================================================================
// Startup system
// =====================================================================

fn spawn_camera_and_tileset(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    params: Res<TexParams>,
) {
    commands.spawn(Camera2d);

    let pixels_per_unit = 1.0;
    let pixels = render_all_tiles(&params, pixels_per_unit);
    let image = create_tileset_image_scaled(pixels, pixels_per_unit);
    let handle = images.add(image);

    commands.spawn(Sprite {
        image: handle.clone(),
        custom_size: Some(Vec2::new(image_width() as f32, image_height() as f32)),
        ..default()
    });
    commands.insert_resource(TilesetImageHandle(handle));
    commands.insert_resource(LastRenderScale(1.0));
}

fn create_tileset_image_scaled(pixels: Vec<u8>, pixels_per_unit: f64) -> Image {
    let w = scaled_image_width(pixels_per_unit);
    let h = scaled_image_height(pixels_per_unit);
    Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    )
}

fn scaled_image_width(pixels_per_unit: f64) -> u32 {
    ((image_width() as f64) * pixels_per_unit).ceil() as u32
}

fn scaled_image_height(pixels_per_unit: f64) -> u32 {
    ((image_height() as f64) * pixels_per_unit).ceil() as u32
}

// =====================================================================
// Parameter UI
// =====================================================================

fn parameter_ui(
    mut contexts: EguiContexts,
    mut params: ResMut<TexParams>,
    mut dirty: ResMut<TexDirty>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("controls").min_width(260.0).show(ctx, |ui| {
        ui.heading("Texture Lab");
        ui.separator();

        let changed = show_preset_buttons(ui, &mut params)
            | show_pattern_controls(ui, &mut params)
            | show_color_controls(ui, &mut params)
            | show_noise_controls(ui, &mut params)
            | show_bevel_controls(ui, &mut params)
            | show_speckle_controls(ui, &mut params)
            | show_rendering_controls(ui, &mut params);

        if changed {
            dirty.0 = true;
        }

        ui.separator();
        ui.label("Scroll to zoom, middle-drag to pan");
    });
}

fn show_preset_buttons(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    ui.label("Presets");
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for &(name, make_fn) in PRESETS {
            if ui.button(name).clicked() {
                **params = make_fn();
                changed = true;
            }
        }
    });
    ui.separator();
    changed
}

fn show_pattern_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
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
    changed
}

fn show_color_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    let mut changed = false;
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
    changed
}

fn show_noise_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    let mut changed = false;
    ui.label("Noise");
    changed |= ui
        .add(egui::Slider::new(&mut params.noise_scale, 0.01..=0.5).text("Scale"))
        .changed();
    let mut octaves = params.noise_octaves as i32;
    if ui.add(egui::Slider::new(&mut octaves, 1..=6).text("Octaves")).changed() {
        params.noise_octaves = octaves as u32;
        changed = true;
    }
    let mut seed = params.seed as i32;
    if ui.add(egui::Slider::new(&mut seed, 0..=999).text("Seed")).changed() {
        params.seed = seed as u32;
        changed = true;
    }
    ui.separator();
    changed
}

fn show_bevel_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    let mut changed = false;
    ui.label("Bevel");
    changed |= ui
        .add(egui::Slider::new(&mut params.bevel_fraction, 0.0..=0.5).text("Bevel"))
        .changed();

    changed |= ui.checkbox(&mut params.use_3d_lighting, "3D lighting").changed();

    if !params.use_3d_lighting {
        changed |= ui
            .add(egui::Slider::new(&mut params.shadow_strength, 0.0..=1.0).text("Shadow"))
            .changed();
        changed |= ui
            .add(egui::Slider::new(&mut params.highlight_strength, 0.0..=1.0).text("Highlight"))
            .changed();
    }

    changed |= ui
        .add(egui::Slider::new(&mut params.light_angle, 0.0..=360.0).text("Light angle°"))
        .changed();

    changed |= ui.checkbox(&mut params.show_edge_lines, "Edge lines").changed();

    ui.separator();
    changed
}

fn show_speckle_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    let mut changed = false;
    ui.label("Speckle");
    changed |= ui
        .add(egui::Slider::new(&mut params.speckle_density, 0.0..=0.3).text("Density"))
        .changed();
    if params.speckle_density > 0.0 {
        changed |= ui.color_edit_button_rgb(&mut params.speckle_color).changed();
    }
    changed
}

fn show_rendering_controls(ui: &mut egui::Ui, params: &mut ResMut<TexParams>) -> bool {
    let mut changed = false;
    ui.separator();
    ui.label("Rendering");
    changed |= ui.checkbox(&mut params.procedural_textures, "Procedural textures").changed();
    changed
}

// =====================================================================
// Regenerate tileset when zoom or parameters change
// =====================================================================

fn regenerate_tileset(
    mut dirty: ResMut<TexDirty>,
    params: Res<TexParams>,
    tileset_handle: Res<TilesetImageHandle>,
    mut last_scale: ResMut<LastRenderScale>,
    mut images: ResMut<Assets<Image>>,
    camera_q: Query<&Projection, With<Camera2d>>,
    mut sprite_q: Query<&mut Sprite>,
) {
    let ortho_scale = camera_q.iter().find_map(|p| {
        if let Projection::Orthographic(o) = p { Some(o.scale) } else { None }
    }).unwrap_or(1.0);

    let scale_changed = (ortho_scale - last_scale.0).abs() > 0.001;
    if !dirty.0 && !scale_changed {
        return;
    }
    dirty.0 = false;
    last_scale.0 = ortho_scale;

    let pixels_per_unit = 1.0 / ortho_scale as f64;
    let new_w = scaled_image_width(pixels_per_unit);
    let new_h = scaled_image_height(pixels_per_unit);
    let pixels = render_all_tiles(&params, pixels_per_unit);

    if let Some(image) = images.get_mut(&tileset_handle.0) {
        image.resize(Extent3d {
            width: new_w,
            height: new_h,
            depth_or_array_layers: 1,
        });
        image.data = Some(pixels);
    }

    // Keep sprite at the same world size regardless of pixel resolution
    let world_size = Vec2::new(image_width() as f32, image_height() as f32);
    for mut sprite in &mut sprite_q {
        sprite.custom_size = Some(world_size);
    }
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
    let Ok((mut transform, projection)) = query.single_mut() else {
        return;
    };
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
// Tile rendering — produces RGBA pixel buffer for the full grid with gaps
// =====================================================================

fn render_all_tiles(params: &TexParams, pixels_per_unit: f64) -> Vec<u8> {
    let img_w = scaled_image_width(pixels_per_unit);
    let img_h = scaled_image_height(pixels_per_unit);
    let mut pixels = vec![0u8; (img_w * img_h * 4) as usize];

    let perlin = Perlin::new(params.seed);
    let simplex = OpenSimplex::new(params.seed.wrapping_add(81));

    let tile_px = (TILE_SIZE as f64 * pixels_per_unit).ceil() as u32;
    let gap_px = (TILE_GAP as f64 * pixels_per_unit).ceil() as u32;

    for &(col, row, mask) in &BLOB_LAYOUT {
        let edges = edges_from_blob_mask(mask);
        let origin_x = col * (tile_px + gap_px);
        let origin_y = row * (tile_px + gap_px);
        render_single_tile(
            &mut pixels, img_w,
            origin_x, origin_y, tile_px,
            pixels_per_unit,
            &edges, params, &perlin, &simplex,
        );
    }

    pixels
}

fn render_single_tile(
    pixels: &mut [u8],
    img_w: u32,
    origin_x: u32, origin_y: u32,
    tile_px: u32,
    pixels_per_unit: f64,
    edges: &BevelEdges,
    params: &TexParams,
    perlin: &Perlin,
    simplex: &OpenSimplex,
) {
    let bevel = params.bevel_fraction as f64 * TILE_SIZE as f64;
    let size = TILE_SIZE as f64;
    let tile_colors = TileBevelColors::new(edges, params);
    let step = 1.0 / pixels_per_unit;

    for py in 0..tile_px {
        for px in 0..tile_px {
            // Map output pixel back to tile-local coordinates [0, TILE_SIZE)
            let local_x = px as f64 * step;
            let local_y = py as f64 * step;
            // World coordinates for noise sampling (tile origin in world units + local)
            let world_x = (origin_x as f64 * step) + local_x;
            let world_y = (origin_y as f64 * step) + local_y;

            let texture_color = if params.procedural_textures {
                sample_textured_pixel(params, perlin, simplex, world_x, world_y)
            } else {
                [params.base_color[0] as f64, params.base_color[1] as f64, params.base_color[2] as f64]
            };
            let brightness = rasterize_bevel_brightness(&tile_colors, edges, local_x, local_y, bevel, size);
            let lit_color = apply_brightness(texture_color, brightness, params.use_3d_lighting);
            let final_color = if params.show_edge_lines {
                apply_edge_line_overlay(lit_color, edges, local_x, local_y, bevel)
            } else {
                lit_color
            };

            let img_x = origin_x + px;
            let img_y = origin_y + py;
            if img_x < img_w {
                let idx = (img_y * img_w + img_x) as usize * 4;
                if idx + 3 < pixels.len() {
                    pixels[idx] = (final_color[0].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 1] = (final_color[1].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 2] = (final_color[2].clamp(0.0, 1.0) * 255.0) as u8;
                    pixels[idx + 3] = 255;
                }
            }
        }
    }
}

fn sample_textured_pixel(
    params: &TexParams,
    perlin: &Perlin,
    simplex: &OpenSimplex,
    world_x: f64,
    world_y: f64,
) -> [f64; 3] {
    let base = sample_pattern(params, perlin, simplex, world_x, world_y);
    apply_speckle(params, perlin, base, world_x, world_y)
}

// =====================================================================
// Bevel edge detection from blob mask
// =====================================================================

struct BevelEdges {
    n: bool, s: bool, e: bool, w: bool,
    inner_nw: bool, inner_ne: bool, inner_sw: bool, inner_se: bool,
}

fn edges_from_blob_mask(mask: u8) -> BevelEdges {
    let has = |bit: u8| mask & bit != 0;
    BevelEdges {
        n: !has(N),
        s: !has(S),
        e: !has(E),
        w: !has(W),
        inner_nw: has(N) && has(W) && !has(NW),
        inner_ne: has(N) && has(E) && !has(NE),
        inner_sw: has(S) && has(W) && !has(SW),
        inner_se: has(S) && has(E) && !has(SE),
    }
}

// =====================================================================
// Software rasterizer — determines bevel brightness per pixel by
// identifying which bevel quad/triangle the pixel falls in and
// bilinearly interpolating precomputed vertex brightnesses.
// =====================================================================

/// Precomputed brightness values for all bevel directions and the face.
struct TileBevelColors {
    face: f64,
    // Cardinal bevel brightnesses (used for inner edges and concave corners)
    top: f64,
    bottom: f64,
    left: f64,
    right: f64,
    // Vertex brightnesses for each cardinal bevel quad's outer edge
    north_left: f64,
    north_right: f64,
    south_left: f64,
    south_right: f64,
    west_top: f64,
    west_bottom: f64,
    east_top: f64,
    east_bottom: f64,
}

impl TileBevelColors {
    fn new(edges: &BevelEdges, params: &TexParams) -> Self {
        if params.use_3d_lighting {
            Self::from_3d_lighting(edges, params)
        } else {
            Self::from_2d_lighting(edges, params)
        }
    }

    fn from_3d_lighting(edges: &BevelEdges, params: &TexParams) -> Self {
        let bevel_width = params.bevel_fraction as f64 * TILE_SIZE as f64;
        let bevel_angle = (BEVEL_DEPTH / bevel_width).atan();
        let bevel_sin = bevel_angle.sin();
        let bevel_cos = bevel_angle.cos();

        let light_rad = (params.light_angle as f64).to_radians();
        let raw = (light_rad.cos(), -light_rad.sin(), OVERHEAD_LIGHT_Z);
        let len = (raw.0 * raw.0 + raw.1 * raw.1 + raw.2 * raw.2).sqrt();
        let light = (raw.0 / len, raw.1 / len, raw.2 / len);

        let brightness_3d = |dir_x: f64, dir_y: f64| -> f64 {
            let nx = dir_x * bevel_sin;
            let ny = dir_y * bevel_sin;
            let nz = bevel_cos;
            let nlen = (nx * nx + ny * ny + nz * nz).sqrt();
            let diffuse = ((nx / nlen) * light.0 + (ny / nlen) * light.1 + (nz / nlen) * light.2).max(0.0);
            AMBIENT + (1.0 - AMBIENT) * diffuse
        };

        let face = {
            let diffuse = light.2.max(0.0);
            AMBIENT + (1.0 - AMBIENT) * diffuse
        };
        // Directions in Y-up world space (matching beveled_block's coordinate system)
        let top = brightness_3d(0.0, 1.0);
        let bottom = brightness_3d(0.0, -1.0);
        let left = brightness_3d(-1.0, 0.0);
        let right = brightness_3d(1.0, 0.0);
        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        let top_left = brightness_3d(-inv_sqrt2, inv_sqrt2);
        let top_right = brightness_3d(inv_sqrt2, inv_sqrt2);
        let bottom_left = brightness_3d(-inv_sqrt2, -inv_sqrt2);
        let bottom_right = brightness_3d(inv_sqrt2, -inv_sqrt2);

        Self {
            face, top, bottom, left, right,
            north_left: if edges.w { top_left } else { top },
            north_right: if edges.e { top_right } else { top },
            south_left: if edges.w { bottom_left } else { bottom },
            south_right: if edges.e { bottom_right } else { bottom },
            west_top: if edges.n { top_left } else { left },
            west_bottom: if edges.s { bottom_left } else { left },
            east_top: if edges.n { top_right } else { right },
            east_bottom: if edges.s { bottom_right } else { right },
        }
    }

    fn from_2d_lighting(edges: &BevelEdges, params: &TexParams) -> Self {
        let light_rad = (params.light_angle as f64).to_radians();
        let lx = light_rad.cos();
        let ly = -light_rad.sin();
        let shadow = params.shadow_strength as f64;
        let highlight = params.highlight_strength as f64;

        // For 2D mode, brightness encodes highlight/shadow as offset from 1.0:
        //   brightness > 1.0 = highlight (lerp toward white)
        //   brightness < 1.0 = shadow (lerp toward black)
        //   brightness = 1.0 = no bevel effect (face)
        let bevel_2d = |dir_x: f64, dir_y: f64| -> f64 {
            let dot = dir_x * lx + dir_y * ly;
            if dot > 0.0 {
                1.0 + dot * highlight
            } else {
                1.0 + dot * shadow
            }
        };

        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
        let top = bevel_2d(0.0, -1.0);
        let bottom = bevel_2d(0.0, 1.0);
        let left = bevel_2d(-1.0, 0.0);
        let right = bevel_2d(1.0, 0.0);
        let top_left = bevel_2d(-inv_sqrt2, -inv_sqrt2);
        let top_right = bevel_2d(inv_sqrt2, -inv_sqrt2);
        let bottom_left = bevel_2d(-inv_sqrt2, inv_sqrt2);
        let bottom_right = bevel_2d(inv_sqrt2, inv_sqrt2);

        Self {
            face: 1.0, top, bottom, left, right,
            north_left: if edges.w { top_left } else { top },
            north_right: if edges.e { top_right } else { top },
            south_left: if edges.w { bottom_left } else { bottom },
            south_right: if edges.e { bottom_right } else { bottom },
            west_top: if edges.n { top_left } else { left },
            west_bottom: if edges.s { bottom_left } else { left },
            east_top: if edges.n { top_right } else { right },
            east_bottom: if edges.s { bottom_right } else { right },
        }
    }
}

/// Determine which bevel region a pixel falls in and return the interpolated
/// brightness. Layers are applied in the same order as beveled_block's mesh
/// spawning: face first, then cardinal bevels (N, S, W, E — last overwrites
/// at corner overlaps), then concave corners on top.
fn north_bevel(colors: &TileBevelColors, px: f64, py: f64, bevel: f64, size: f64) -> f64 {
    let t = py / bevel;
    let s = px / (size - 1.0);
    let outer = lerp(colors.north_left, colors.north_right, s);
    lerp(outer, colors.top, t)
}

fn south_bevel(colors: &TileBevelColors, px: f64, py: f64, bevel: f64, size: f64) -> f64 {
    let t = (size - 1.0 - py) / bevel;
    let s = px / (size - 1.0);
    let outer = lerp(colors.south_left, colors.south_right, s);
    lerp(outer, colors.bottom, t)
}

fn west_bevel(colors: &TileBevelColors, px: f64, py: f64, bevel: f64, size: f64) -> f64 {
    let t = px / bevel;
    let s = py / (size - 1.0);
    let outer = lerp(colors.west_top, colors.west_bottom, s);
    lerp(outer, colors.left, t)
}

fn east_bevel(colors: &TileBevelColors, px: f64, py: f64, bevel: f64, size: f64) -> f64 {
    let t = (size - 1.0 - px) / bevel;
    let s = py / (size - 1.0);
    let outer = lerp(colors.east_top, colors.east_bottom, s);
    lerp(outer, colors.right, t)
}

fn rasterize_bevel_brightness(
    colors: &TileBevelColors,
    edges: &BevelEdges,
    px: f64, py: f64,
    bevel: f64, size: f64,
) -> f64 {
    let mut brightness = colors.face;

    let in_n = edges.n && py < bevel;
    let in_s = edges.s && py >= size - bevel;
    let in_w = edges.w && px < bevel;
    let in_e = edges.e && px >= size - bevel;

    // Cardinal bevels with diagonal splits at convex corners.
    // Each bevel quad owns one half of the corner — the diagonal runs
    // from the tile's outer corner to the inner face corner.
    if in_n && in_w {
        brightness = if py < px { north_bevel(colors, px, py, bevel, size) }
                     else { west_bevel(colors, px, py, bevel, size) };
    } else if in_n && in_e {
        brightness = if py < size - px { north_bevel(colors, px, py, bevel, size) }
                     else { east_bevel(colors, px, py, bevel, size) };
    } else if in_s && in_w {
        brightness = if py >= size - px { south_bevel(colors, px, py, bevel, size) }
                     else { west_bevel(colors, px, py, bevel, size) };
    } else if in_s && in_e {
        brightness = if py >= px { south_bevel(colors, px, py, bevel, size) }
                     else { east_bevel(colors, px, py, bevel, size) };
    } else if in_n {
        brightness = north_bevel(colors, px, py, bevel, size);
    } else if in_s {
        brightness = south_bevel(colors, px, py, bevel, size);
    } else if in_w {
        brightness = west_bevel(colors, px, py, bevel, size);
    } else if in_e {
        brightness = east_bevel(colors, px, py, bevel, size);
    }

    // Concave corners — flat brightness, diagonal split (higher Z)
    let sx = size - 1.0 - px;
    let sy = size - 1.0 - py;
    if edges.inner_nw && px < bevel && py < bevel {
        brightness = if py >= px { colors.top } else { colors.left };
    }
    if edges.inner_ne && sx < bevel && py < bevel {
        brightness = if py >= sx { colors.top } else { colors.right };
    }
    if edges.inner_sw && px < bevel && sy < bevel {
        brightness = if sy >= px { colors.bottom } else { colors.left };
    }
    if edges.inner_se && sx < bevel && sy < bevel {
        brightness = if sy >= sx { colors.bottom } else { colors.right };
    }

    brightness
}

// =====================================================================
/// Apply brightness to texture color.
/// In 3D mode, brightness is a real multiplier (0.0–1.0).
/// In 2D mode, brightness encodes highlight/shadow as offset from 1.0:
///   >1.0 lerps toward white, <1.0 lerps toward black.
fn apply_brightness(color: [f64; 3], brightness: f64, is_3d: bool) -> [f64; 3] {
    if is_3d {
        [color[0] * brightness, color[1] * brightness, color[2] * brightness]
    } else if brightness > 1.0 {
        let f = brightness - 1.0;
        [lerp(color[0], 1.0, f), lerp(color[1], 1.0, f), lerp(color[2], 1.0, f)]
    } else {
        let f = 1.0 - brightness;
        [lerp(color[0], 0.0, f), lerp(color[1], 0.0, f), lerp(color[2], 0.0, f)]
    }
}

// Edge line overlay
// =====================================================================

fn apply_edge_line_overlay(
    color: [f64; 3],
    edges: &BevelEdges,
    px: f64, py: f64,
    bevel: f64,
) -> [f64; 3] {
    if let Some((overlay_r, overlay_g, overlay_b, alpha)) = find_edge_line(edges, px, py, bevel) {
        let inv = 1.0 - alpha;
        [
            color[0] * inv + overlay_r * alpha,
            color[1] * inv + overlay_g * alpha,
            color[2] * inv + overlay_b * alpha,
        ]
    } else {
        color
    }
}

/// Check if a pixel falls on any edge line. Returns RGBA overlay if so.
/// Checks convex edges first, then concave edges.
fn find_edge_line(
    edges: &BevelEdges,
    px: f64, py: f64,
    bevel: f64,
) -> Option<(f64, f64, f64, f64)> {
    let size = TILE_SIZE as f64;
    let lighter: (f64, f64, f64, f64) = (1.0, 1.0, 1.0, 0.04);
    let darker: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 0.05);

    // Face boundaries
    let face_left = if edges.w { bevel } else { 0.0 };
    let face_right = if edges.e { size - bevel } else { size };
    let face_top = if edges.n { bevel } else { 0.0 };
    let face_bottom = if edges.s { size - bevel } else { size };

    // Convex inner square edges (lighter — top of bevel ridge)
    if edges.n && point_on_horizontal_line(px, py, face_left, face_right, face_top) {
        return Some(lighter);
    }
    if edges.s && point_on_horizontal_line(px, py, face_left, face_right, face_bottom) {
        return Some(lighter);
    }
    if edges.w && point_on_vertical_line(px, py, face_left, face_top, face_bottom) {
        return Some(lighter);
    }
    if edges.e && point_on_vertical_line(px, py, face_right, face_top, face_bottom) {
        return Some(lighter);
    }

    // Convex corner diagonals (darker — where bevel meets face)
    if edges.n && edges.w && point_on_segment(px, py, face_left, face_top, 0.0, 0.0) {
        return Some(darker);
    }
    if edges.n && edges.e && point_on_segment(px, py, face_right, face_top, size - 1.0, 0.0) {
        return Some(darker);
    }
    if edges.s && edges.w && point_on_segment(px, py, face_left, face_bottom, 0.0, size - 1.0) {
        return Some(darker);
    }
    if edges.s && edges.e && point_on_segment(px, py, face_right, face_bottom, size - 1.0, size - 1.0) {
        return Some(darker);
    }

    // Concave edge lines
    if edges.inner_nw {
        let inner_x = bevel;
        let inner_y = bevel;
        if point_on_segment(px, py, inner_x, inner_y, inner_x, 0.0) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, 0.0, inner_y) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, 0.0, 0.0) { return Some(darker); }
    }
    if edges.inner_ne {
        let inner_x = size - bevel;
        let inner_y = bevel;
        if point_on_segment(px, py, inner_x, inner_y, inner_x, 0.0) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, size - 1.0, inner_y) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, size - 1.0, 0.0) { return Some(darker); }
    }
    if edges.inner_sw {
        let inner_x = bevel;
        let inner_y = size - bevel;
        if point_on_segment(px, py, inner_x, inner_y, inner_x, size - 1.0) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, 0.0, inner_y) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, 0.0, size - 1.0) { return Some(darker); }
    }
    if edges.inner_se {
        let inner_x = size - bevel;
        let inner_y = size - bevel;
        if point_on_segment(px, py, inner_x, inner_y, inner_x, size - 1.0) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, size - 1.0, inner_y) { return Some(lighter); }
        if point_on_segment(px, py, inner_x, inner_y, size - 1.0, size - 1.0) { return Some(darker); }
    }

    None
}

fn point_on_horizontal_line(px: f64, py: f64, x_min: f64, x_max: f64, y_line: f64) -> bool {
    px >= x_min && px <= x_max && (py - y_line).abs() < EDGE_LINE_HALF_WIDTH
}

fn point_on_vertical_line(px: f64, py: f64, x_line: f64, y_min: f64, y_max: f64) -> bool {
    py >= y_min && py <= y_max && (px - x_line).abs() < EDGE_LINE_HALF_WIDTH
}

fn point_on_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> bool {
    distance_to_segment(px, py, ax, ay, bx, by) < EDGE_LINE_HALF_WIDTH
}

fn distance_to_segment(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < 0.001 {
        return ((px - ax) * (px - ax) + (py - ay) * (py - ay)).sqrt();
    }
    let t = (((px - ax) * dx + (py - ay) * dy) / len_sq).clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    ((px - proj_x) * (px - proj_x) + (py - proj_y) * (py - proj_y)).sqrt()
}

// =====================================================================
// Material sampling
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
        0 => fbm(perlin, x * scale, y * scale, octaves),
        1 => cellular_noise(x * scale, y * scale),
        2 => brick_pattern(x, y, params, perlin),
        3 => stripe_pattern(x, y, params, perlin, simplex),
        4 => marble_pattern(x, y, params, perlin, simplex),
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

fn stripe_pattern(x: f64, y: f64, params: &TexParams, perlin: &Perlin, simplex: &OpenSimplex) -> f64 {
    let scale = params.noise_scale as f64;
    let angle = (params.stripe_angle as f64).to_radians();
    let rotated = x * angle.cos() + y * angle.sin();
    let stripe = fbm(perlin, rotated * scale * 3.0, 0.3, params.noise_octaves);
    let detail = fbm(simplex, x * scale * 0.5, y * scale * 2.0, 2);
    stripe * 0.8 + detail * 0.2
}

fn marble_pattern(x: f64, y: f64, params: &TexParams, perlin: &Perlin, simplex: &OpenSimplex) -> f64 {
    let scale = params.noise_scale as f64;
    let warp_x = fbm(perlin, x * scale, y * scale, 3) * 8.0;
    let warp_y = fbm(simplex, x * scale + 5.3, y * scale + 1.7, 3) * 8.0;
    let v = (x * scale + warp_x).sin() * 0.5 + 0.5;
    let detail = fbm(perlin, x * scale * 2.0 + warp_y, y * scale * 2.0, 2);
    v * 0.7 + detail * 0.3
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

// =====================================================================
// Noise primitives
// =====================================================================

fn fbm<F: NoiseFn<f64, 2>>(noise: &F, x: f64, y: f64, octaves: u32) -> f64 {
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
