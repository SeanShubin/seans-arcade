//! Interactive 2D primitives lab — view and tweak all Bevy 2D shapes.
//!
//! Run with: `cargo run --example primitives_lab`
//!
//! - Primitives are rendered as Mesh2d in world space, laid out in a reflowing grid
//! - Egui side panel provides sliders with numeric input for each primitive's parameters
//! - Shared color and rotation controls affect all primitives

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use std::f32::consts::TAU;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const PRIMITIVE_COUNT: usize = 10;
const CELL_SIZE: f32 = 220.0;
const SHAPE_SCALE: f32 = 70.0;
const LABEL_OFFSET_Y: f32 = -90.0;
const PANEL_WIDTH: f32 = 300.0;

const PRIMITIVE_NAMES: [&str; PRIMITIVE_COUNT] = [
    "Circle",
    "Ellipse",
    "Annulus",
    "Rectangle",
    "Triangle",
    "Rhombus",
    "Capsule",
    "RegularPolygon",
    "CircularSector",
    "CircularSegment",
];

// ---------------------------------------------------------------------------
// Resources
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct PrimitiveParams {
    circle_radius: f32,
    ellipse_half_x: f32,
    ellipse_half_y: f32,
    annulus_inner: f32,
    annulus_outer: f32,
    rect_width: f32,
    rect_height: f32,
    tri_vertices: [Vec2; 3],
    rhombus_half_x: f32,
    rhombus_half_y: f32,
    capsule_radius: f32,
    capsule_half_length: f32,
    polygon_radius: f32,
    polygon_sides: u32,
    sector_radius: f32,
    sector_angle: f32,
    segment_radius: f32,
    segment_angle: f32,
    color: [f32; 3],
    rotation: f32,
}

impl Default for PrimitiveParams {
    fn default() -> Self {
        Self {
            circle_radius: 1.0,
            ellipse_half_x: 1.0,
            ellipse_half_y: 0.6,
            annulus_inner: 0.5,
            annulus_outer: 1.0,
            rect_width: 1.6,
            rect_height: 1.0,
            tri_vertices: [
                Vec2::new(0.0, 1.0),
                Vec2::new(-0.87, -0.5),
                Vec2::new(0.87, -0.5),
            ],
            rhombus_half_x: 1.0,
            rhombus_half_y: 0.7,
            capsule_radius: 0.4,
            capsule_half_length: 0.6,
            polygon_radius: 1.0,
            polygon_sides: 6,
            sector_radius: 1.0,
            sector_angle: TAU / 4.0,
            segment_radius: 1.0,
            segment_angle: TAU / 3.0,
            color: [0.3, 0.7, 1.0],
            rotation: 0.0,
        }
    }
}

#[derive(Resource, Default)]
struct ParamsDirty(bool);

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct PrimitiveEntity(usize);

