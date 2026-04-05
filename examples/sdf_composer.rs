//! Interactive SDF shape composer for exploring signed distance field primitives.
//!
//! Build shapes from SDF primitives (circle, box, line, polygon) using boolean
//! operations (union, intersection, subtraction) and smooth blending. Renders
//! the distance field to a texture with configurable visualization.
//!
//! Usage:
//!   cargo run --example sdf_composer
//!
//! Controls:
//!   Left panel — add/remove shapes, tweak parameters, select operations
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

// =====================================================================
// SDF primitives
// =====================================================================

fn sd_circle(px: f64, py: f64, cx: f64, cy: f64, r: f64) -> f64 {
    let dx = px - cx;
    let dy = py - cy;
    (dx * dx + dy * dy).sqrt() - r
}

fn sd_box(px: f64, py: f64, cx: f64, cy: f64, hw: f64, hh: f64) -> f64 {
    let dx = (px - cx).abs() - hw;
    let dy = (py - cy).abs() - hh;
    let outside = (dx.max(0.0) * dx.max(0.0) + dy.max(0.0) * dy.max(0.0)).sqrt();
    let inside = dx.max(dy).min(0.0);
    outside + inside
}

fn sd_line(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64, thickness: f64) -> f64 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let t = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    let cx = pax - bax * t;
    let cy = pay - bay * t;
    (cx * cx + cy * cy).sqrt() - thickness
}

fn sd_equilateral_triangle(px: f64, py: f64, cx: f64, cy: f64, size: f64) -> f64 {
    // Equilateral triangle centered at (cx, cy) with circumradius = size
    let x = (px - cx).abs();
    let y = -(py - cy) + size * 0.5;
    let k = 3.0_f64.sqrt();

    // Clamp to triangle half-plane
    let x = x - (-k * y).clamp(-size, 0.0) * 0.5;
    let y = y - x.clamp(0.0, size * k * 0.5);

    let _ = (x, y); // Use the shadowed values
    // Simpler approach: point-to-edge distances for equilateral triangle
    let lx = px - cx;
    let ly = py - cy;

    // Three vertices of equilateral triangle
    let v0x = 0.0;
    let v0y = size;
    let v1x = -size * k * 0.5;
    let v1y = -size * 0.5;
    let v2x = size * k * 0.5;
    let v2y = -size * 0.5;

    // Distance to each edge (signed)
    let d0 = edge_dist(lx, ly, v0x, v0y, v1x, v1y);
    let d1 = edge_dist(lx, ly, v1x, v1y, v2x, v2y);
    let d2 = edge_dist(lx, ly, v2x, v2y, v0x, v0y);

    // Inside = all negative, outside = max of positives
    let inner = d0.max(d1).max(d2);
    if inner < 0.0 {
        inner // negative inside
    } else {
        // Distance to nearest edge segment
        let s0 = seg_dist(lx, ly, v0x, v0y, v1x, v1y);
        let s1 = seg_dist(lx, ly, v1x, v1y, v2x, v2y);
        let s2 = seg_dist(lx, ly, v2x, v2y, v0x, v0y);
        s0.min(s1).min(s2)
    }
}

/// Signed distance from point to the left side of directed edge (a -> b).
fn edge_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let ex = bx - ax;
    let ey = by - ay;
    let len = (ex * ex + ey * ey).sqrt();
    // Normal pointing left of edge direction
    let nx = -ey / len;
    let ny = ex / len;
    (px - ax) * nx + (py - ay) * ny
}

/// Unsigned distance from point to line segment.
fn seg_dist(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let t = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    let cx = pax - bax * t;
    let cy = pay - bay * t;
    (cx * cx + cy * cy).sqrt()
}

fn sd_star(px: f64, py: f64, cx: f64, cy: f64, outer: f64, inner: f64, points: u32) -> f64 {
    let lx = px - cx;
    let ly = py - cy;
    let angle = ly.atan2(lx);
    let r = (lx * lx + ly * ly).sqrt();

    let n = points as f64;
    let sector = std::f64::consts::TAU / n;
    let half = sector * 0.5;

    // Angle within the current sector
    let a = ((angle % sector) + sector) % sector;
    let a = if a > half { sector - a } else { a };

    // The star edge is a line from (inner, half) to (outer, 0) in polar-ish coords
    // Convert to cartesian for that sector
    let tip_x = outer;
    let tip_y = 0.0;
    let notch_x = inner * half.cos();
    let notch_y = inner * half.sin();

    let pt_x = r * a.cos();
    let pt_y = r * a.sin();

    // Signed distance to the edge line
    let edge_x = tip_x - notch_x;
    let edge_y = tip_y - notch_y;
    let len = (edge_x * edge_x + edge_y * edge_y).sqrt();
    let nx = edge_y / len;
    let ny = -edge_x / len;

    let d = (pt_x - notch_x) * nx + (pt_y - notch_y) * ny;

    // Also need distance to segment for outside points
    let t = (((pt_x - notch_x) * edge_x + (pt_y - notch_y) * edge_y) / (len * len)).clamp(0.0, 1.0);
    let closest_x = notch_x + edge_x * t;
    let closest_y = notch_y + edge_y * t;
    let seg_d = ((pt_x - closest_x).powi(2) + (pt_y - closest_y).powi(2)).sqrt();

    if d < 0.0 { d } else { seg_d }
}

