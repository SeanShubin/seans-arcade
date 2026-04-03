//! Renders all 47 beveled blob tiles with physically-based bevel lighting.
//!
//! Uses the documented LDtk blob layout (12×5 grid).
//! Bevel width is 1/4 of tile size. Edge lines mark bevel boundaries:
//!   - Lighter gray at the outer edge (top of bevel ridge)
//!   - Darker gray at the inner edge (where bevel meets face)
//!   - No lines at tile boundaries (far edges between adjacent tiles)
//!
//! Usage:
//!   cargo run --example beveled_block                      # interactive
//!   cargo run --example beveled_block -- --render          # export PNG
//!   cargo run --example beveled_block -- --render out.png  # export to path
//!
//! Controls:
//!   Scroll wheel — zoom in/out
//!   Middle mouse drag — pan
//!   Arrow keys — pan

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseMotion, MouseWheel},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

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
// Tile geometry constants
// =====================================================================

const TILE_SIZE: f32 = 64.0;
const BEVEL_FRACTION: f32 = 0.25;
const TILE_GAP: f32 = 2.0;
const GRID_COLS: f32 = 12.0;
const GRID_ROWS: f32 = 5.0;
const BOUNDARY_LINE_WIDTH: f32 = 0.6;
const OUTER_LINE_OPACITY: f32 = 0.04;
const INNER_LINE_OPACITY: f32 = 0.05;

const Z_BEVEL: f32 = 0.5;
const Z_FACE: f32 = 1.0;
const Z_CONCAVE: f32 = 1.5;
const Z_EDGE_LINE: f32 = 2.0;

// Pixel rendering constants
const TILE_PX: u32 = TILE_SIZE as u32;
const GAP_PX: u32 = TILE_GAP as u32;
const STRIDE_PX: u32 = TILE_PX + GAP_PX;
const COLS: u32 = GRID_COLS as u32;
const ROWS: u32 = GRID_ROWS as u32;
const EDGE_LINE_HALF_WIDTH: f32 = 0.5;

fn pixel_image_width() -> u32 { COLS * TILE_PX + (COLS - 1) * GAP_PX }
fn pixel_image_height() -> u32 { ROWS * TILE_PX + (ROWS - 1) * GAP_PX }

// =====================================================================
// App entry point
// =====================================================================

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let render_mode = args.iter().any(|a| a == "--render");

    if render_mode {
        let render_idx = args.iter().position(|a| a == "--render").unwrap();
        let output_path = args.get(render_idx + 1)
            .filter(|a| !a.starts_with("--"))
            .map(|s| s.as_str())
            .unwrap_or("assets/generated/beveled_block.png");
        render_and_exit(output_path);
    } else {
        run_interactive();
    }
}

fn render_and_exit(output_path: &str) {
    let pixels = render_all_tiles_to_pixels();
    let img = image::RgbaImage::from_raw(pixel_image_width(), pixel_image_height(), pixels)
        .expect("Failed to create image buffer");
    let dir = std::path::Path::new(output_path).parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir).ok();
    img.save(output_path).expect("Failed to save image");
    println!("Rendered: {}", output_path);
}

fn run_interactive() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Beveled Blob 47".into(),
                resolution: bevy::window::WindowResolution::new(1200, 600),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, spawn_camera_and_tiles)
        .add_systems(Update, (camera_zoom, camera_pan))
        .run();
}

// =====================================================================
// Startup — spawns camera and all 47 blob tiles
// =====================================================================

fn spawn_camera_and_tiles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let bevel = TILE_SIZE * BEVEL_FRACTION;
    let stride = TILE_SIZE + TILE_GAP;
    let lighting = BevelLighting::new(bevel);
    let shared = SharedMaterials::new(&mut materials, &lighting);

    let grid_offset = Vec2::new(
        -stride * (GRID_COLS / 2.0),
        stride * ((GRID_ROWS - 1.0) / 2.0),
    );

    for &(col, row, mask) in &BLOB_LAYOUT {
        let center = Vec2::new(
            grid_offset.x + col as f32 * stride,
            grid_offset.y - row as f32 * stride,
        );
        spawn_beveled_tile(&mut commands, &mut meshes, &shared, &lighting, center, bevel, mask);
    }
}

// =====================================================================
// Bevel lighting model
// =====================================================================