#[derive(Component)]
struct PrimitiveLabel(usize);

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Primitives Lab".into(),
                    resolution: bevy::window::WindowResolution::new(1400, 800),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
        ))
        .insert_resource(PrimitiveParams::default())
        .init_resource::<ParamsDirty>()
        .add_systems(Startup, setup)
        .add_systems(bevy_egui::EguiPrimaryContextPass, egui_panel)
        .add_systems(Update, (update_grid_layout, regenerate_meshes))
        .run();
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    params: Res<PrimitiveParams>,
    asset_server: Res<AssetServer>,
) {
    commands.spawn(Camera2d);

    let material = materials.add(ColorMaterial::from_color(Color::srgb(
        params.color[0],
        params.color[1],
        params.color[2],
    )));

    let font: Handle<Font> = asset_server.load("local/fonts/FiraMono-Regular.ttf");

    for i in 0..PRIMITIVE_COUNT {
        let mesh_handle = meshes.add(build_mesh(i, &params));

        commands.spawn((
            PrimitiveEntity(i),
            Mesh2d(mesh_handle),
            MeshMaterial2d(material.clone()),
            Transform::from_scale(Vec3::splat(SHAPE_SCALE)),
        ));

        commands.spawn((
            PrimitiveLabel(i),
            Text2d::new(PRIMITIVE_NAMES[i]),
            TextFont {
                font: font.clone(),
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
        ));
    }
}

// ---------------------------------------------------------------------------
// Egui slider panel
// ---------------------------------------------------------------------------

fn egui_panel(mut contexts: EguiContexts, mut params: ResMut<PrimitiveParams>, mut dirty: ResMut<ParamsDirty>) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let mut changed = false;

    egui::SidePanel::left("controls")
        .default_width(PANEL_WIDTH)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Shared controls
                ui.heading("Shared");
                changed |= ui
                    .add(egui::Slider::new(&mut params.rotation, 0.0..=TAU).text("Rotation"))
                    .changed();
                ui.horizontal(|ui| {
                    ui.label("Color");
                    changed |= ui.color_edit_button_rgb(&mut params.color).changed();
                });
                ui.separator();

                // Circle
                ui.collapsing("Circle", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.circle_radius, 0.1..=2.0).text("Radius"))
                        .changed();
                });

                // Ellipse
                ui.collapsing("Ellipse", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.ellipse_half_x, 0.1..=2.0).text("Half X"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.ellipse_half_y, 0.1..=2.0).text("Half Y"))
                        .changed();
                });

                // Annulus
                ui.collapsing("Annulus", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.annulus_inner, 0.1..=1.5).text("Inner R"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.annulus_outer, 0.2..=2.0).text("Outer R"))
                        .changed();
                });

                // Rectangle
                ui.collapsing("Rectangle", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.rect_width, 0.1..=3.0).text("Width"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.rect_height, 0.1..=3.0).text("Height"))
                        .changed();
                });

                // Triangle
                ui.collapsing("Triangle", |ui| {
                    for (idx, label) in ["A", "B", "C"].iter().enumerate() {
                        ui.label(format!("Vertex {label}"));
                        changed |= ui
                            .add(egui::Slider::new(&mut params.tri_vertices[idx].x, -2.0..=2.0).text("X"))
                            .changed();
                        changed |= ui
                            .add(egui::Slider::new(&mut params.tri_vertices[idx].y, -2.0..=2.0).text("Y"))
                            .changed();
                    }
                });

                // Rhombus
                ui.collapsing("Rhombus", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.rhombus_half_x, 0.1..=2.0).text("Half Diag X"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.rhombus_half_y, 0.1..=2.0).text("Half Diag Y"))
                        .changed();
                });

                // Capsule
                ui.collapsing("Capsule", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.capsule_radius, 0.1..=1.5).text("Radius"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.capsule_half_length, 0.1..=2.0).text("Half Length"))
                        .changed();
                });

                // RegularPolygon
                ui.collapsing("RegularPolygon", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.polygon_radius, 0.1..=2.0).text("Radius"))
                        .changed();
                    let mut sides = params.polygon_sides as i32;
                    if ui
                        .add(egui::Slider::new(&mut sides, 3..=12).text("Sides"))
                        .changed()
                    {
                        params.polygon_sides = sides as u32;
                        changed = true;
                    }
                });

                // CircularSector
                ui.collapsing("CircularSector", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.sector_radius, 0.1..=2.0).text("Radius"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.sector_angle, 0.1..=TAU).text("Angle"))
                        .changed();
                });

                // CircularSegment
                ui.collapsing("CircularSegment", |ui| {
                    changed |= ui
                        .add(egui::Slider::new(&mut params.segment_radius, 0.1..=2.0).text("Radius"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(&mut params.segment_angle, 0.1..=TAU).text("Angle"))
                        .changed();
                });
            });
        });

    if changed {
        dirty.0 = true;
    }
}

// ---------------------------------------------------------------------------
// Grid layout — positions primitives in world space, reflowing by window width
// ---------------------------------------------------------------------------

