//! Procedural robot prototype.
//!
//! Demonstrates modular robot construction from geometric primitives with
//! configurable locomotion (wheels, treads, legs, hover) and weapon systems
//! (contact, melee, ranged). Robots are assembled from a part tree and
//! animated parametrically.
//!
//! Controls:
//!   Arrow keys / WASD — move robot
//!   Space — attack animation
//!   1-4 — switch locomotion (wheels, treads, legs, hover)
//!   5-7 — switch weapon (contact, melee, ranged)
//!   Scroll wheel — zoom
//!   Middle mouse drag — pan
//!   F1 — toggle debug gizmos
//!   [ / ] — decrease / increase animation speed
//!   \ — reset animation speed
//!
//! Run with: `cargo run --example procedural_robot`

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

// =====================================================================
// Constants
// =====================================================================

const WINDOW_WIDTH: u32 = 900;
const WINDOW_HEIGHT: u32 = 700;

// Chassis
const CHASSIS_W: f32 = 0.7;
const CHASSIS_H: f32 = 0.35;
const CHASSIS_D: f32 = 0.5;
const CHASSIS_Y: f32 = 0.55;

// Head / sensor dome
const HEAD_RADIUS: f32 = 0.18;
const HEAD_Y: f32 = CHASSIS_Y + CHASSIS_H / 2.0 + HEAD_RADIUS * 0.7;

// Eye / sensor
const EYE_RADIUS: f32 = 0.06;
const EYE_FORWARD: f32 = 0.15;

// Arms
const ARM_W: f32 = 0.12;
const ARM_H: f32 = 0.4;
const ARM_D: f32 = 0.12;
const ARM_X: f32 = CHASSIS_W / 2.0 + ARM_W / 2.0;
const ARM_Y: f32 = CHASSIS_Y + CHASSIS_H / 2.0 - 0.05;

// Legs (when using leg locomotion)
const LEG_W: f32 = 0.14;
const LEG_H: f32 = 0.45;
const LEG_D: f32 = 0.14;
const LEG_SPREAD: f32 = 0.2;
const LEG_Y: f32 = CHASSIS_Y - CHASSIS_H / 2.0;

// Wheels
const WHEEL_RADIUS: f32 = 0.18;
const WHEEL_WIDTH: f32 = 0.1;
const WHEEL_X: f32 = CHASSIS_W / 2.0 + WHEEL_WIDTH / 2.0 + 0.02;
const WHEEL_Y: f32 = 0.18;
const WHEEL_Z_SPREAD: f32 = 0.15;

// Treads
const TREAD_W: f32 = 0.12;
const TREAD_H: f32 = 0.2;
const TREAD_D: f32 = 0.5;
const TREAD_X: f32 = CHASSIS_W / 2.0 + TREAD_W / 2.0 + 0.02;
const TREAD_Y: f32 = 0.15;

// Hover pads
const HOVER_RADIUS: f32 = 0.12;
const HOVER_HEIGHT: f32 = 0.06;
const HOVER_Y: f32 = 0.05;

// Weapons
const BLADE_W: f32 = 0.06;
const BLADE_H: f32 = 0.5;
const BLADE_D: f32 = 0.04;

const BARREL_RADIUS: f32 = 0.04;
const BARREL_LENGTH: f32 = 0.5;

const RAM_W: f32 = 0.8;
const RAM_H: f32 = 0.2;
const RAM_D: f32 = 0.15;

// Movement
const MOVE_SPEED: f32 = 4.0;
const HOVER_MOVE_SPEED: f32 = 5.5;

// Animation
const WALK_CYCLE_SPEED: f32 = 10.0;
const LEG_SWING: f32 = 0.4;
const ARM_SWING: f32 = 0.25;
const WHEEL_SPIN_RATE: f32 = 8.0;
const HOVER_BOB_FREQ: f32 = 3.0;
const HOVER_BOB_AMP: f32 = 0.04;
const TREAD_BOB: f32 = 0.01;
const ATTACK_DURATION: f32 = 0.4;

// Camera
const ISO_DISTANCE: f32 = 15.0;
const ISO_PITCH: f32 = 35.264;
const ISO_YAW: f32 = 45.0;
const ZOOM_SPEED_LINE: f32 = 0.15;
const ZOOM_SPEED_PIXEL: f32 = 0.002;
const ZOOM_MIN: f32 = 0.002;
const ZOOM_MAX: f32 = 0.5;
const DEFAULT_ZOOM: f32 = 0.012;

