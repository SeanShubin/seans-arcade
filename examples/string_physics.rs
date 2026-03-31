//! String physics simulation — rectangles connected by strings with gravity.
//!
//! Drag the anchor (green rectangle) with the mouse and watch the chain follow.
//!
//! Run with: `cargo run --example string_physics`

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const GRAVITY: f32 = 980.0;
const CONSTRAINT_ITERATIONS: usize = 8;
const RECT_WIDTH: f32 = 60.0;
const RECT_HEIGHT: f32 = 30.0;
const STRING_LENGTH: f32 = 80.0;
const NODE_COUNT: usize = 8;
const DAMPING: f32 = 0.99;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (handle_mouse, apply_gravity, solve_constraints, draw_strings).chain())
        .run();
}

/// A point mass in the Verlet simulation.
#[derive(Component)]
struct VerletNode {
    prev_pos: Vec2,
    mass: f32,
}

/// Marks the anchor node that the mouse drags.
#[derive(Component)]
struct Anchor;

/// Whether the user is currently dragging the anchor.
#[derive(Resource, Default)]
struct DragState {
    dragging: bool,
}

/// A distance constraint (string) between two entities.
#[derive(Resource)]
struct Constraints {
    pairs: Vec<(Entity, Entity, f32)>,
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(DragState::default());

    let mut entities = Vec::new();
    for i in 0..NODE_COUNT {
        let x = 0.0;
        let y = 200.0 - i as f32 * STRING_LENGTH;
        let pos = Vec2::new(x, y);

        let is_anchor = i == 0;
        let color = if is_anchor {
            Color::srgb(0.2, 0.8, 0.2)
        } else {
            let t = i as f32 / (NODE_COUNT - 1) as f32;
            Color::srgb(0.9, 0.4 + t * 0.4, 0.2)
        };

        let mut entity_commands = commands.spawn((
            Sprite {
                color,
                custom_size: Some(Vec2::new(RECT_WIDTH, RECT_HEIGHT)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, 0.0),
            VerletNode {
                prev_pos: pos,
                mass: if is_anchor { 0.0 } else { 1.0 },
            },
        ));

        if is_anchor {
            entity_commands.insert(Anchor);
        }

        entities.push(entity_commands.id());
    }

    // Create string constraints between consecutive nodes
    let mut pairs = Vec::new();
    for i in 0..entities.len() - 1 {
        pairs.push((entities[i], entities[i + 1], STRING_LENGTH));
    }
    commands.insert_resource(Constraints { pairs });
}

fn handle_mouse(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    mut drag: ResMut<DragState>,
    mut anchor_query: Query<(&mut Transform, &mut VerletNode), With<Anchor>>,
) {
    let Ok(window) = windows.single() else { return };
    let Ok((camera, cam_transform)) = cameras.single() else { return };

    if mouse.just_pressed(MouseButton::Left) {
        drag.dragging = true;
    }
    if mouse.just_released(MouseButton::Left) {
        drag.dragging = false;
    }

    if drag.dragging {
        if let Some(cursor_pos) = window.cursor_position() {
            if let Ok(world_pos) = camera.viewport_to_world_2d(cam_transform, cursor_pos) {
                let Ok((mut transform, mut node)) = anchor_query.single_mut() else { return };
                let new_pos = Vec2::new(world_pos.x, world_pos.y);
                node.prev_pos = new_pos;
                transform.translation.x = new_pos.x;
                transform.translation.y = new_pos.y;
            }
        }
    }
}

fn apply_gravity(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut VerletNode)>,
) {
    let dt = time.delta_secs().min(0.033); // cap to avoid explosions

    for (mut transform, mut node) in &mut query {
        if node.mass == 0.0 {
            continue; // anchor — skip physics
        }

        let pos = Vec2::new(transform.translation.x, transform.translation.y);
        let velocity = (pos - node.prev_pos) * DAMPING;
        let new_pos = pos + velocity + Vec2::new(0.0, -GRAVITY) * dt * dt;

        node.prev_pos = pos;
        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
    }
}

fn solve_constraints(
    constraints: Res<Constraints>,
    mut query: Query<(&mut Transform, &VerletNode)>,
) {
    for _ in 0..CONSTRAINT_ITERATIONS {
        for &(e1, e2, length) in &constraints.pairs {
            let Ok([(mut t1, n1), (mut t2, n2)]) = query.get_many_mut([e1, e2]) else {
                continue;
            };

            let p1 = Vec2::new(t1.translation.x, t1.translation.y);
            let p2 = Vec2::new(t2.translation.x, t2.translation.y);
            let delta = p2 - p1;
            let dist = delta.length();
            if dist < 0.001 {
                continue;
            }

            let diff = (dist - length) / dist;
            let correction = delta * diff * 0.5;

            let total_mass = n1.mass + n2.mass;
            if total_mass == 0.0 {
                continue;
            }

            let w1 = n1.mass / total_mass;
            let w2 = n2.mass / total_mass;

            t1.translation.x += correction.x * w1;
            t1.translation.y += correction.y * w1;
            t2.translation.x -= correction.x * w2;
            t2.translation.y -= correction.y * w2;
        }
    }
}

fn draw_strings(
    mut gizmos: Gizmos,
    constraints: Res<Constraints>,
    query: Query<&Transform>,
) {
    for &(e1, e2, _) in &constraints.pairs {
        let Ok([t1, t2]) = query.get_many([e1, e2]) else { continue };
        gizmos.line_2d(
            Vec2::new(t1.translation.x, t1.translation.y),
            Vec2::new(t2.translation.x, t2.translation.y),
            Color::srgb(0.7, 0.7, 0.7),
        );
    }
}