// =====================================================================
// Boolean operations
// =====================================================================

fn op_union(a: f64, b: f64) -> f64 {
    a.min(b)
}

fn op_intersection(a: f64, b: f64) -> f64 {
    a.max(b)
}

fn op_subtraction(a: f64, b: f64) -> f64 {
    // Subtract b from a
    a.max(-b)
}

fn op_smooth_union(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

fn op_smooth_subtraction(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.max(-b);
    }
    let h = (0.5 - 0.5 * (a + b) / k).clamp(0.0, 1.0);
    a + (-b - a) * h + k * h * (1.0 - h)
}

fn op_smooth_intersection(a: f64, b: f64, k: f64) -> f64 {
    if k <= 0.0 {
        return a.max(b);
    }
    let h = (0.5 - 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h + k * h * (1.0 - h)
}

// =====================================================================
// Shape & scene model
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrimitiveType {
    Circle,
    Box,
    Line,
    Triangle,
    Star,
}

impl PrimitiveType {
    const ALL: &[PrimitiveType] = &[
        PrimitiveType::Circle,
        PrimitiveType::Box,
        PrimitiveType::Line,
        PrimitiveType::Triangle,
        PrimitiveType::Star,
    ];

    fn label(self) -> &'static str {
        match self {
            PrimitiveType::Circle => "Circle",
            PrimitiveType::Box => "Box",
            PrimitiveType::Line => "Line",
            PrimitiveType::Triangle => "Triangle",
            PrimitiveType::Star => "Star",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoolOp {
    Union,
    Intersection,
    Subtraction,
    SmoothUnion,
    SmoothSubtraction,
    SmoothIntersection,
}

impl BoolOp {
    const ALL: &[BoolOp] = &[
        BoolOp::Union,
        BoolOp::Intersection,
        BoolOp::Subtraction,
        BoolOp::SmoothUnion,
        BoolOp::SmoothSubtraction,
        BoolOp::SmoothIntersection,
    ];

    fn label(self) -> &'static str {
        match self {
            BoolOp::Union => "Union (min)",
            BoolOp::Intersection => "Intersection (max)",
            BoolOp::Subtraction => "Subtraction (A - B)",
            BoolOp::SmoothUnion => "Smooth Union",
            BoolOp::SmoothSubtraction => "Smooth Subtraction",
            BoolOp::SmoothIntersection => "Smooth Intersection",
        }
    }
}

#[derive(Clone)]
struct Shape {
    primitive: PrimitiveType,
    x: f64,
    y: f64,
    // Size params (meaning varies by primitive)
    size_a: f64, // radius, half-width, thickness, triangle size, star outer
    size_b: f64, // half-height, star inner radius
    // Line endpoint offset
    line_dx: f64,
    line_dy: f64,
    // Star points
    star_points: u32,
    // Rotation (radians)
    rotation: f64,
    // How this shape combines with previous shapes
    op: BoolOp,
    smooth_k: f64,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            primitive: PrimitiveType::Circle,
            x: 0.0,
            y: 0.0,
            size_a: 0.15,
            size_b: 0.1,
            line_dx: 0.2,
            line_dy: 0.0,
            star_points: 5,
            rotation: 0.0,
            op: BoolOp::Union,
            smooth_k: 0.05,
        }
    }
}

impl Shape {
    fn evaluate(&self, px: f64, py: f64) -> f64 {
        // Apply rotation around shape center
        let dx = px - self.x;
        let dy = py - self.y;
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();
        let rx = dx * cos_r + dy * sin_r + self.x;
        let ry = -dx * sin_r + dy * cos_r + self.y;

        match self.primitive {
            PrimitiveType::Circle => sd_circle(rx, ry, self.x, self.y, self.size_a),
            PrimitiveType::Box => sd_box(rx, ry, self.x, self.y, self.size_a, self.size_b),
            PrimitiveType::Line => sd_line(rx, ry, self.x, self.y, self.x + self.line_dx, self.y + self.line_dy, self.size_a),
            PrimitiveType::Triangle => sd_equilateral_triangle(rx, ry, self.x, self.y, self.size_a),
            PrimitiveType::Star => sd_star(rx, ry, self.x, self.y, self.size_a, self.size_b, self.star_points),
        }
    }