// Palette
const COLOR_CHASSIS: Color = Color::srgb(0.45, 0.45, 0.50);
const COLOR_DARK_METAL: Color = Color::srgb(0.25, 0.25, 0.28);
const COLOR_HEAD: Color = Color::srgb(0.55, 0.55, 0.60);
const COLOR_EYE: Color = Color::srgb(0.9, 0.2, 0.1);
const COLOR_ACCENT: Color = Color::srgb(0.8, 0.5, 0.1);
const COLOR_WEAPON: Color = Color::srgb(0.6, 0.25, 0.25);
const COLOR_HOVER_GLOW: Color = Color::srgb(0.3, 0.7, 1.0);
const COLOR_BARREL: Color = Color::srgb(0.3, 0.3, 0.35);

// =====================================================================
// Robot configuration
// =====================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LocoType {
    Wheels,
    Treads,
    Legs,
    Hover,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WeaponType {
    Contact,  // ram / body slam
    Melee,    // blade arms
    Ranged,   // barrel / turret
}

#[derive(Resource)]
struct RobotConfig {
    locomotion: LocoType,
    weapon: WeaponType,
}

impl Default for RobotConfig {
    fn default() -> Self {
        Self {
            locomotion: LocoType::Wheels,
            weapon: WeaponType::Melee,
        }
    }
}

// =====================================================================
// Components
// =====================================================================

#[derive(Component)]
struct Robot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum RobotPart {
    Chassis,
    Head,
    Eye,
    LeftArm,
    RightArm,
    // Locomotion parts
    WheelFL, WheelFR, WheelBL, WheelBR,
    TreadLeft, TreadRight,
    LegLeft, LegRight,
    HoverFL, HoverFR, HoverBL, HoverBR,
    // Weapon parts
    Ram,
    BladeLeft, BladeRight,
    Barrel,
}

#[derive(Component, Default)]
struct WalkAnim {
    phase: f32,
    active: bool,
}

#[derive(Component, Default)]
struct FacingAngle(f32);

#[derive(Component, Default)]
struct AttackAnim {
    timer: f32,
    active: bool,
}

#[derive(Component)]
struct IsoCamera;

#[derive(Component)]
struct HudLabel;

#[derive(Message)]
struct MoveIntent(Vec2);

#[derive(Resource)]
struct DebugSettings {
    show_gizmos: bool,
    anim_speed: f32,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self { show_gizmos: false, anim_speed: 1.0 }
    }
}

#[derive(Resource)]
struct NeedsRebuild(bool);

// =====================================================================
// Entry point
// =====================================================================

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Procedural Robot".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_message::<MoveIntent>()
        .insert_resource(RobotConfig::default())
        .insert_resource(DebugSettings::default())
        .insert_resource(NeedsRebuild(true))
        .add_systems(Startup, (setup_scene, setup_hud))
        .add_systems(Update, (
            read_keyboard,
            read_config_keys,
            rebuild_robot,
            apply_movement,
            animate_robot,
            tick_attack_timer,
            update_camera,
            camera_zoom,
            camera_pan,
            update_hud,
            draw_gizmos,
        ))
        .run();
}

// =====================================================================
// Scene setup
// =====================================================================

fn setup_scene(mut commands: Commands) {
    // Camera
    let pitch_rad = ISO_PITCH.to_radians();
    let yaw_rad = ISO_YAW.to_radians();
    let rotation = Quat::from_euler(EulerRot::YXZ, -yaw_rad, -pitch_rad, 0.0);
    let position = rotation * Vec3::new(0.0, 0.0, ISO_DISTANCE);

    commands.spawn((
        IsoCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: DEFAULT_ZOOM,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.4, 0.0)),
    ));

    // Ground plane
    commands.spawn((
        Mesh3d(Handle::default()),
        Transform::default(),
    ));
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        HudLabel,
        Text::new(""),
        TextFont { font_size: 18.0, ..default() },
        Node { margin: UiRect::all(Val::Px(10.0)), ..default() },
    ));
}

// =====================================================================
// Robot construction
// =====================================================================