struct BevelLighting {
    light_dir: Vec3,
    base_color: Vec3,
    ambient: f32,
    bevel_sin: f32,
    bevel_cos: f32,
}

impl BevelLighting {
    fn new(bevel_width: f32) -> Self {
        let light_angle = std::f32::consts::FRAC_PI_8;
        let bevel_depth = 10.0;
        let bevel_angle = (bevel_depth / bevel_width).atan();
        Self {
            light_dir: Vec3::new(-light_angle.cos(), light_angle.sin(), 2.0).normalize(),
            base_color: Vec3::new(0.50, 0.50, 0.55),
            ambient: 0.25,
            bevel_sin: bevel_angle.sin(),
            bevel_cos: bevel_angle.cos(),
        }
    }

    fn brightness(&self, normal: Vec3) -> f32 {
        let diffuse = normal.dot(self.light_dir).max(0.0);
        (self.ambient + (1.0 - self.ambient) * diffuse).clamp(0.0, 1.0)
    }

    fn color_for_normal(&self, normal: Vec3) -> Color {
        let b = self.brightness(normal);
        Color::srgb(
            self.base_color.x * b,
            self.base_color.y * b,
            self.base_color.z * b,
        )
    }

    fn face_color(&self) -> Color {
        self.color_for_normal(Vec3::Z)
    }

    fn surface_normal(&self, dir: Vec2) -> Vec3 {
        Vec3::new(dir.x * self.bevel_sin, dir.y * self.bevel_sin, self.bevel_cos).normalize()
    }
}

// =====================================================================
// Shared materials — created once, reused across all tiles
// =====================================================================

struct SharedMaterials {
    face: Handle<ColorMaterial>,
    white: Handle<ColorMaterial>,
    outer_edge: Handle<ColorMaterial>,
    inner_edge: Handle<ColorMaterial>,
}

impl SharedMaterials {
    fn new(materials: &mut ResMut<Assets<ColorMaterial>>, lighting: &BevelLighting) -> Self {
        Self {
            face: materials.add(ColorMaterial::from_color(lighting.face_color())),
            white: materials.add(ColorMaterial::from_color(Color::WHITE)),
            outer_edge: materials.add(ColorMaterial::from_color(Color::srgba(1.0, 1.0, 1.0, OUTER_LINE_OPACITY))),
            inner_edge: materials.add(ColorMaterial::from_color(Color::srgba(0.0, 0.0, 0.0, INNER_LINE_OPACITY))),
        }
    }
}

// =====================================================================
// Tile geometry — face bounds and corner positions
// =====================================================================

struct TileGeometry {
    half: f32,
    face_left: f32,
    face_right: f32,
    face_top: f32,
    face_bottom: f32,
}

impl TileGeometry {
    fn from_mask(mask: u8, bevel: f32) -> Self {
        let half = TILE_SIZE / 2.0;
        let inner = half - bevel;
        let has = |bit: u8| mask & bit != 0;
        Self {
            half,
            face_left: if !has(W) { -inner } else { -half },
            face_right: if !has(E) { inner } else { half },
            face_top: if !has(N) { inner } else { half },
            face_bottom: if !has(S) { -inner } else { -half },
        }
    }

    fn face_center(&self) -> Vec2 {
        Vec2::new(
            (self.face_left + self.face_right) / 2.0,
            (self.face_bottom + self.face_top) / 2.0,
        )
    }

    fn face_size(&self) -> Vec2 {
        Vec2::new(
            self.face_right - self.face_left,
            self.face_top - self.face_bottom,
        )
    }

    fn outer_corner(&self, corner: Corner) -> Vec2 {
        match corner {
            Corner::TopLeft => Vec2::new(-self.half, self.half),
            Corner::TopRight => Vec2::new(self.half, self.half),
            Corner::BottomLeft => Vec2::new(-self.half, -self.half),
            Corner::BottomRight => Vec2::new(self.half, -self.half),
        }
    }

    fn inner_corner(&self, corner: Corner) -> Vec2 {
        match corner {
            Corner::TopLeft => Vec2::new(self.face_left, self.face_top),
            Corner::TopRight => Vec2::new(self.face_right, self.face_top),
            Corner::BottomLeft => Vec2::new(self.face_left, self.face_bottom),
            Corner::BottomRight => Vec2::new(self.face_right, self.face_bottom),
        }
    }
}