    fn combine(&self, existing: f64, mine: f64) -> f64 {
        match self.op {
            BoolOp::Union => op_union(existing, mine),
            BoolOp::Intersection => op_intersection(existing, mine),
            BoolOp::Subtraction => op_subtraction(existing, mine),
            BoolOp::SmoothUnion => op_smooth_union(existing, mine, self.smooth_k),
            BoolOp::SmoothSubtraction => op_smooth_subtraction(existing, mine, self.smooth_k),
            BoolOp::SmoothIntersection => op_smooth_intersection(existing, mine, self.smooth_k),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisMode {
    Solid,
    DistanceField,
    Contours,
}

#[derive(Resource)]
struct Scene {
    shapes: Vec<Shape>,
    vis_mode: VisMode,
    show_grid: bool,
    bg_color: [u8; 3],
    fg_color: [u8; 3],
    selected: usize,
}

impl Default for Scene {
    fn default() -> Self {
        let circle = Shape {
            primitive: PrimitiveType::Circle,
            x: -0.1,
            y: 0.0,
            size_a: 0.2,
            ..Default::default()
        };
        let box_shape = Shape {
            primitive: PrimitiveType::Box,
            x: 0.1,
            y: 0.0,
            size_a: 0.15,
            size_b: 0.15,
            op: BoolOp::SmoothUnion,
            smooth_k: 0.05,
            ..Default::default()
        };
        Self {
            shapes: vec![circle, box_shape],
            vis_mode: VisMode::Solid,
            show_grid: false,
            bg_color: [30, 30, 35],
            fg_color: [220, 140, 60],
            selected: 0,
        }
    }
}

#[derive(Resource)]
struct SceneDirty(bool);

#[derive(Resource, Default)]
struct DragState {
    active: bool,
}

#[derive(Component)]
struct SdfSprite;

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
                    title: "SDF Composer".into(),
                    resolution: bevy::window::WindowResolution::new(1100, 720),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(Scene::default())
        .insert_resource(SceneDirty(true))
        .insert_resource(DragState::default())
        .add_systems(Startup, setup)
        .add_systems(EguiPrimaryContextPass, ui_panel)
        .add_systems(Update, (regenerate_texture, camera_zoom, camera_pan, drag_shape))
        .run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn(Camera2d);

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
        SdfSprite,
    ));
}

// =====================================================================
// UI
// =====================================================================