fn rebuild_robot(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<RobotConfig>,
    mut needs_rebuild: ResMut<NeedsRebuild>,
    existing: Query<Entity, With<Robot>>,
) {
    if !needs_rebuild.0 { return; }
    needs_rebuild.0 = false;

    // Despawn existing robot
    for e in &existing {
        commands.entity(e).despawn();
    }

    // Root entity
    let robot = commands.spawn((
        Robot,
        WalkAnim::default(),
        FacingAngle::default(),
        AttackAnim::default(),
        Transform::default(),
        Visibility::default(),
    )).id();

    // Chassis
    spawn_part(&mut commands, robot, RobotPart::Chassis,
        meshes.add(Cuboid::new(CHASSIS_W, CHASSIS_H, CHASSIS_D)),
        materials.add(StandardMaterial::from_color(COLOR_CHASSIS)),
        Vec3::new(0.0, CHASSIS_Y, 0.0), Vec3::ZERO);

    // Head
    spawn_part(&mut commands, robot, RobotPart::Head,
        meshes.add(Sphere::new(HEAD_RADIUS)),
        materials.add(StandardMaterial::from_color(COLOR_HEAD)),
        Vec3::new(0.0, HEAD_Y, 0.0), Vec3::ZERO);

    // Eye
    spawn_part(&mut commands, robot, RobotPart::Eye,
        meshes.add(Sphere::new(EYE_RADIUS)),
        materials.add(StandardMaterial {
            base_color: COLOR_EYE,
            emissive: COLOR_EYE.into(),
            ..default()
        }),
        Vec3::new(0.0, HEAD_Y, EYE_FORWARD), Vec3::ZERO);

    // Arms
    spawn_pivoted(&mut commands, robot, RobotPart::LeftArm,
        meshes.add(Cuboid::new(ARM_W, ARM_H, ARM_D)),
        materials.add(StandardMaterial::from_color(COLOR_DARK_METAL)),
        Vec3::new(-ARM_X, ARM_Y, 0.0),
        Vec3::new(0.0, -ARM_H / 2.0, 0.0));

    spawn_pivoted(&mut commands, robot, RobotPart::RightArm,
        meshes.add(Cuboid::new(ARM_W, ARM_H, ARM_D)),
        materials.add(StandardMaterial::from_color(COLOR_DARK_METAL)),
        Vec3::new(ARM_X, ARM_Y, 0.0),
        Vec3::new(0.0, -ARM_H / 2.0, 0.0));

    // Locomotion
    match config.locomotion {
        LocoType::Wheels => {
            let wheel_mesh = meshes.add(Cylinder::new(WHEEL_RADIUS, WHEEL_WIDTH));
            let wheel_mat = materials.add(StandardMaterial::from_color(COLOR_DARK_METAL));
            for (part, sx, sz) in [
                (RobotPart::WheelFL, -1.0f32, 1.0f32),
                (RobotPart::WheelFR, 1.0, 1.0),
                (RobotPart::WheelBL, -1.0, -1.0),
                (RobotPart::WheelBR, 1.0, -1.0),
            ] {
                spawn_pivoted(&mut commands, robot, part,
                    wheel_mesh.clone(), wheel_mat.clone(),
                    Vec3::new(sx * WHEEL_X, WHEEL_Y, sz * WHEEL_Z_SPREAD),
                    Vec3::ZERO);
            }
        }
        LocoType::Treads => {
            let tread_mesh = meshes.add(Cuboid::new(TREAD_W, TREAD_H, TREAD_D));
            let tread_mat = materials.add(StandardMaterial::from_color(COLOR_DARK_METAL));
            spawn_part(&mut commands, robot, RobotPart::TreadLeft,
                tread_mesh.clone(), tread_mat.clone(),
                Vec3::new(-TREAD_X, TREAD_Y, 0.0), Vec3::ZERO);
            spawn_part(&mut commands, robot, RobotPart::TreadRight,
                tread_mesh, tread_mat,
                Vec3::new(TREAD_X, TREAD_Y, 0.0), Vec3::ZERO);
        }
        LocoType::Legs => {
            let leg_mesh = meshes.add(Cuboid::new(LEG_W, LEG_H, LEG_D));
            let leg_mat = materials.add(StandardMaterial::from_color(COLOR_DARK_METAL));
            spawn_pivoted(&mut commands, robot, RobotPart::LegLeft,
                leg_mesh.clone(), leg_mat.clone(),
                Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
                Vec3::new(0.0, -LEG_H / 2.0, 0.0));
            spawn_pivoted(&mut commands, robot, RobotPart::LegRight,
                leg_mesh, leg_mat,
                Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
                Vec3::new(0.0, -LEG_H / 2.0, 0.0));
        }
        LocoType::Hover => {
            let hover_mesh = meshes.add(Cylinder::new(HOVER_RADIUS, HOVER_HEIGHT));
            let hover_mat = materials.add(StandardMaterial {
                base_color: COLOR_HOVER_GLOW,
                emissive: COLOR_HOVER_GLOW.into(),
                ..default()
            });
            for (part, sx, sz) in [
                (RobotPart::HoverFL, -1.0f32, 1.0f32),
                (RobotPart::HoverFR, 1.0, 1.0),
                (RobotPart::HoverBL, -1.0, -1.0),
                (RobotPart::HoverBR, 1.0, -1.0),
            ] {
                spawn_part(&mut commands, robot, part,
                    hover_mesh.clone(), hover_mat.clone(),
                    Vec3::new(sx * 0.25, HOVER_Y, sz * 0.18), Vec3::ZERO);
            }
        }
    }

    // Weapon
    match config.weapon {
        WeaponType::Contact => {
            // Ram plate on front
            spawn_part(&mut commands, robot, RobotPart::Ram,
                meshes.add(Cuboid::new(RAM_W, RAM_H, RAM_D)),
                materials.add(StandardMaterial::from_color(COLOR_ACCENT)),
                Vec3::new(0.0, CHASSIS_Y - 0.05, CHASSIS_D / 2.0 + RAM_D / 2.0), Vec3::ZERO);
        }
        WeaponType::Melee => {
            // Blades attached to arms
            let blade_mesh = meshes.add(Cuboid::new(BLADE_W, BLADE_H, BLADE_D));
            let blade_mat = materials.add(StandardMaterial::from_color(COLOR_WEAPON));
            spawn_part(&mut commands, robot, RobotPart::BladeLeft,
                blade_mesh.clone(), blade_mat.clone(),
                Vec3::new(-ARM_X, ARM_Y - ARM_H / 2.0 - BLADE_H / 2.0, 0.0), Vec3::ZERO);
            spawn_part(&mut commands, robot, RobotPart::BladeRight,
                blade_mesh, blade_mat,
                Vec3::new(ARM_X, ARM_Y - ARM_H / 2.0 - BLADE_H / 2.0, 0.0), Vec3::ZERO);
        }
        WeaponType::Ranged => {
            // Barrel on top
            spawn_pivoted(&mut commands, robot, RobotPart::Barrel,
                meshes.add(Cylinder::new(BARREL_RADIUS, BARREL_LENGTH)),
                materials.add(StandardMaterial::from_color(COLOR_BARREL)),
                Vec3::new(0.0, HEAD_Y + HEAD_RADIUS * 0.5, 0.0),
                Vec3::new(0.0, 0.0, BARREL_LENGTH / 2.0));
        }
    }
}

