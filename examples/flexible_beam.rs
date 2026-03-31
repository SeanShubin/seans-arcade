//! Pigtail hair simulation using Avian2D physics.
//!
//! A chain of rectangles connected by revolute joints with angular motors.
//! Near the binding the hair holds its angle; toward the tips gravity wins.
//! Drag the green anchor with the mouse. Adjust spring frequency and damping
//! with the sliders.
//!
//! Run with: `cargo run --example flexible_beam`

use avian2d::{math::*, prelude::*};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, egui};

const SEGMENT_COUNT: usize = 16;
const SEGMENT_WIDTH: f32 = 30.0;
const SEGMENT_HEIGHT: f32 = 10.0;
const SEGMENT_SPACING: f32 = 32.0;
const DEFAULT_FREQUENCY: f32 = 5.0;
const DEFAULT_DAMPING_RATIO: f32 = 1.0;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins,
            EguiPlugin::default(),
            PhysicsPlugins::default(),
        ))
        .insert_resource(ClearColor(Color::srgb(0.05, 0.05, 0.1)))
        .insert_resource(SubstepCount(50))
        .insert_resource(Gravity(Vector::NEG_Y * 500.0))
        .init_resource::<HairConfig>()
        .add_systems(Startup, setup)
        .add_systems(Update, (follow_mouse, sync_joint_motors))
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

#[derive(Component)]
struct FollowMouse;

#[derive(Component)]
struct HairJoint;

#[derive(Resource)]
struct HairConfig {
    /// Spring natural frequency in Hz. Higher = stiffer.
    frequency: f32,
    frequency_text: String,
    /// Damping ratio. 1.0 = critically damped, <1 = bouncy, >1 = sluggish.
    damping_ratio: f32,
    damping_text: String,
}

impl Default for HairConfig {
    fn default() -> Self {
        Self {
            frequency: DEFAULT_FREQUENCY,
            frequency_text: format!("{DEFAULT_FREQUENCY:.1}"),
            damping_ratio: DEFAULT_DAMPING_RATIO,
            damping_text: format!("{DEFAULT_DAMPING_RATIO:.1}"),
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Lay out hair at 45 degrees
    let angle = std::f32::consts::FRAC_PI_4;
    let dir = Vec2::new(angle.cos(), angle.sin());
    let anchor_pos = Vec2::new(-200.0, -100.0);

    // Kinematic anchor (green)
    let mut previous = commands
        .spawn((
            Sprite {
                color: Color::srgb(0.2, 0.8, 0.2),
                custom_size: Some(Vec2::new(SEGMENT_WIDTH, SEGMENT_HEIGHT)),
                ..default()
            },
            Transform::from_translation(anchor_pos.extend(0.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            RigidBody::Kinematic,
            FollowMouse,
        ))
        .id();

    // Dynamic segments
    for i in 1..SEGMENT_COUNT {
        let pos = anchor_pos + dir * (i as f32 * SEGMENT_SPACING);
        let t = i as f32 / (SEGMENT_COUNT - 1) as f32;
        let color = Color::srgb(0.6 + t * 0.3, 0.35 + t * 0.1, 0.15);

        let current = commands
            .spawn((
                Sprite {
                    color,
                    custom_size: Some(Vec2::new(SEGMENT_WIDTH, SEGMENT_HEIGHT)),
                    ..default()
                },
                Transform::from_translation(pos.extend(0.0))
                    .with_rotation(Quat::from_rotation_z(angle)),
                RigidBody::Dynamic,
                MassPropertiesBundle::from_shape(
                    &Rectangle::new(SEGMENT_WIDTH, SEGMENT_HEIGHT),
                    1.0,
                ),
            ))
            .id();

        // Revolute joint with angular motor acting as a spring.
        // Target position 0 = no relative rotation = straight continuation.
        let half_w = SEGMENT_WIDTH / 2.0;
        let motor = AngularMotor::new(MotorModel::SpringDamper {
            frequency: DEFAULT_FREQUENCY as Scalar,
            damping_ratio: DEFAULT_DAMPING_RATIO as Scalar,
        })
        .with_target_position(0.0);

        commands.spawn((
            HairJoint,
            RevoluteJoint::new(previous, current)
                .with_local_anchor1(Vector::X * half_w as Scalar)
                .with_local_anchor2(Vector::NEG_X * half_w as Scalar)
                .with_point_compliance(0.0000001)
                .with_motor(motor),
        ));

        previous = current;
    }
}

fn follow_mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut follower: Query<&mut Transform, With<FollowMouse>>,
    mut contexts: EguiContexts,
) {
    let egui_wants_pointer = contexts.ctx_mut().unwrap().wants_pointer_input();

    if buttons.pressed(MouseButton::Left) && !egui_wants_pointer {
        let Ok(window) = windows.single() else { return };
        let Ok((camera, camera_transform)) = camera.single() else { return };
        let Ok(mut follower_pos) = follower.single_mut() else { return };

        if let Some(world_pos) = window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        {
            follower_pos.translation = world_pos.extend(follower_pos.translation.z);
        }
    }
}

fn sync_joint_motors(
    config: Res<HairConfig>,
    mut joints: Query<&mut RevoluteJoint, With<HairJoint>>,
) {
    if !config.is_changed() {
        return;
    }
    for mut joint in &mut joints {
        joint.motor.motor_model = MotorModel::SpringDamper {
            frequency: config.frequency as Scalar,
            damping_ratio: config.damping_ratio as Scalar,
        };
    }
}

fn ui_system(mut contexts: EguiContexts, mut config: ResMut<HairConfig>) {
    egui::Window::new("Hair Controls")
        .default_pos([10.0, 10.0])
        .show(contexts.ctx_mut().unwrap(), |ui| {
            // Frequency slider
            ui.label("Spring Frequency Hz (0 = rope, higher = stiffer)");
            let prev_f = config.frequency;
            ui.add(
                egui::Slider::new(&mut config.frequency, 0.0..=30.0)
                    .step_by(0.1),
            );
            if (config.frequency - prev_f).abs() > 0.01 {
                config.frequency_text = format!("{:.1}", config.frequency);
            }
            ui.horizontal(|ui| {
                ui.label("Value:");
                let response = ui.text_edit_singleline(&mut config.frequency_text);
                if response.lost_focus() {
                    if let Ok(val) = config.frequency_text.parse::<f32>() {
                        config.frequency = val.max(0.0);
                        config.frequency_text = format!("{:.1}", config.frequency);
                    } else {
                        config.frequency_text = format!("{:.1}", config.frequency);
                    }
                }
            });

            ui.add_space(10.0);

            // Damping ratio slider
            ui.label("Damping Ratio (1.0 = critically damped)");
            let prev_d = config.damping_ratio;
            ui.add(
                egui::Slider::new(&mut config.damping_ratio, 0.0..=3.0)
                    .step_by(0.1),
            );
            if (config.damping_ratio - prev_d).abs() > 0.01 {
                config.damping_text = format!("{:.1}", config.damping_ratio);
            }
            ui.horizontal(|ui| {
                ui.label("Value:");
                let response = ui.text_edit_singleline(&mut config.damping_text);
                if response.lost_focus() {
                    if let Ok(val) = config.damping_text.parse::<f32>() {
                        config.damping_ratio = val.max(0.0);
                        config.damping_text = format!("{:.1}", config.damping_ratio);
                    } else {
                        config.damping_text = format!("{:.1}", config.damping_ratio);
                    }
                }
            });
        });
}