fn ui_panel(mut contexts: EguiContexts, mut scene: ResMut<Scene>, mut dirty: ResMut<SceneDirty>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    egui::SidePanel::left("sdf_params").min_width(300.0).show(ctx, |ui| {
        ui.heading("SDF Composer");
        ui.separator();

        // Visualization mode
        ui.label("Visualization");
        if ui.radio_value(&mut scene.vis_mode, VisMode::Solid, "Solid").changed() { dirty.0 = true; }
        if ui.radio_value(&mut scene.vis_mode, VisMode::DistanceField, "Distance Field").changed() { dirty.0 = true; }
        if ui.radio_value(&mut scene.vis_mode, VisMode::Contours, "Contours").changed() { dirty.0 = true; }

        if ui.checkbox(&mut scene.show_grid, "Show grid").changed() { dirty.0 = true; }
        ui.separator();

        // Shape list
        ui.horizontal(|ui| {
            ui.label("Shapes");
            if ui.button("+ Add").clicked() {
                scene.shapes.push(Shape::default());
                scene.selected = scene.shapes.len() - 1;
                dirty.0 = true;
            }
        });

        let num_shapes = scene.shapes.len();
        for i in 0..num_shapes {
            let label = format!("{}: {}", i, scene.shapes[i].primitive.label());
            if ui.selectable_label(scene.selected == i, &label).clicked() {
                scene.selected = i;
            }
        }

        if num_shapes > 1 {
            if ui.button("Remove selected").clicked() {
                let sel = scene.selected;
                scene.shapes.remove(sel);
                if scene.selected >= scene.shapes.len() {
                    scene.selected = scene.shapes.len() - 1;
                }
                dirty.0 = true;
            }
        }

        ui.separator();

        // Selected shape params
        if scene.selected < scene.shapes.len() {
            let sel = scene.selected;

            ui.label("Primitive");
            for &pt in PrimitiveType::ALL {
                if ui.radio_value(&mut scene.shapes[sel].primitive, pt, pt.label()).changed() {
                    dirty.0 = true;
                }
            }
            ui.separator();

            ui.label("Position X");
            if ui.add(egui::Slider::new(&mut scene.shapes[sel].x, -0.5..=0.5)).changed() { dirty.0 = true; }
            ui.label("Position Y");
            if ui.add(egui::Slider::new(&mut scene.shapes[sel].y, -0.5..=0.5)).changed() { dirty.0 = true; }

            ui.label("Rotation");
            let mut deg = scene.shapes[sel].rotation.to_degrees();
            if ui.add(egui::Slider::new(&mut deg, -180.0..=180.0).suffix("°")).changed() {
                scene.shapes[sel].rotation = deg.to_radians();
                dirty.0 = true;
            }

            ui.separator();
            match scene.shapes[sel].primitive {
                PrimitiveType::Circle => {
                    ui.label("Radius");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_a, 0.01..=0.4)).changed() { dirty.0 = true; }
                }
                PrimitiveType::Box => {
                    ui.label("Half Width");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_a, 0.01..=0.4)).changed() { dirty.0 = true; }
                    ui.label("Half Height");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_b, 0.01..=0.4)).changed() { dirty.0 = true; }
                }
                PrimitiveType::Line => {
                    ui.label("Thickness");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_a, 0.005..=0.1)).changed() { dirty.0 = true; }
                    ui.label("End X offset");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].line_dx, -0.5..=0.5)).changed() { dirty.0 = true; }
                    ui.label("End Y offset");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].line_dy, -0.5..=0.5)).changed() { dirty.0 = true; }
                }
                PrimitiveType::Triangle => {
                    ui.label("Size");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_a, 0.02..=0.4)).changed() { dirty.0 = true; }
                }
                PrimitiveType::Star => {
                    ui.label("Outer Radius");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_a, 0.02..=0.4)).changed() { dirty.0 = true; }
                    ui.label("Inner Radius");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].size_b, 0.01..=0.35)).changed() { dirty.0 = true; }
                    ui.label("Points");
                    let mut pts = scene.shapes[sel].star_points as i32;
                    if ui.add(egui::Slider::new(&mut pts, 3..=12)).changed() {
                        scene.shapes[sel].star_points = pts as u32;
                        dirty.0 = true;
                    }
                }
            }

            // Boolean operation (not for first shape)
            if sel > 0 {
                ui.separator();
                ui.label("Boolean Operation");
                for &op in BoolOp::ALL {
                    if ui.radio_value(&mut scene.shapes[sel].op, op, op.label()).changed() {
                        dirty.0 = true;
                    }
                }

                let is_smooth = matches!(
                    scene.shapes[sel].op,
                    BoolOp::SmoothUnion | BoolOp::SmoothSubtraction | BoolOp::SmoothIntersection
                );
                if is_smooth {
                    ui.label("Smoothness (k)");
                    if ui.add(egui::Slider::new(&mut scene.shapes[sel].smooth_k, 0.001..=0.2)).changed() {
                        dirty.0 = true;
                    }
                }
            }
        }
    });
}

// =====================================================================
// Texture regeneration
// =====================================================================

fn regenerate_texture(
    mut dirty: ResMut<SceneDirty>,
    scene: Res<Scene>,
    mut images: ResMut<Assets<Image>>,
    sprites: Query<&Sprite, With<SdfSprite>>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let Ok(sprite) = sprites.single() else { return };
    let Some(image) = images.get_mut(&sprite.image) else { return };

    let num_pixels = (TEX_W * TEX_H * 4) as usize;
    let mut pixels = vec![0u8; num_pixels];

    let pixel_size = 1.0 / TEX_W as f64;

    for py in 0..TEX_H {
        for px in 0..TEX_W {
            // Map pixel to [-0.5, 0.5] normalized coords
            let nx = (px as f64 + 0.5) / TEX_W as f64 - 0.5;
            let ny = 0.5 - (py as f64 + 0.5) / TEX_H as f64; // Y up

            let d = evaluate_scene(&scene.shapes, nx, ny);

            let idx = ((py * TEX_W + px) * 4) as usize;
            let [r, g, b] = render_pixel(d, pixel_size, &scene, nx, ny);
            pixels[idx] = r;
            pixels[idx + 1] = g;
            pixels[idx + 2] = b;
            pixels[idx + 3] = 255;
        }
    }

    image.data = Some(pixels);
}