fn spawn_part(
    commands: &mut Commands, parent: Entity, part: RobotPart,
    mesh: Handle<Mesh>, material: Handle<StandardMaterial>,
    offset: Vec3, _mesh_offset: Vec3,
) {
    let child = commands.spawn((
        part,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(offset),
    )).id();
    commands.entity(parent).add_child(child);
}

fn spawn_pivoted(
    commands: &mut Commands, parent: Entity, part: RobotPart,
    mesh: Handle<Mesh>, material: Handle<StandardMaterial>,
    pivot_offset: Vec3, mesh_offset: Vec3,
) {
    let pivot = commands.spawn((
        part,
        Transform::from_translation(pivot_offset),
        Visibility::default(),
    )).id();

    let mesh_child = commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(mesh_offset),
    )).id();

    commands.entity(pivot).add_child(mesh_child);
    commands.entity(parent).add_child(pivot);
}

// =====================================================================
// Input
// =====================================================================

fn read_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut move_events: MessageWriter<MoveIntent>,
    mut attack: Query<&mut AttackAnim, With<Robot>>,
    mut debug: ResMut<DebugSettings>,
) {
    let mut dir = Vec2::ZERO;
    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) { dir.y += 1.0; }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) { dir.y -= 1.0; }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) { dir.x -= 1.0; }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) { dir.x += 1.0; }

    if dir != Vec2::ZERO {
        move_events.write(MoveIntent(dir.normalize()));
    }

    if keys.just_pressed(KeyCode::Space) {
        if let Ok(mut atk) = attack.single_mut() {
            atk.active = true;
            atk.timer = ATTACK_DURATION;
        }
    }

    if keys.just_pressed(KeyCode::F1) { debug.show_gizmos = !debug.show_gizmos; }
    if keys.just_pressed(KeyCode::BracketLeft) {
        debug.anim_speed = (debug.anim_speed / 2.0).max(0.0625);
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        debug.anim_speed = (debug.anim_speed * 2.0).min(16.0);
    }
    if keys.just_pressed(KeyCode::Backslash) { debug.anim_speed = 1.0; }
}