#[derive(Clone, Copy)]
enum Corner { TopLeft, TopRight, BottomLeft, BottomRight }

// =====================================================================
// Bevel colors for each direction
// =====================================================================

struct BevelColors {
    top: Color,
    bottom: Color,
    left: Color,
    right: Color,
    top_left: Color,
    top_right: Color,
    bottom_left: Color,
    bottom_right: Color,
}

impl BevelColors {
    fn from_lighting(lighting: &BevelLighting) -> Self {
        let top_n = lighting.surface_normal(Vec2::Y);
        let bottom_n = lighting.surface_normal(Vec2::NEG_Y);
        let left_n = lighting.surface_normal(Vec2::NEG_X);
        let right_n = lighting.surface_normal(Vec2::X);
        Self {
            top: lighting.color_for_normal(top_n),
            bottom: lighting.color_for_normal(bottom_n),
            left: lighting.color_for_normal(left_n),
            right: lighting.color_for_normal(right_n),
            top_left: lighting.color_for_normal((top_n + left_n).normalize()),
            top_right: lighting.color_for_normal((top_n + right_n).normalize()),
            bottom_left: lighting.color_for_normal((bottom_n + left_n).normalize()),
            bottom_right: lighting.color_for_normal((bottom_n + right_n).normalize()),
        }
    }
}

// =====================================================================
// Spawning a single beveled tile (orchestration level)
// =====================================================================

fn spawn_beveled_tile(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    lighting: &BevelLighting,
    center: Vec2,
    bevel: f32,
    mask: u8,
) {
    let has = |bit: u8| mask & bit != 0;
    let geo = TileGeometry::from_mask(mask, bevel);
    let colors = BevelColors::from_lighting(lighting);

    spawn_face(commands, meshes, mats, center, &geo);
    spawn_convex_bevels(commands, meshes, mats, center, &geo, &colors, mask);
    spawn_concave_bevels(commands, meshes, mats, center, &geo, &colors, bevel, mask);
    spawn_convex_edge_lines(commands, meshes, mats, center, &geo, mask);
    spawn_concave_edge_lines(commands, meshes, mats, center, &geo, bevel, has);
}

fn spawn_face(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    center: Vec2,
    geo: &TileGeometry,
) {
    let size = geo.face_size();
    let face_center = geo.face_center();
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(size.x, size.y))),
        MeshMaterial2d(mats.face.clone()),
        Transform::from_xyz(center.x + face_center.x, center.y + face_center.y, Z_FACE),
    ));
}

// =====================================================================
// Convex bevels — the sloped edges where this tile borders void
// =====================================================================

fn spawn_convex_bevels(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    center: Vec2,
    geo: &TileGeometry,
    colors: &BevelColors,
    mask: u8,
) {
    let has = |bit: u8| mask & bit != 0;
    let bevel_n = !has(N);
    let bevel_e = !has(E);
    let bevel_s = !has(S);
    let bevel_w = !has(W);

    if bevel_n {
        let color_left = if bevel_w { colors.top_left } else { colors.top };
        let color_right = if bevel_e { colors.top_right } else { colors.top };
        let outer_l = if bevel_w { geo.outer_corner(Corner::TopLeft) } else { Vec2::new(-geo.half, geo.half) };
        let outer_r = if bevel_e { geo.outer_corner(Corner::TopRight) } else { Vec2::new(geo.half, geo.half) };
        spawn_gradient_quad(commands, meshes, &mats.white, center, Z_BEVEL,
            outer_l, outer_r, geo.inner_corner(Corner::TopRight), geo.inner_corner(Corner::TopLeft),
            color_left, color_right, colors.top, colors.top);
    }

    if bevel_s {
        let color_left = if bevel_w { colors.bottom_left } else { colors.bottom };
        let color_right = if bevel_e { colors.bottom_right } else { colors.bottom };
        let outer_l = if bevel_w { geo.outer_corner(Corner::BottomLeft) } else { Vec2::new(-geo.half, -geo.half) };
        let outer_r = if bevel_e { geo.outer_corner(Corner::BottomRight) } else { Vec2::new(geo.half, -geo.half) };
        spawn_gradient_quad(commands, meshes, &mats.white, center, Z_BEVEL,
            geo.inner_corner(Corner::BottomLeft), geo.inner_corner(Corner::BottomRight), outer_r, outer_l,
            colors.bottom, colors.bottom, color_right, color_left);
    }

    if bevel_w {
        let color_top = if bevel_n { colors.top_left } else { colors.left };
        let color_bottom = if bevel_s { colors.bottom_left } else { colors.left };
        let outer_t = if bevel_n { geo.outer_corner(Corner::TopLeft) } else { Vec2::new(-geo.half, geo.half) };
        let outer_b = if bevel_s { geo.outer_corner(Corner::BottomLeft) } else { Vec2::new(-geo.half, -geo.half) };
        spawn_gradient_quad(commands, meshes, &mats.white, center, Z_BEVEL,
            outer_t, geo.inner_corner(Corner::TopLeft), geo.inner_corner(Corner::BottomLeft), outer_b,
            color_top, colors.left, colors.left, color_bottom);
    }

    if bevel_e {
        let color_top = if bevel_n { colors.top_right } else { colors.right };
        let color_bottom = if bevel_s { colors.bottom_right } else { colors.right };
        let outer_t = if bevel_n { geo.outer_corner(Corner::TopRight) } else { Vec2::new(geo.half, geo.half) };
        let outer_b = if bevel_s { geo.outer_corner(Corner::BottomRight) } else { Vec2::new(geo.half, -geo.half) };
        spawn_gradient_quad(commands, meshes, &mats.white, center, Z_BEVEL,
            geo.inner_corner(Corner::TopRight), outer_t, outer_b, geo.inner_corner(Corner::BottomRight),
            colors.right, color_top, color_bottom, colors.right);
    }
}

