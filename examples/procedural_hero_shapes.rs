//! Procedural hero — Example 1: Shape Composition
//!
//! A static top-down hero assembled from filled geometric primitives using Mesh2d.
//!
//! Controls:
//!   - Scroll wheel: zoom in/out
//!   - Click + drag: pan camera
//!   - Home key: reset zoom and pan
//!
//! Run with: `cargo run --example procedural_hero_shapes`

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Window
const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

// Hero body proportions (pixels, relative to hero origin at feet)
const BODY_WIDTH: f32 = 20.0;
const BODY_HEIGHT: f32 = 16.0;
const BODY_Y: f32 = 16.0;

const HEAD_RADIUS: f32 = 10.0;
const HEAD_Y: f32 = 34.0;

const HAIR_RADIUS_RATIO: f32 = 0.7;
const HAIR_Y_OFFSET: f32 = 3.0;

const LEG_WIDTH: f32 = 7.0;
const LEG_HEIGHT: f32 = 10.0;
const LEG_SPREAD: f32 = 5.0;
const LEG_Y: f32 = 3.0;

const ARM_WIDTH: f32 = 6.0;
const ARM_HEIGHT: f32 = 12.0;
const ARM_X: f32 = 15.0;
const ARM_Y: f32 = 18.0;

const SHIELD_WIDTH: f32 = 8.0;
const SHIELD_HEIGHT: f32 = 12.0;
const SHIELD_X: f32 = -18.0;
const SHIELD_Y: f32 = 16.0;

const EYE_RADIUS: f32 = 1.5;
const EYE_SPREAD: f32 = 4.0;
const EYE_Y_OFFSET: f32 = -2.0;

// Hero palette
const COLOR_SKIN: Color = Color::srgb(0.93, 0.76, 0.57);
const COLOR_TUNIC: Color = Color::srgb(0.18, 0.55, 0.18);
const COLOR_PANTS: Color = Color::srgb(0.55, 0.40, 0.25);
const COLOR_SHIELD: Color = Color::srgb(0.25, 0.35, 0.65);
const COLOR_HAIR: Color = Color::srgb(0.75, 0.60, 0.20);
const COLOR_EYE: Color = Color::BLACK;

// Z-layers (higher = closer to camera = drawn on top)
const Z_LEGS: f32 = 0.0;
const Z_BODY: f32 = 1.0;
const Z_ARMS: f32 = 2.0;
const Z_SHIELD: f32 = 3.0;
const Z_HEAD: f32 = 4.0;
const Z_HAIR: f32 = 5.0;
const Z_EYES: f32 = 6.0;

// Zoom
const ZOOM_SPEED_LINE: f32 = 0.15;
const ZOOM_SPEED_PIXEL: f32 = 0.002;
const ZOOM_MIN: f32 = 0.02;
const ZOOM_MAX: f32 = 10.0;

// Camera reset
const DEFAULT_ZOOM_SCALE: f32 = 1.0;

// HUD
const HUD_FONT_SIZE: f32 = 20.0;
const HUD_MARGIN: f32 = 12.0;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

#[derive(Component)]
struct ZoomLabel;

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Procedural Hero — Shapes".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, (setup_camera, setup_hero, setup_hud))
        .add_systems(Update, (read_zoom_input, read_pan_input, read_reset_input, update_zoom_label))
        .run();
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

fn setup_hero(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    spawn_legs(&mut commands, &mut meshes, &mut materials);
    spawn_body(&mut commands, &mut meshes, &mut materials);
    spawn_arms(&mut commands, &mut meshes, &mut materials);
    spawn_shield(&mut commands, &mut meshes, &mut materials);
    spawn_head(&mut commands, &mut meshes, &mut materials);
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        ZoomLabel,
        Text::new(""),
        TextFont {
            font_size: HUD_FONT_SIZE,
            ..default()
        },
        Node {
            margin: UiRect::all(Val::Px(HUD_MARGIN)),
            ..default()
        },
    ));
}

// ---------------------------------------------------------------------------
// Input layer — translates raw input into camera changes
// ---------------------------------------------------------------------------

fn read_zoom_input(
    mut scroll_events: MessageReader<MouseWheel>,
    mut projection_query: Query<&mut Projection, With<Camera2d>>,
) {
    let mut zoom_delta = 0.0;
    for event in scroll_events.read() {
        zoom_delta += match event.unit {
            MouseScrollUnit::Line => -event.y * ZOOM_SPEED_LINE,
            MouseScrollUnit::Pixel => -event.y * ZOOM_SPEED_PIXEL,
        };
    }

    if zoom_delta == 0.0 {
        return;
    }

    let Ok(mut projection) = projection_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };
    let new_scale = ortho.scale * (1.0 + zoom_delta);
    ortho.scale = new_scale.clamp(ZOOM_MIN, ZOOM_MAX);
}

fn read_pan_input(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut motion_events: MessageReader<MouseMotion>,
    mut camera_query: Query<(&mut Transform, &Projection), With<Camera2d>>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        motion_events.read(); // drain to prevent stale events
        return;
    }

    let mut drag_delta = Vec2::ZERO;
    for event in motion_events.read() {
        drag_delta += event.delta;
    }

    if drag_delta == Vec2::ZERO {
        return;
    }

    let Ok((mut transform, projection)) = camera_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };

    // Scale drag by zoom level so panning feels consistent at any zoom
    let pan_offset_x = -drag_delta.x * ortho.scale;
    let pan_offset_y = drag_delta.y * ortho.scale;
    transform.translation.x += pan_offset_x;
    transform.translation.y += pan_offset_y;
}