fn read_config_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<RobotConfig>,
    mut rebuild: ResMut<NeedsRebuild>,
) {
    let prev = (config.locomotion, config.weapon);

    if keys.just_pressed(KeyCode::Digit1) { config.locomotion = LocoType::Wheels; }
    if keys.just_pressed(KeyCode::Digit2) { config.locomotion = LocoType::Treads; }
    if keys.just_pressed(KeyCode::Digit3) { config.locomotion = LocoType::Legs; }
    if keys.just_pressed(KeyCode::Digit4) { config.locomotion = LocoType::Hover; }
    if keys.just_pressed(KeyCode::Digit5) { config.weapon = WeaponType::Contact; }
    if keys.just_pressed(KeyCode::Digit6) { config.weapon = WeaponType::Melee; }
    if keys.just_pressed(KeyCode::Digit7) { config.weapon = WeaponType::Ranged; }

    if (config.locomotion, config.weapon) != prev {
        rebuild.0 = true;
    }
}

// =====================================================================
// Movement
// =====================================================================

fn apply_movement(
    mut move_events: MessageReader<MoveIntent>,
    mut robot: Query<(&mut Transform, &mut WalkAnim, &mut FacingAngle), With<Robot>>,
    camera_q: Query<&Transform, (With<IsoCamera>, Without<Robot>)>,
    config: Res<RobotConfig>,
    debug: Res<DebugSettings>,
    time: Res<Time>,
) {
    let mut combined = Vec2::ZERO;
    for intent in move_events.read() {
        combined += intent.0;
    }

    let Ok((mut tf, mut walk, mut facing)) = robot.single_mut() else { return };
    let is_moving = combined.length() > 0.01;

    if is_moving {
        let normalized = combined.normalize();

        // Project camera orientation onto the ground plane so screen directions
        // map to world directions correctly for the isometric view.
        let cam_tf = camera_q.single().unwrap();
        let cam_forward = cam_tf.forward().as_vec3();
        let cam_right = cam_tf.right().as_vec3();
        let ground_forward = Vec3::new(cam_forward.x, 0.0, cam_forward.z).normalize();
        let ground_right = Vec3::new(cam_right.x, 0.0, cam_right.z).normalize();

        let world_dir = ground_right * normalized.x + ground_forward * normalized.y;
        let speed = match config.locomotion {
            LocoType::Hover => HOVER_MOVE_SPEED,
            _ => MOVE_SPEED,
        };
        tf.translation += world_dir * speed * time.delta_secs();

        // Face movement direction
        let target_angle = world_dir.x.atan2(world_dir.z);
        facing.0 = target_angle;
        tf.rotation = Quat::from_rotation_y(target_angle);

        walk.phase += WALK_CYCLE_SPEED * time.delta_secs() * debug.anim_speed;
        walk.active = true;
    } else {
        walk.active = false;
    }
}

// =====================================================================
// Animation
// =====================================================================

