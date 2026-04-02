//! Renders all 47 beveled blob tiles with physically-based bevel lighting.
//!
//! Uses the documented LDtk blob layout (12×5 grid).
//! Bevel width is 1/4 of tile size. Edge lines mark bevel boundaries:
//!   - Lighter gray at the outer edge (top of bevel ridge)
//!   - Darker gray at the inner edge (where bevel meets face)
//!   - No lines at tile boundaries (far edges between adjacent tiles)
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

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Beveled Blob 47".into(),
                resolution: bevy::window::WindowResolution::new(1200, 600),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(Update, (camera_zoom, camera_pan))
        .run();
}

struct BevelLighting {
    light_dir: Vec3,
    base_color: Vec3,
    ambient: f32,
    bevel_sin: f32,
    bevel_cos: f32,
}

impl BevelLighting {
    fn new() -> Self {
        let angle = std::f32::consts::FRAC_PI_8;
        let bevel_depth = 10.0_f32;
        let bevel_width = 16.0_f32; // 1/4 of 64
        let bevel_angle = (bevel_depth / bevel_width).atan();
        Self {
            light_dir: Vec3::new(-angle.cos(), angle.sin(), 2.0).normalize(),
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

    fn to_color(&self, normal: Vec3) -> Color {
        let b = self.brightness(normal);
        Color::srgb(
            self.base_color.x * b,
            self.base_color.y * b,
            self.base_color.z * b,
        )
    }

    fn face_color(&self) -> Color {
        self.to_color(Vec3::Z)
    }

    fn normal_for_dir(&self, dir: Vec2) -> Vec3 {
        Vec3::new(dir.x * self.bevel_sin, dir.y * self.bevel_sin, self.bevel_cos).normalize()
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let tile_size = 64.0_f32;
    let bevel = tile_size / 4.0; // 1/4 of tile
    let gap = 2.0_f32;
    let stride = tile_size + gap;
    let lighting = BevelLighting::new();
    let white_mat = materials.add(ColorMaterial::from_color(Color::WHITE));
    let face_mat = materials.add(ColorMaterial::from_color(lighting.face_color()));

    let light_mat = materials.add(ColorMaterial::from_color(Color::srgba(1.0, 1.0, 1.0, 0.04)));
    let dark_mat = materials.add(ColorMaterial::from_color(Color::srgba(0.0, 0.0, 0.0, 0.05)));

    let offset_x = -stride * 6.0;
    let offset_y = stride * 2.0;

    for &(col, row, mask) in &BLOB_LAYOUT {
        let cx = offset_x + col as f32 * stride;
        let cy = offset_y - row as f32 * stride;

        spawn_beveled_tile(
            &mut commands, &mut meshes, &face_mat, &white_mat,
            &light_mat, &dark_mat,
            &lighting, Vec2::new(cx, cy), tile_size, bevel, mask,
        );
    }
}

fn spawn_beveled_tile(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    face_mat: &Handle<ColorMaterial>,
    white_mat: &Handle<ColorMaterial>,
    light_mat: &Handle<ColorMaterial>,
    dark_mat: &Handle<ColorMaterial>,
    lighting: &BevelLighting,
    center: Vec2,
    tile_size: f32,
    bevel: f32,
    mask: u8,
) {
    let half = tile_size / 2.0;
    let inner = half - bevel;
    let has = |bit: u8| mask & bit != 0;

    let bevel_n = !has(N);
    let bevel_e = !has(E);
    let bevel_s = !has(S);
    let bevel_w = !has(W);

    // Face bounds — extends to tile edge where no bevel
    let face_l = if bevel_w { -inner } else { -half };
    let face_r = if bevel_e { inner } else { half };
    let face_t = if bevel_n { inner } else { half };
    let face_b = if bevel_s { -inner } else { -half };

    let face_w = face_r - face_l;
    let face_h = face_t - face_b;
    let face_cx = (face_l + face_r) / 2.0;
    let face_cy = (face_b + face_t) / 2.0;

    // Center face
    commands.spawn((
        Mesh2d(meshes.add(Rectangle::new(face_w, face_h))),
        MeshMaterial2d(face_mat.clone()),
        Transform::from_xyz(center.x + face_cx, center.y + face_cy, 1.0),
    ));

    // Bevel normals and colors
    let top_n = lighting.normal_for_dir(Vec2::Y);
    let bottom_n = lighting.normal_for_dir(Vec2::NEG_Y);
    let left_n = lighting.normal_for_dir(Vec2::NEG_X);
    let right_n = lighting.normal_for_dir(Vec2::X);

    let top_c = lighting.to_color(top_n);
    let bottom_c = lighting.to_color(bottom_n);
    let left_c = lighting.to_color(left_n);
    let right_c = lighting.to_color(right_n);

    let tl_c = lighting.to_color((top_n + left_n).normalize());
    let tr_c = lighting.to_color((top_n + right_n).normalize());
    let bl_c = lighting.to_color((bottom_n + left_n).normalize());
    let br_c = lighting.to_color((bottom_n + right_n).normalize());

    let outer_tl = Vec2::new(-half, half);
    let outer_tr = Vec2::new(half, half);
    let outer_bl = Vec2::new(-half, -half);
    let outer_br = Vec2::new(half, -half);

    let inner_tl = Vec2::new(face_l, face_t);
    let inner_tr = Vec2::new(face_r, face_t);
    let inner_bl = Vec2::new(face_l, face_b);
    let inner_br = Vec2::new(face_r, face_b);

    // Top bevel
    if bevel_n {
        let ol = if bevel_w { tl_c } else { top_c };
        let or_ = if bevel_e { tr_c } else { top_c };
        let outer_l = if bevel_w { outer_tl } else { Vec2::new(-half, half) };
        let outer_r = if bevel_e { outer_tr } else { Vec2::new(half, half) };
        spawn_quad(commands, meshes, white_mat, center, 0.5,
            outer_l, outer_r, inner_tr, inner_tl,
            ol, or_, top_c, top_c);
    }

    // Bottom bevel
    if bevel_s {
        let ol = if bevel_w { bl_c } else { bottom_c };
        let or_ = if bevel_e { br_c } else { bottom_c };
        let outer_l = if bevel_w { outer_bl } else { Vec2::new(-half, -half) };
        let outer_r = if bevel_e { outer_br } else { Vec2::new(half, -half) };
        spawn_quad(commands, meshes, white_mat, center, 0.5,
            inner_bl, inner_br, outer_r, outer_l,
            bottom_c, bottom_c, or_, ol);
    }

    // Left bevel
    if bevel_w {
        let ot = if bevel_n { tl_c } else { left_c };
        let ob = if bevel_s { bl_c } else { left_c };
        let outer_t = if bevel_n { outer_tl } else { Vec2::new(-half, half) };
        let outer_b = if bevel_s { outer_bl } else { Vec2::new(-half, -half) };
        spawn_quad(commands, meshes, white_mat, center, 0.5,
            outer_t, inner_tl, inner_bl, outer_b,
            ot, left_c, left_c, ob);
    }

    // Right bevel
    if bevel_e {
        let ot = if bevel_n { tr_c } else { right_c };
        let ob = if bevel_s { br_c } else { right_c };
        let outer_t = if bevel_n { outer_tr } else { Vec2::new(half, half) };
        let outer_b = if bevel_s { outer_br } else { Vec2::new(half, -half) };
        spawn_quad(commands, meshes, white_mat, center, 0.5,
            inner_tr, outer_t, outer_b, inner_br,
            right_c, ot, ob, right_c);
    }

    // Inner corner bevels — concave bevel where two cardinals present but diagonal absent
    // Same size as outer bevels (1/4 tile). The bevel slopes INTO the corner.
    // Triangle covers the corner area, with the hypotenuse running from
    // (corner + bevel along one axis) to (corner + bevel along other axis).
    // The diagonal runs OPPOSITE to outer corner diagonals:
    //   outer TL diagonal: corner→center (↘)
    //   inner NW diagonal: top-edge→left-edge (↙)

    // Concave corners — two triangular faces, each matching an adjacent cardinal bevel.
    // The diagonal runs from the inner point (1/4 toward center) to the tile corner.

    // NW: bottom-left face → north (top_c), top-right face → west (left_c)
    if has(N) && has(W) && !has(NW) {
        let outer = Vec2::new(-half, half);
        let top_edge = Vec2::new(-half + bevel, half);
        let inner_pt = Vec2::new(-half + bevel, half - bevel);
        let left_edge = Vec2::new(-half, half - bevel);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, left_edge, inner_pt, top_c);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, inner_pt, top_edge, left_c);
    }
    // NE: top-left face → east (right_c), bottom-right face → north (top_c)
    if has(N) && has(E) && !has(NE) {
        let outer = Vec2::new(half, half);
        let top_edge = Vec2::new(half - bevel, half);
        let inner_pt = Vec2::new(half - bevel, half - bevel);
        let right_edge = Vec2::new(half, half - bevel);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, top_edge, inner_pt, right_c);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, inner_pt, right_edge, top_c);
    }
    // SW: top-left face → south (bottom_c), bottom-right face → west (left_c)
    if has(S) && has(W) && !has(SW) {
        let outer = Vec2::new(-half, -half);
        let left_edge = Vec2::new(-half, -half + bevel);
        let inner_pt = Vec2::new(-half + bevel, -half + bevel);
        let bottom_edge = Vec2::new(-half + bevel, -half);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, left_edge, inner_pt, bottom_c);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, inner_pt, bottom_edge, left_c);
    }
    // SE: bottom-left face → east (right_c), top-right face → south (bottom_c)
    if has(S) && has(E) && !has(SE) {
        let outer = Vec2::new(half, -half);
        let right_edge = Vec2::new(half, -half + bevel);
        let inner_pt = Vec2::new(half - bevel, -half + bevel);
        let bottom_edge = Vec2::new(half - bevel, -half);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, bottom_edge, inner_pt, right_c);
        spawn_triangle(commands, meshes, white_mat, center, 1.5,
            outer, inner_pt, right_edge, bottom_c);
    }

    // ── Edge boundary lines ────────────────────────────────────────────
    // Subtle gray lines to show bevel boundaries:
    //   - Lighter gray at the OUTER edge (top of bevel ridge)
    //   - Darker gray at the INNER edge (where bevel meets face)
    //   - NO lines at tile boundaries (far edges between adjacent tiles)
    let line_w = 0.6_f32;

    // ── Convex edges ──────────────────────────────────────────────────
    // Outer square: no border (tile boundary)
    // Inner square: light border (top of bevel ridge)
    // Diagonal from inner corner to outer corner in same direction: dark

    // Inner square edges (light)
    if bevel_n {
        spawn_line(commands, meshes, light_mat, center, 2.0,
            inner_tl, inner_tr, line_w);
    }
    if bevel_s {
        spawn_line(commands, meshes, light_mat, center, 2.0,
            inner_bl, inner_br, line_w);
    }
    if bevel_w {
        spawn_line(commands, meshes, light_mat, center, 2.0,
            inner_tl, inner_bl, line_w);
    }
    if bevel_e {
        spawn_line(commands, meshes, light_mat, center, 2.0,
            inner_tr, inner_br, line_w);
    }

    // Convex corner diagonals: inner corner → outer corner (dark)
    if bevel_n && bevel_w {
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            inner_tl, outer_tl, line_w);
    }
    if bevel_n && bevel_e {
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            inner_tr, outer_tr, line_w);
    }
    if bevel_s && bevel_w {
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            inner_bl, outer_bl, line_w);
    }
    if bevel_s && bevel_e {
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            inner_br, outer_br, line_w);
    }

    // ── Concave edges ─────────────────────────────────────────────────
    if has(N) && has(W) && !has(NW) {
        let p = Vec2::new(-half + bevel, half - bevel);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(-half + bevel, half), line_w);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(-half, half - bevel), line_w);
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            p, Vec2::new(-half, half), line_w);
    }
    if has(N) && has(E) && !has(NE) {
        let p = Vec2::new(half - bevel, half - bevel);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(half - bevel, half), line_w);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(half, half - bevel), line_w);
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            p, Vec2::new(half, half), line_w);
    }
    if has(S) && has(W) && !has(SW) {
        let p = Vec2::new(-half + bevel, -half + bevel);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(-half + bevel, -half), line_w);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(-half, -half + bevel), line_w);
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            p, Vec2::new(-half, -half), line_w);
    }
    if has(S) && has(E) && !has(SE) {
        let p = Vec2::new(half - bevel, -half + bevel);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(half - bevel, -half), line_w);
        spawn_line(commands, meshes, light_mat, center, 2.0,
            p, Vec2::new(half, -half + bevel), line_w);
        spawn_line(commands, meshes, dark_mat, center, 2.0,
            p, Vec2::new(half, -half), line_w);
    }
}