fn update_grid_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut shapes: Query<(&PrimitiveEntity, &mut Transform), Without<PrimitiveLabel>>,
    mut labels: Query<(&PrimitiveLabel, &mut Transform), Without<PrimitiveEntity>>,
    mut camera: Query<&mut Transform, (With<Camera2d>, Without<PrimitiveEntity>, Without<PrimitiveLabel>)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    let available_width = window.width() - PANEL_WIDTH;
    let columns = (available_width / CELL_SIZE).floor().max(1.0) as usize;
    let rows = (PRIMITIVE_COUNT + columns - 1) / columns;

    let grid_width = columns as f32 * CELL_SIZE;
    let grid_height = rows as f32 * CELL_SIZE;

    // Place grid at origin — shapes are positioned relative to (0,0)
    for (prim, mut transform) in &mut shapes {
        let pos = grid_position(prim.0, columns, CELL_SIZE);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }

    for (label, mut transform) in &mut labels {
        let pos = grid_position(label.0, columns, CELL_SIZE);
        transform.translation.x = pos.x;
        transform.translation.y = pos.y + LABEL_OFFSET_Y;
    }

    // Camera centers on grid, then shifts right by half the panel width
    // so the grid appears centered in the visible area to the right of egui
    if let Ok(mut cam_transform) = camera.single_mut() {
        let grid_center_x = (grid_width - CELL_SIZE) / 2.0;
        let grid_center_y = -(grid_height - CELL_SIZE) / 2.0;
        let panel_offset_x = PANEL_WIDTH / 2.0;
        cam_transform.translation.x = grid_center_x - panel_offset_x;
        cam_transform.translation.y = grid_center_y;
    }
}

fn grid_position(index: usize, columns: usize, cell_size: f32) -> Vec2 {
    let col = index % columns;
    let row = index / columns;
    Vec2::new(col as f32 * cell_size, -(row as f32 * cell_size))
}

// ---------------------------------------------------------------------------
// Mesh regeneration — rebuilds meshes when parameters change
// ---------------------------------------------------------------------------

fn regenerate_meshes(
    mut dirty: ResMut<ParamsDirty>,
    params: Res<PrimitiveParams>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut shapes: Query<(&PrimitiveEntity, &Mesh2d, &MeshMaterial2d<ColorMaterial>, &mut Transform)>,
) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let color = Color::srgb(params.color[0], params.color[1], params.color[2]);

    for (prim, mesh_handle, mat_handle, mut transform) in &mut shapes {
        let mesh = build_mesh(prim.0, &params);
        if let Some(existing) = meshes.get_mut(&mesh_handle.0) {
            *existing = mesh;
        }

        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.color = color;
        }

        let scale = SHAPE_SCALE;
        transform.rotation = Quat::from_rotation_z(params.rotation);
        transform.scale = Vec3::splat(scale);
    }
}

// ---------------------------------------------------------------------------
// Mesh builder — pure function, maps index to a Bevy Mesh
// ---------------------------------------------------------------------------

fn build_mesh(index: usize, params: &PrimitiveParams) -> Mesh {
    match index {
        0 => Mesh::from(Circle::new(params.circle_radius)),
        1 => Mesh::from(Ellipse::new(params.ellipse_half_x, params.ellipse_half_y)),
        2 => Mesh::from(Annulus::new(params.annulus_inner, params.annulus_outer)),
        3 => Mesh::from(Rectangle::new(params.rect_width, params.rect_height)),
        4 => Mesh::from(Triangle2d::new(
            params.tri_vertices[0],
            params.tri_vertices[1],
            params.tri_vertices[2],
        )),
        5 => Mesh::from(Rhombus::new(params.rhombus_half_x, params.rhombus_half_y)),
        6 => Mesh::from(Capsule2d::new(params.capsule_radius, params.capsule_half_length)),
        7 => Mesh::from(RegularPolygon::new(params.polygon_radius, params.polygon_sides)),
        8 => Mesh::from(CircularSector::new(params.sector_radius, params.sector_angle)),
        9 => Mesh::from(CircularSegment::new(params.segment_radius, params.segment_angle)),
        _ => Mesh::from(Circle::new(1.0)),
    }
}