fn animate_robot(
    config: Res<RobotConfig>,
    debug: Res<DebugSettings>,
    time: Res<Time>,
    robot_q: Query<(&WalkAnim, &AttackAnim), With<Robot>>,
    mut parts: Query<(&RobotPart, &mut Transform), Without<Robot>>,
) {
    let Ok((walk, attack)) = robot_q.single() else { return };
    let phase = walk.phase;
    let active = walk.active;
    let t = time.elapsed_secs() * debug.anim_speed;

    // Attack progress (1.0 = start, 0.0 = done)
    let atk_t = if attack.active { (attack.timer / ATTACK_DURATION).clamp(0.0, 1.0) } else { 0.0 };
    // Attack "punch" curve: quick out, slow back
    let atk_punch = if atk_t > 0.5 { (1.0 - atk_t) * 2.0 } else { atk_t * 2.0 };

    for (part, mut tf) in &mut parts {
        match part {
            // Locomotion animation
            RobotPart::WheelFL | RobotPart::WheelFR | RobotPart::WheelBL | RobotPart::WheelBR => {
                // Base orientation: rotate cylinder so axle points along X (left-right)
                let base = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
                if active {
                    // Spin around the axle (local X after base rotation = world X)
                    tf.rotation = base * Quat::from_rotation_y(phase * WHEEL_SPIN_RATE);
                } else {
                    tf.rotation = base;
                }
            }

            RobotPart::LegLeft => {
                let swing = if active { (phase).sin() * LEG_SWING } else { 0.0 };
                tf.rotation = Quat::from_rotation_x(swing);
            }
            RobotPart::LegRight => {
                let swing = if active { (phase + std::f32::consts::PI).sin() * LEG_SWING } else { 0.0 };
                tf.rotation = Quat::from_rotation_x(swing);
            }

            RobotPart::TreadLeft | RobotPart::TreadRight => {
                // Subtle vibration when moving
                let bob = if active { (phase * 3.0).sin() * TREAD_BOB } else { 0.0 };
                let base_y = TREAD_Y;
                tf.translation.y = base_y + bob;
            }

            RobotPart::HoverFL | RobotPart::HoverFR | RobotPart::HoverBL | RobotPart::HoverBR => {
                let bob = (t * HOVER_BOB_FREQ).sin() * HOVER_BOB_AMP;
                tf.translation.y = HOVER_Y + bob;
            }

            // Arm animation
            RobotPart::LeftArm => {
                let walk_swing = if active { (phase).sin() * ARM_SWING } else { 0.0 };
                let atk_swing = match config.weapon {
                    WeaponType::Melee => -atk_punch * 1.2,
                    WeaponType::Contact => -atk_punch * 0.5,
                    _ => 0.0,
                };
                tf.rotation = Quat::from_rotation_x(walk_swing + atk_swing);
            }
            RobotPart::RightArm => {
                let walk_swing = if active { (phase + std::f32::consts::PI).sin() * ARM_SWING } else { 0.0 };
                let atk_swing = match config.weapon {
                    WeaponType::Melee => -atk_punch * 1.2,
                    WeaponType::Contact => -atk_punch * 0.5,
                    _ => 0.0,
                };
                tf.rotation = Quat::from_rotation_x(walk_swing + atk_swing);
            }

            // Weapon animation
            RobotPart::Ram => {
                let base_z = CHASSIS_D / 2.0 + RAM_D / 2.0;
                tf.translation.z = base_z + atk_punch * 0.3;
            }
            RobotPart::BladeLeft | RobotPart::BladeRight => {
                // Blades are children of chassis, not arms — they swing independently during attack
                let base_y = ARM_Y - ARM_H / 2.0 - BLADE_H / 2.0;
                tf.translation.y = base_y - atk_punch * 0.15;
            }
            RobotPart::Barrel => {
                // Barrel recoils during attack
                let recoil = atk_punch * 0.15;
                tf.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2 + recoil * 0.3);
            }

            // Head bob
            RobotPart::Head | RobotPart::Eye => {
                let bob = if active { (phase * 2.0).sin().abs() * 0.015 } else { 0.0 };
                let base_y = if *part == RobotPart::Head { HEAD_Y } else { HEAD_Y };
                tf.translation.y = base_y + bob;
            }

            RobotPart::Chassis => {
                // Chassis bob
                let bob = if active {
                    match config.locomotion {
                        LocoType::Legs => (phase * 2.0).sin().abs() * 0.03,
                        LocoType::Hover => (t * HOVER_BOB_FREQ).sin() * HOVER_BOB_AMP,
                        _ => (phase * 2.0).sin().abs() * 0.01,
                    }
                } else if config.locomotion == LocoType::Hover {
                    (t * HOVER_BOB_FREQ).sin() * HOVER_BOB_AMP
                } else {
                    0.0
                };
                tf.translation.y = CHASSIS_Y + bob;

                // Contact weapon: chassis lunges forward
                if config.weapon == WeaponType::Contact {
                    tf.translation.z = atk_punch * 0.2;
                }
            }
        }
    }

    // Tick attack timer on the robot itself (need mut)
    // This is handled separately to avoid borrow conflicts
}