// =====================================================================
// Concave bevels — inner corners where diagonal is void
// =====================================================================

fn spawn_concave_bevels(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    center: Vec2,
    geo: &TileGeometry,
    colors: &BevelColors,
    bevel: f32,
    mask: u8,
) {
    let has = |bit: u8| mask & bit != 0;

    // NW concave: two triangular faces meeting at the diagonal
    if has(N) && has(W) && !has(NW) {
        let outer = Vec2::new(-geo.half, geo.half);
        let top_edge = Vec2::new(-geo.half + bevel, geo.half);
        let inner_pt = Vec2::new(-geo.half + bevel, geo.half - bevel);
        let left_edge = Vec2::new(-geo.half, geo.half - bevel);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, left_edge, inner_pt, colors.top);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, inner_pt, top_edge, colors.left);
    }

    // NE concave
    if has(N) && has(E) && !has(NE) {
        let outer = Vec2::new(geo.half, geo.half);
        let top_edge = Vec2::new(geo.half - bevel, geo.half);
        let inner_pt = Vec2::new(geo.half - bevel, geo.half - bevel);
        let right_edge = Vec2::new(geo.half, geo.half - bevel);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, top_edge, inner_pt, colors.right);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, inner_pt, right_edge, colors.top);
    }

    // SW concave
    if has(S) && has(W) && !has(SW) {
        let outer = Vec2::new(-geo.half, -geo.half);
        let left_edge = Vec2::new(-geo.half, -geo.half + bevel);
        let inner_pt = Vec2::new(-geo.half + bevel, -geo.half + bevel);
        let bottom_edge = Vec2::new(-geo.half + bevel, -geo.half);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, left_edge, inner_pt, colors.bottom);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, inner_pt, bottom_edge, colors.left);
    }

    // SE concave
    if has(S) && has(E) && !has(SE) {
        let outer = Vec2::new(geo.half, -geo.half);
        let right_edge = Vec2::new(geo.half, -geo.half + bevel);
        let inner_pt = Vec2::new(geo.half - bevel, -geo.half + bevel);
        let bottom_edge = Vec2::new(geo.half - bevel, -geo.half);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, bottom_edge, inner_pt, colors.right);
        spawn_flat_triangle(commands, meshes, &mats.white, center, Z_CONCAVE,
            outer, inner_pt, right_edge, colors.bottom);
    }
}

// =====================================================================
// Edge boundary lines — convex edges
// =====================================================================