fn read_reset_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    if !keys.just_pressed(KeyCode::Home) {
        return;
    }

    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };
    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    transform.translation = Vec3::ZERO;
    ortho.scale = DEFAULT_ZOOM_SCALE;
}

// ---------------------------------------------------------------------------
// Presentation layer — spawns hero body parts as Mesh2d entities
// ---------------------------------------------------------------------------

fn spawn_legs(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let leg_mesh = meshes.add(Rectangle::new(LEG_WIDTH, LEG_HEIGHT));
    let leg_material = materials.add(ColorMaterial::from_color(COLOR_PANTS));

    let left_leg_pos = Vec3::new(-LEG_SPREAD, LEG_Y, Z_LEGS);
    let right_leg_pos = Vec3::new(LEG_SPREAD, LEG_Y, Z_LEGS);

    commands.spawn((
        Mesh2d(leg_mesh.clone()),
        MeshMaterial2d(leg_material.clone()),
        Transform::from_translation(left_leg_pos),
    ));
    commands.spawn((
        Mesh2d(leg_mesh),
        MeshMaterial2d(leg_material),
        Transform::from_translation(right_leg_pos),
    ));
}

fn spawn_body(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let body_mesh = meshes.add(Rectangle::new(BODY_WIDTH, BODY_HEIGHT));
    let body_material = materials.add(ColorMaterial::from_color(COLOR_TUNIC));
    let body_pos = Vec3::new(0.0, BODY_Y, Z_BODY);

    commands.spawn((
        Mesh2d(body_mesh),
        MeshMaterial2d(body_material),
        Transform::from_translation(body_pos),
    ));
}

fn spawn_arms(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let arm_mesh = meshes.add(Rectangle::new(ARM_WIDTH, ARM_HEIGHT));
    let arm_material = materials.add(ColorMaterial::from_color(COLOR_SKIN));

    let left_arm_pos = Vec3::new(-ARM_X, ARM_Y, Z_ARMS);
    let right_arm_pos = Vec3::new(ARM_X, ARM_Y, Z_ARMS);

    commands.spawn((
        Mesh2d(arm_mesh.clone()),
        MeshMaterial2d(arm_material.clone()),
        Transform::from_translation(left_arm_pos),
    ));
    commands.spawn((
        Mesh2d(arm_mesh),
        MeshMaterial2d(arm_material),
        Transform::from_translation(right_arm_pos),
    ));
}

fn spawn_shield(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let shield_mesh = meshes.add(Rectangle::new(SHIELD_WIDTH, SHIELD_HEIGHT));
    let shield_material = materials.add(ColorMaterial::from_color(COLOR_SHIELD));
    let shield_pos = Vec3::new(SHIELD_X, SHIELD_Y, Z_SHIELD);

    commands.spawn((
        Mesh2d(shield_mesh),
        MeshMaterial2d(shield_material),
        Transform::from_translation(shield_pos),
    ));
}

fn spawn_head(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let head_mesh = meshes.add(Circle::new(HEAD_RADIUS));
    let head_material = materials.add(ColorMaterial::from_color(COLOR_SKIN));
    let head_pos = Vec3::new(0.0, HEAD_Y, Z_HEAD);

    commands.spawn((
        Mesh2d(head_mesh),
        MeshMaterial2d(head_material),
        Transform::from_translation(head_pos),
    ));

    let hair_radius = HEAD_RADIUS * HAIR_RADIUS_RATIO;
    let hair_mesh = meshes.add(Circle::new(hair_radius));
    let hair_material = materials.add(ColorMaterial::from_color(COLOR_HAIR));
    let hair_pos = Vec3::new(0.0, HEAD_Y + HAIR_Y_OFFSET, Z_HAIR);

    commands.spawn((
        Mesh2d(hair_mesh),
        MeshMaterial2d(hair_material),
        Transform::from_translation(hair_pos),
    ));

    let eye_mesh = meshes.add(Circle::new(EYE_RADIUS));
    let eye_material = materials.add(ColorMaterial::from_color(COLOR_EYE));
    let eye_y = HEAD_Y + EYE_Y_OFFSET;
    let left_eye_pos = Vec3::new(-EYE_SPREAD, eye_y, Z_EYES);
    let right_eye_pos = Vec3::new(EYE_SPREAD, eye_y, Z_EYES);

    commands.spawn((
        Mesh2d(eye_mesh.clone()),
        MeshMaterial2d(eye_material.clone()),
        Transform::from_translation(left_eye_pos),
    ));
    commands.spawn((
        Mesh2d(eye_mesh),
        MeshMaterial2d(eye_material),
        Transform::from_translation(right_eye_pos),
    ));
}

// ---------------------------------------------------------------------------
// HUD — displays current zoom level
// ---------------------------------------------------------------------------

fn update_zoom_label(
    projection_query: Query<&Projection, With<Camera2d>>,
    mut label_query: Query<&mut Text, With<ZoomLabel>>,
) {
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let Ok(mut text) = label_query.single_mut() else {
        return;
    };

    let zoom_percentage = 1.0 / ortho.scale;
    *text = Text::new(zoom_display_text(zoom_percentage));
}

fn zoom_display_text(zoom_percentage: f32) -> String {
    format!("Zoom: {:.0}%", zoom_percentage * 100.0)
}