// We need a separate system to tick the attack timer since we can't mutably
// borrow Robot and immutably read it in the same query above
fn tick_attack_timer(
    time: Res<Time>,
    debug: Res<DebugSettings>,
    mut robot: Query<&mut AttackAnim, With<Robot>>,
) {
    let Ok(mut atk) = robot.single_mut() else { return };
    if atk.active {
        atk.timer -= time.delta_secs() * debug.anim_speed;
        if atk.timer <= 0.0 {
            atk.active = false;
            atk.timer = 0.0;
        }
    }
}

// =====================================================================
// Camera
// =====================================================================

fn update_camera(
    robot: Query<&Transform, With<Robot>>,
    mut camera: Query<&mut Transform, (With<IsoCamera>, Without<Robot>)>,
) {
    let Ok(robot_tf) = robot.single() else { return };
    let Ok(mut cam_tf) = camera.single_mut() else { return };

    let pitch_rad = ISO_PITCH.to_radians();
    let yaw_rad = ISO_YAW.to_radians();
    let rotation = Quat::from_euler(EulerRot::YXZ, -yaw_rad, -pitch_rad, 0.0);
    let offset = rotation * Vec3::new(0.0, 0.0, ISO_DISTANCE);

    let target = robot_tf.translation;
    cam_tf.translation = target + offset;
    cam_tf.look_at(target, Vec3::Y);
}

fn camera_zoom(
    mut query: Query<&mut Projection, With<IsoCamera>>,
    mut scroll: MessageReader<MouseWheel>,
) {
    for ev in scroll.read() {
        for mut proj in &mut query {
            if let Projection::Orthographic(ortho) = proj.as_mut() {
                let delta = match ev.unit {
                    MouseScrollUnit::Line => -ev.y * ZOOM_SPEED_LINE,
                    MouseScrollUnit::Pixel => -ev.y * ZOOM_SPEED_PIXEL,
                };
                ortho.scale = (ortho.scale * (1.0 + delta)).clamp(ZOOM_MIN, ZOOM_MAX);
            }
        }
    }
}

fn camera_pan(
    mut query: Query<(&mut Transform, &Projection), With<IsoCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
) {
    let Ok((mut tf, proj)) = query.single_mut() else { return };
    let scale = match proj {
        Projection::Orthographic(o) => o.scale,
        _ => 1.0,
    };

    if mouse.pressed(MouseButton::Middle) {
        for ev in motion.read() {
            tf.translation.x -= ev.delta.x * scale;
            tf.translation.y += ev.delta.y * scale;
        }
    } else {
        motion.clear();
    }
}

// =====================================================================
// HUD
// =====================================================================

fn update_hud(
    config: Res<RobotConfig>,
    debug: Res<DebugSettings>,
    mut hud: Query<&mut Text, With<HudLabel>>,
) {
    let Ok(mut text) = hud.single_mut() else { return };
    let loco = match config.locomotion {
        LocoType::Wheels => "Wheels",
        LocoType::Treads => "Treads",
        LocoType::Legs => "Legs",
        LocoType::Hover => "Hover",
    };
    let weapon = match config.weapon {
        WeaponType::Contact => "Contact (ram)",
        WeaponType::Melee => "Melee (blades)",
        WeaponType::Ranged => "Ranged (barrel)",
    };
    **text = format!(
        "Locomotion: {} (1-4)  |  Weapon: {} (5-7)  |  Space: attack  |  Speed: {:.1}x",
        loco, weapon, debug.anim_speed,
    );
}

// =====================================================================
// Debug gizmos
// =====================================================================

fn draw_gizmos(
    debug: Res<DebugSettings>,
    mut gizmos: Gizmos,
    robot: Query<&Transform, With<Robot>>,
) {
    if !debug.show_gizmos { return; }
    let Ok(tf) = robot.single() else { return };
    let pos = tf.translation;

    // Ground contact circle
    gizmos.circle(
        Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        0.5,
        Color::srgb(0.0, 1.0, 0.0),
    );

    // Forward direction
    let forward = tf.rotation * Vec3::new(0.0, 0.0, 1.0);
    gizmos.line(pos + Vec3::Y * 0.5, pos + Vec3::Y * 0.5 + forward * 1.0, Color::srgb(0.0, 0.0, 1.0));
}

// =====================================================================
// Ground plane (spawned after meshes are available)
// =====================================================================