fn evaluate_scene(shapes: &[Shape], px: f64, py: f64) -> f64 {
    if shapes.is_empty() {
        return f64::MAX;
    }
    let mut d = shapes[0].evaluate(px, py);
    for shape in &shapes[1..] {
        let sd = shape.evaluate(px, py);
        d = shape.combine(d, sd);
    }
    d
}

fn render_pixel(d: f64, pixel_size: f64, scene: &Scene, nx: f64, ny: f64) -> [u8; 3] {
    let grid = if scene.show_grid {
        let gx = ((nx * 10.0).fract().abs() - 0.5).abs();
        let gy = ((ny * 10.0).fract().abs() - 0.5).abs();
        let g = gx.min(gy);
        if g < 0.02 { 0.15 } else { 0.0 }
    } else {
        0.0
    };

    match scene.vis_mode {
        VisMode::Solid => {
            // Anti-aliased solid fill
            let coverage = 1.0 - smoothstep(-pixel_size, pixel_size, d);
            let r = scene.bg_color[0] as f64 + (scene.fg_color[0] as f64 - scene.bg_color[0] as f64) * coverage;
            let g_val = scene.bg_color[1] as f64 + (scene.fg_color[1] as f64 - scene.bg_color[1] as f64) * coverage;
            let b = scene.bg_color[2] as f64 + (scene.fg_color[2] as f64 - scene.bg_color[2] as f64) * coverage;
            [
                (r + grid * 255.0).min(255.0) as u8,
                (g_val + grid * 255.0).min(255.0) as u8,
                (b + grid * 255.0).min(255.0) as u8,
            ]
        }
        VisMode::DistanceField => {
            // Visualize signed distance: blue = inside (negative), red = outside (positive)
            let t = (d * 5.0).clamp(-1.0, 1.0);
            if t < 0.0 {
                // Inside: blue
                let v = (-t * 200.0) as u8;
                [(grid * 255.0) as u8, (grid * 255.0) as u8, (v as f64 + grid * 255.0).min(255.0) as u8]
            } else {
                // Outside: red
                let v = (t * 200.0) as u8;
                [(v as f64 + grid * 255.0).min(255.0) as u8, (grid * 255.0) as u8, (grid * 255.0) as u8]
            }
        }
        VisMode::Contours => {
            // Distance field contour lines
            let abs_d = d.abs();
            let contour_spacing = 0.02;
            let contour = (abs_d % contour_spacing) / contour_spacing;
            let line = smoothstep(0.4, 0.5, contour) * (1.0 - smoothstep(0.5, 0.6, contour));

            // Zero contour (shape boundary) is bright
            let boundary = 1.0 - smoothstep(0.0, pixel_size * 2.0, abs_d);

            let inside = if d < 0.0 { 0.15 } else { 0.05 };
            let v = inside + line * 0.3 + boundary * 0.8 + grid;
            let v = (v * 255.0).min(255.0) as u8;

            if d < 0.0 {
                [v / 3, v / 2, v] // Blue tint inside
            } else {
                [v, v / 2, v / 3] // Red tint outside
            }
        }
    }
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// =====================================================================
// Camera controls (matching texture_lab / noise_visualizer pattern)
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
// Drag selected shape with left mouse
// =====================================================================

fn drag_shape(
    mut scene: ResMut<Scene>,
    mut dirty: ResMut<SceneDirty>,
    mut drag: ResMut<DragState>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    sprites: Query<(&Sprite, &GlobalTransform), With<SdfSprite>>,
    mut contexts: EguiContexts,
) {
    let egui_wants_pointer = contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_pointer_input());

    if mouse.just_released(MouseButton::Left) {
        drag.active = false;
        return;
    }

    if mouse.just_pressed(MouseButton::Left) && !egui_wants_pointer {
        drag.active = true;
    }

    if !drag.active || !mouse.pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_tf)) = camera.single() else { return };
    let Ok((sprite, sprite_tf)) = sprites.single() else { return };

    let Some(cursor) = window.cursor_position() else { return };
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_tf, cursor) else { return };

    // Convert world position to sprite-local normalized coords [-0.5, 0.5]
    let sprite_size = sprite.custom_size.unwrap_or(Vec2::new(TEX_W as f32, TEX_H as f32));
    let sprite_pos = sprite_tf.translation().truncate();
    let local = (world_pos - sprite_pos) / sprite_size;

    let nx = local.x as f64;
    let ny = local.y as f64;

    let sel = scene.selected;
    if let Some(shape) = scene.shapes.get_mut(sel) {
        shape.x = nx.clamp(-0.5, 0.5);
        shape.y = ny.clamp(-0.5, 0.5);
        dirty.0 = true;
    }
}