fn spawn_quad(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    a: Vec2, b: Vec2, c: Vec2, d: Vec2,
    ca: Color, cb: Color, cc: Color, cd: Color,
) {
    commands.spawn((
        Mesh2d(meshes.add(gradient_quad(a, b, c, d, ca, cb, cc, cd))),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

fn spawn_triangle(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    a: Vec2, b: Vec2, c: Vec2,
    color: Color,
) {
    let arr = color_to_array(color);
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[a.x, a.y, 0.0], [b.x, b.y, 0.0], [c.x, c.y, 0.0]],
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, vec![arr, arr, arr])
    .with_inserted_indices(Indices::U32(vec![0, 1, 2]));
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

fn spawn_line(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    mat: &Handle<ColorMaterial>,
    offset: Vec2, z: f32,
    from: Vec2, to: Vec2,
    width: f32,
) {
    let dir = (to - from).normalize();
    let perp = Vec2::new(-dir.y, dir.x) * width * 0.5;
    let mesh = Mesh::new(
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
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    commands.spawn((
        Mesh2d(meshes.add(mesh)),
        MeshMaterial2d(mat.clone()),
        Transform::from_xyz(offset.x, offset.y, z),
    ));
}

fn color_to_array(c: Color) -> [f32; 4] {
    let lin = c.to_linear();
    [lin.red, lin.green, lin.blue, lin.alpha]
}

fn gradient_quad(
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
        vec![color_to_array(ca), color_to_array(cb), color_to_array(cc), color_to_array(cd)],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

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
    let scale = if let Projection::Orthographic(ortho) = projection {
        ortho.scale
    } else {
        1.0
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