fn spawn_convex_edge_lines(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    center: Vec2,
    geo: &TileGeometry,
    mask: u8,
) {
    let has = |bit: u8| mask & bit != 0;
    let bevel_n = !has(N);
    let bevel_e = !has(E);
    let bevel_s = !has(S);
    let bevel_w = !has(W);

    // Inner square edges (outer_edge = lighter, top of bevel ridge)
    if bevel_n {
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::TopLeft), geo.inner_corner(Corner::TopRight));
    }
    if bevel_s {
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::BottomLeft), geo.inner_corner(Corner::BottomRight));
    }
    if bevel_w {
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::TopLeft), geo.inner_corner(Corner::BottomLeft));
    }
    if bevel_e {
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::TopRight), geo.inner_corner(Corner::BottomRight));
    }

    // Convex corner diagonals (inner_edge = darker, where bevel meets face)
    if bevel_n && bevel_w {
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::TopLeft), geo.outer_corner(Corner::TopLeft));
    }
    if bevel_n && bevel_e {
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::TopRight), geo.outer_corner(Corner::TopRight));
    }
    if bevel_s && bevel_w {
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::BottomLeft), geo.outer_corner(Corner::BottomLeft));
    }
    if bevel_s && bevel_e {
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            geo.inner_corner(Corner::BottomRight), geo.outer_corner(Corner::BottomRight));
    }
}

// =====================================================================
// Edge boundary lines — concave edges
// =====================================================================

fn spawn_concave_edge_lines(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mats: &SharedMaterials,
    center: Vec2,
    geo: &TileGeometry,
    bevel: f32,
    has: impl Fn(u8) -> bool,
) {
    if has(N) && has(W) && !has(NW) {
        let p = Vec2::new(-geo.half + bevel, geo.half - bevel);
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half + bevel, geo.half));
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half, geo.half - bevel));
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half, geo.half));
    }
    if has(N) && has(E) && !has(NE) {
        let p = Vec2::new(geo.half - bevel, geo.half - bevel);
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half - bevel, geo.half));
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half, geo.half - bevel));
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half, geo.half));
    }
    if has(S) && has(W) && !has(SW) {
        let p = Vec2::new(-geo.half + bevel, -geo.half + bevel);
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half + bevel, -geo.half));
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half, -geo.half + bevel));
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            p, Vec2::new(-geo.half, -geo.half));
    }
    if has(S) && has(E) && !has(SE) {
        let p = Vec2::new(geo.half - bevel, -geo.half + bevel);
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half - bevel, -geo.half));
        spawn_line_quad(commands, meshes, &mats.outer_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half, -geo.half + bevel));
        spawn_line_quad(commands, meshes, &mats.inner_edge, center, Z_EDGE_LINE,
            p, Vec2::new(geo.half, -geo.half));
    }
}

// =====================================================================
// Mesh primitives — lowest abstraction level
// =====================================================================

fn spawn_gradient_quad(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    a: Vec2, b: Vec2, c: Vec2, d: Vec2,
    ca: Color, cb: Color, cc: Color, cd: Color,
) {
    let mesh = build_gradient_quad_mesh(a, b, c, d, ca, cb, cc, cd);
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

fn spawn_flat_triangle(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    a: Vec2, b: Vec2, c: Vec2,
    color: Color,
) {
    let mesh = build_flat_triangle_mesh(a, b, c, color);
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

fn spawn_line_quad(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    from: Vec2, to: Vec2,
) {
    let mesh = build_line_mesh(from, to, BOUNDARY_LINE_WIDTH);
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

// =====================================================================
// Mesh construction — pure geometry, no ECS interaction
// =====================================================================

fn color_to_linear(c: Color) -> [f32; 4] {
    let lin = c.to_linear();
    [lin.red, lin.green, lin.blue, lin.alpha]
}

fn build_gradient_quad_mesh(
    a: Vec2, b: Vec2, c: Vec2, d: Vec2,
    ca: Color, cb: Color, cc: Color, cd: Color,
) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[a.x, a.y, 0.0], [b.x, b.y, 0.0], [c.x, c.y, 0.0], [d.x, d.y, 0.0]],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_COLOR,
        vec![color_to_linear(ca), color_to_linear(cb), color_to_linear(cc), color_to_linear(cd)],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

fn build_flat_triangle_mesh(a: Vec2, b: Vec2, c: Vec2, color: Color) -> Mesh {
    let arr = color_to_linear(color);
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[a.x, a.y, 0.0], [b.x, b.y, 0.0], [c.x, c.y, 0.0]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![arr, arr, arr])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

fn build_line_mesh(from: Vec2, to: Vec2, width: f32) -> Mesh {
    let dir = (to - from).normalize();
    let perp = Vec2::new(-dir.y, dir.x) * width * 0.5;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [(from + perp).x, (from + perp).y, 0.0],
            [(to + perp).x, (to + perp).y, 0.0],
            [(to - perp).x, (to - perp).y, 0.0],
            [(from - perp).x, (from - perp).y, 0.0],
        ],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

// =====================================================================
// CPU pixel rendering — for --render export
// =====================================================================

/// Precomputed sRGB colors for all bevel directions.
struct DirectionalColors {
    face: [f32; 3],
    top: [f32; 3],
    bottom: [f32; 3],
    left: [f32; 3],
    right: [f32; 3],
    top_left: [f32; 3],
    top_right: [f32; 3],
    bottom_left: [f32; 3],
    bottom_right: [f32; 3],
}

impl DirectionalColors {
    fn from_lighting(lighting: &BevelLighting) -> Self {
        Self {
            face: srgb_from_normal(lighting, Vec3::Z),
            top: srgb_from_normal(lighting, lighting.surface_normal(Vec2::Y)),
            bottom: srgb_from_normal(lighting, lighting.surface_normal(Vec2::NEG_Y)),
            left: srgb_from_normal(lighting, lighting.surface_normal(Vec2::NEG_X)),
            right: srgb_from_normal(lighting, lighting.surface_normal(Vec2::X)),
            top_left: srgb_from_normal(lighting, (lighting.surface_normal(Vec2::Y) + lighting.surface_normal(Vec2::NEG_X)).normalize()),
            top_right: srgb_from_normal(lighting, (lighting.surface_normal(Vec2::Y) + lighting.surface_normal(Vec2::X)).normalize()),
            bottom_left: srgb_from_normal(lighting, (lighting.surface_normal(Vec2::NEG_Y) + lighting.surface_normal(Vec2::NEG_X)).normalize()),
            bottom_right: srgb_from_normal(lighting, (lighting.surface_normal(Vec2::NEG_Y) + lighting.surface_normal(Vec2::X)).normalize()),
        }
    }
}

fn srgb_from_normal(lighting: &BevelLighting, normal: Vec3) -> [f32; 3] {
    let b = lighting.brightness(normal);
    [lighting.base_color.x * b, lighting.base_color.y * b, lighting.base_color.z * b]
}

fn render_all_tiles_to_pixels() -> Vec<u8> {
    let img_w = pixel_image_width();
    let img_h = pixel_image_height();
    let mut pixels = vec![0u8; (img_w * img_h * 4) as usize];

    let bevel = TILE_SIZE * BEVEL_FRACTION;
    let lighting = BevelLighting::new(bevel);
    let dc = DirectionalColors::from_lighting(&lighting);

    for &(col, row, mask) in &BLOB_LAYOUT {
        let origin_x = col * STRIDE_PX;
        let origin_y = row * STRIDE_PX;
        render_tile_to_pixels(&mut pixels, img_w, origin_x, origin_y, mask, bevel, &dc);
    }

    pixels
}

fn render_tile_to_pixels(
    pixels: &mut [u8],
    img_w: u32,
    origin_x: u32, origin_y: u32,
    mask: u8,
    bevel: f32,
    dc: &DirectionalColors,
) {
    let has = |bit: u8| mask & bit != 0;
    let bevel_n = !has(N);
    let bevel_e = !has(E);
    let bevel_s = !has(S);
    let bevel_w = !has(W);
    let inner_nw = has(N) && has(W) && !has(NW);
    let inner_ne = has(N) && has(E) && !has(NE);
    let inner_sw = has(S) && has(W) && !has(SW);
    let inner_se = has(S) && has(E) && !has(SE);

    let size = TILE_SIZE;
    let north_left = if bevel_w { dc.top_left } else { dc.top };
    let north_right = if bevel_e { dc.top_right } else { dc.top };
    let south_left = if bevel_w { dc.bottom_left } else { dc.bottom };
    let south_right = if bevel_e { dc.bottom_right } else { dc.bottom };
    let west_top = if bevel_n { dc.top_left } else { dc.left };
    let west_bottom = if bevel_s { dc.bottom_left } else { dc.left };
    let east_top = if bevel_n { dc.top_right } else { dc.right };
    let east_bottom = if bevel_s { dc.bottom_right } else { dc.right };

    for py in 0..TILE_PX {
        for px in 0..TILE_PX {
            let fpx = px as f32;
            let fpy = py as f32;

            // Start with face color
            let mut color = dc.face;

            // Cardinal bevels with diagonal splits at convex corners.
            let in_n = bevel_n && fpy < bevel;
            let in_s = bevel_s && fpy >= size - bevel;
            let in_w = bevel_w && fpx < bevel;
            let in_e = bevel_e && fpx >= size - bevel;

            if in_n && in_w {
                color = if fpy < fpx {
                    let t = fpy / bevel;
                    let s = fpx / (size - 1.0);
                    lerp_rgb(lerp_rgb(north_left, north_right, s), dc.top, t)
                } else {
                    let t = fpx / bevel;
                    let s = fpy / (size - 1.0);
                    lerp_rgb(lerp_rgb(west_top, west_bottom, s), dc.left, t)
                };
            } else if in_n && in_e {
                color = if fpy < size - fpx {
                    let t = fpy / bevel;
                    let s = fpx / (size - 1.0);
                    lerp_rgb(lerp_rgb(north_left, north_right, s), dc.top, t)
                } else {
                    let t = (size - 1.0 - fpx) / bevel;
                    let s = fpy / (size - 1.0);
                    lerp_rgb(lerp_rgb(east_top, east_bottom, s), dc.right, t)
                };
            } else if in_s && in_w {
                color = if fpy >= size - fpx {
                    let t = (size - 1.0 - fpy) / bevel;
                    let s = fpx / (size - 1.0);
                    lerp_rgb(lerp_rgb(south_left, south_right, s), dc.bottom, t)
                } else {
                    let t = fpx / bevel;
                    let s = fpy / (size - 1.0);
                    lerp_rgb(lerp_rgb(west_top, west_bottom, s), dc.left, t)
                };
            } else if in_s && in_e {
                color = if fpy >= fpx {
                    let t = (size - 1.0 - fpy) / bevel;
                    let s = fpx / (size - 1.0);
                    lerp_rgb(lerp_rgb(south_left, south_right, s), dc.bottom, t)
                } else {
                    let t = (size - 1.0 - fpx) / bevel;
                    let s = fpy / (size - 1.0);
                    lerp_rgb(lerp_rgb(east_top, east_bottom, s), dc.right, t)
                };
            } else if in_n {
                let t = fpy / bevel;
                let s = fpx / (size - 1.0);
                color = lerp_rgb(lerp_rgb(north_left, north_right, s), dc.top, t);
            } else if in_s {
                let t = (size - 1.0 - fpy) / bevel;
                let s = fpx / (size - 1.0);
                color = lerp_rgb(lerp_rgb(south_left, south_right, s), dc.bottom, t);
            } else if in_w {
                let t = fpx / bevel;
                let s = fpy / (size - 1.0);
                color = lerp_rgb(lerp_rgb(west_top, west_bottom, s), dc.left, t);
            } else if in_e {
                let t = (size - 1.0 - fpx) / bevel;
                let s = fpy / (size - 1.0);
                color = lerp_rgb(lerp_rgb(east_top, east_bottom, s), dc.right, t);
            }

            // Concave corners override (higher Z than face and bevels)
            if inner_nw && fpx < bevel && fpy < bevel {
                color = if fpy >= fpx { dc.top } else { dc.left };
            }
            if inner_ne && fpx >= size - bevel && fpy < bevel {
                color = if fpy >= size - 1.0 - fpx { dc.top } else { dc.right };
            }
            if inner_sw && fpx < bevel && fpy >= size - bevel {
                color = if size - 1.0 - fpy >= fpx { dc.bottom } else { dc.left };
            }
            if inner_se && fpx >= size - bevel && fpy >= size - bevel {
                color = if size - 1.0 - fpy >= size - 1.0 - fpx { dc.bottom } else { dc.right };
            }

            // Edge line overlay
            color = apply_pixel_edge_lines(color, fpx, fpy, bevel, size,
                bevel_n, bevel_e, bevel_s, bevel_w,
                inner_nw, inner_ne, inner_sw, inner_se);

            let idx = ((origin_y + py) * img_w + (origin_x + px)) as usize * 4;
            pixels[idx] = (color[0].clamp(0.0, 1.0) * 255.0) as u8;
            pixels[idx + 1] = (color[1].clamp(0.0, 1.0) * 255.0) as u8;
            pixels[idx + 2] = (color[2].clamp(0.0, 1.0) * 255.0) as u8;
            pixels[idx + 3] = 255;
        }
    }
}

fn lerp_rgb(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn apply_pixel_edge_lines(
    color: [f32; 3],
    px: f32, py: f32,
    bevel: f32, size: f32,
    bevel_n: bool, bevel_e: bool, bevel_s: bool, bevel_w: bool,
    inner_nw: bool, inner_ne: bool, inner_sw: bool, inner_se: bool,
) -> [f32; 3] {
    let face_left = if bevel_w { bevel } else { 0.0 };
    let face_right = if bevel_e { size - bevel } else { size };
    let face_top = if bevel_n { bevel } else { 0.0 };
    let face_bottom = if bevel_s { size - bevel } else { size };

    let hw = EDGE_LINE_HALF_WIDTH;
    let lighter = (1.0_f32, 1.0_f32, 1.0_f32, OUTER_LINE_OPACITY);
    let darker = (0.0_f32, 0.0_f32, 0.0_f32, INNER_LINE_OPACITY);

    // Convex inner square edges (lighter)
    if bevel_n && px >= face_left && px <= face_right && (py - bevel).abs() < hw {
        return alpha_blend(color, lighter);
    }
    if bevel_s && px >= face_left && px <= face_right && (py - (size - bevel)).abs() < hw {
        return alpha_blend(color, lighter);
    }
    if bevel_w && py >= face_top && py <= face_bottom && (px - bevel).abs() < hw {
        return alpha_blend(color, lighter);
    }
    if bevel_e && py >= face_top && py <= face_bottom && (px - (size - bevel)).abs() < hw {
        return alpha_blend(color, lighter);
    }

    // Convex corner diagonals (darker)
    if bevel_n && bevel_w && dist_to_seg(px, py, bevel, bevel, 0.0, 0.0) < hw {
        return alpha_blend(color, darker);
    }
    if bevel_n && bevel_e && dist_to_seg(px, py, size - bevel, bevel, size - 1.0, 0.0) < hw {
        return alpha_blend(color, darker);
    }
    if bevel_s && bevel_w && dist_to_seg(px, py, bevel, size - bevel, 0.0, size - 1.0) < hw {
        return alpha_blend(color, darker);
    }
    if bevel_s && bevel_e && dist_to_seg(px, py, size - bevel, size - bevel, size - 1.0, size - 1.0) < hw {
        return alpha_blend(color, darker);
    }

    // Concave edge lines
    if inner_nw {
        let ix = bevel;
        let iy = bevel;
        if dist_to_seg(px, py, ix, iy, ix, 0.0) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, 0.0, iy) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, 0.0, 0.0) < hw { return alpha_blend(color, darker); }
    }
    if inner_ne {
        let ix = size - bevel;
        let iy = bevel;
        if dist_to_seg(px, py, ix, iy, ix, 0.0) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, size - 1.0, iy) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, size - 1.0, 0.0) < hw { return alpha_blend(color, darker); }
    }
    if inner_sw {
        let ix = bevel;
        let iy = size - bevel;
        if dist_to_seg(px, py, ix, iy, ix, size - 1.0) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, 0.0, iy) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, 0.0, size - 1.0) < hw { return alpha_blend(color, darker); }
    }
    if inner_se {
        let ix = size - bevel;
        let iy = size - bevel;
        if dist_to_seg(px, py, ix, iy, ix, size - 1.0) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, size - 1.0, iy) < hw { return alpha_blend(color, lighter); }
        if dist_to_seg(px, py, ix, iy, size - 1.0, size - 1.0) < hw { return alpha_blend(color, darker); }
    }

    color
}

fn alpha_blend(base: [f32; 3], overlay: (f32, f32, f32, f32)) -> [f32; 3] {
    let a = overlay.3;
    let inv = 1.0 - a;
    [base[0] * inv + overlay.0 * a, base[1] * inv + overlay.1 * a, base[2] * inv + overlay.2 * a]
}

fn dist_to_seg(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
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
