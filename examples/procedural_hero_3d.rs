//! Procedural hero — 3D version with isometric camera and analog stick support.
//!
//! The hero is built from 3D primitives (spheres, cuboids) and rotates freely
//! to face any direction. No manual facing layouts — the camera projection
//! handles occlusion naturally.
//!
//! Controls:
//!   - Left analog stick / arrow keys: move and face direction
//!   - Scroll wheel: zoom in/out
//!   - Click + drag: pan camera
//!   - Home key: reset zoom and pan
//!   - F1: toggle debug gizmos
//!   - [ / ]: decrease / increase animation speed
//!   - \: reset animation speed
//!
//! Run with: `cargo run --example procedural_hero_3d`

use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// Window
const WINDOW_WIDTH: u32 = 800;
const WINDOW_HEIGHT: u32 = 600;

// Hero body proportions (world units, origin at feet)
//
// Layout (Y axis, bottom to top):
//   0.0          feet (ground)
//   0.0 – 0.85   legs (pivot at hip, hang down to ground)
//   0.85 – 1.35  body (torso)
//   1.35         shoulders (arm pivot, arms hang down to ~0.55)
//   1.35 – 1.95  head (center 1.65, radius 0.3)
//
// ~3-head-tall character, total height ~1.8
//
//   0.00          ground / feet
//   0.00 – 0.70   legs (pivot at 0.70, hang down to ground)
//   0.70 – 1.20   body / torso
//   1.20          shoulders (arm pivot, arms hang to ~0.70)
//   1.20 – 1.80   head (center 1.50, radius 0.30)
//
const BODY_WIDTH: f32 = 0.5;
const BODY_HEIGHT: f32 = 0.5;
const BODY_DEPTH: f32 = 0.3;
const BODY_Y: f32 = 0.95; // center of torso

const HEAD_RADIUS: f32 = 0.3;
const HEAD_Y: f32 = BODY_Y + BODY_HEIGHT / 2.0 + HEAD_RADIUS; // sits on top of body

const LEG_WIDTH: f32 = 0.18;
const LEG_HEIGHT: f32 = BODY_Y - BODY_HEIGHT / 2.0; // reaches from hip to ground (0.70)
const LEG_DEPTH: f32 = 0.18;
const LEG_SPREAD: f32 = 0.12;
const LEG_Y: f32 = BODY_Y - BODY_HEIGHT / 2.0; // bottom of body (hip)

const ARM_WIDTH: f32 = 0.14;
const ARM_HEIGHT: f32 = 0.5; // shoulder to roughly waist level
const ARM_DEPTH: f32 = 0.14;
const ARM_X: f32 = BODY_WIDTH / 2.0 + ARM_WIDTH / 2.0; // flush against body sides
const ARM_Y: f32 = BODY_Y + BODY_HEIGHT / 2.0; // top of body (shoulder)

const EYE_RADIUS: f32 = 0.06;
const EYE_SPREAD: f32 = 0.12;
const EYE_Y_OFFSET: f32 = -0.02;
const EYE_FORWARD: f32 = 0.26;

const HAIR_RADIUS: f32 = 0.31;

// Twin tails — 4-segment chains arcing outward then down
// Rest pose (right side, facing down): NE → E → SE → S
// Each segment adds ~45° (π/4) relative to its parent
const PIGTAIL_SEGMENTS: usize = 4;
const PIGTAIL_WIDTH: f32 = 0.08;
const PIGTAIL_SEGMENT_HEIGHT: f32 = 0.12;
const PIGTAIL_DEPTH: f32 = 0.08;
const PIGTAIL_X: f32 = HAIR_RADIUS - PIGTAIL_WIDTH / 2.0; // block edge meets hair surface
const PIGTAIL_Y: f32 = HEAD_Y + HEAD_RADIUS - PIGTAIL_SEGMENT_HEIGHT / 2.0; // bottom of first block aligns with hair edge
const PIGTAIL_SWING_BASE: f32 = 0.1; // walk animation swing per segment
const PIGTAIL_REST_ANGLE: f32 = std::f32::consts::FRAC_PI_4; // 45° per segment

// Hero palette
const COLOR_SKIN: Color = Color::srgb(0.93, 0.76, 0.57);
const COLOR_TUNIC: Color = Color::srgb(0.18, 0.55, 0.18);
const COLOR_PANTS: Color = Color::srgb(0.55, 0.40, 0.25);
const COLOR_HAIR: Color = Color::srgb(0.8, 0.2, 0.05);
const COLOR_EYE: Color = Color::srgb(0.15, 0.4, 0.9);

// Movement
const MOVE_SPEED: f32 = 5.0;
const STICK_DEADZONE: f32 = 0.2;

// Walk animation
const WALK_CYCLE_SPEED: f32 = 10.0;
const LEG_SWING_AMPLITUDE: f32 = 0.4; // radians
const ARM_SWING_AMPLITUDE: f32 = 0.3;
const HEAD_BOB_AMPLITUDE: f32 = 0.03;
const BODY_BOB_AMPLITUDE: f32 = 0.02;

// Isometric camera
const ISO_DISTANCE: f32 = 15.0;
const ISO_PITCH: f32 = 35.264; // arctan(1/sqrt(2)) in degrees — true isometric
const ISO_YAW: f32 = 45.0;

// Zoom
const ZOOM_SPEED_LINE: f32 = 0.15;
const ZOOM_SPEED_PIXEL: f32 = 0.002;
const ZOOM_MIN: f32 = 0.002;
const ZOOM_MAX: f32 = 0.5;
const DEFAULT_ZOOM_SCALE: f32 = 0.01;

// HUD
const HUD_FONT_SIZE: f32 = 20.0;
const HUD_MARGIN: f32 = 12.0;

// Animation speed
const ANIMATION_SPEED_STEP: f32 = 2.0;
const ANIMATION_SPEED_MIN: f32 = 0.0625;
const ANIMATION_SPEED_MAX: f32 = 16.0;

// ---------------------------------------------------------------------------
// Components & Messages
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Hero;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BodyPart {
    LeftLeg,
    RightLeg,
    Body,
    LeftArm,
    RightArm,
    Head,
    Hair,
    LeftEye,
    RightEye,
}

/// A segment of a pigtail/twin tail chain.
/// `side`: -1.0 for left, 1.0 for right.
/// `depth`: 0 = root (attached to head), 1 = middle, 2 = tip.
#[derive(Component)]
struct PigtailSegment {
    side: f32,
    depth: usize,
}

#[derive(Component, Default)]
struct WalkAnimation {
    phase: f32,
    active: bool,
}

/// The angle the hero is facing, in radians. 0 = forward (+Z in hero local space).
#[derive(Component, Default)]
struct FacingAngle(f32);

#[derive(Component)]
struct HudLabel;

#[derive(Component)]
struct IsoCamera;

/// Input protocol: the direction the player wants to move this frame (world XZ plane).
#[derive(Message)]
struct MoveIntent(Vec2);

#[derive(Resource)]
struct DebugSettings {
    show_gizmos: bool,
    animation_speed_factor: f32,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            show_gizmos: false,
            animation_speed_factor: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// XInput FFI
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    _thumb_rx: i16,
    _thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    _packet_number: u32,
    gamepad: XInputGamepad,
}

const XINPUT_GAMEPAD_DPAD_UP: u16 = 0x0001;
const XINPUT_GAMEPAD_DPAD_DOWN: u16 = 0x0002;
const XINPUT_GAMEPAD_DPAD_LEFT: u16 = 0x0004;
const XINPUT_GAMEPAD_DPAD_RIGHT: u16 = 0x0008;

const XINPUT_SUCCESS: u32 = 0;
const GAMEPAD_INDEX: u32 = 0;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

fn load_xinput() -> Option<XInputGetStateFn> {
    use std::ffi::CString;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut std::ffi::c_void;
        fn GetProcAddress(
            module: *mut std::ffi::c_void,
            name: *const u8,
        ) -> *mut std::ffi::c_void;
    }

    for dll in &[b"xinput1_4.dll\0" as &[u8], b"xinput9_1_0.dll\0"] {
        let module = unsafe { LoadLibraryA(dll.as_ptr()) };
        if module.is_null() {
            continue;
        }
        let proc_name = CString::new("XInputGetState").unwrap();
        let proc = unsafe { GetProcAddress(module, proc_name.as_ptr() as *const u8) };
        if !proc.is_null() {
            return Some(unsafe { std::mem::transmute(proc) });
        }
    }
    None
}

fn normalize_thumb(value: i16) -> f32 {
    if value >= 0 {
        value as f32 / 32767.0
    } else {
        value as f32 / 32768.0
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Procedural Hero — 3D Isometric".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_message::<MoveIntent>()
        .init_resource::<DebugSettings>()
        .add_systems(Startup, (setup_camera, setup_hero, setup_ground, setup_hud))
        .add_systems(
            Update,
            (
                read_keyboard_movement,
                read_gamepad_movement,
                read_zoom_input,
                read_pan_input,
                read_reset_input,
                read_debug_input,
                apply_movement,
                animate_walk_cycle.after(apply_movement),
                draw_debug_gizmos,
                update_hud_label,
            ),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    let pitch_rad = ISO_PITCH.to_radians();
    let yaw_rad = ISO_YAW.to_radians();

    // Position camera at isometric angle looking at origin
    let direction = Vec3::new(
        yaw_rad.cos() * pitch_rad.cos(),
        pitch_rad.sin(),
        yaw_rad.sin() * pitch_rad.cos(),
    );
    let position = direction * ISO_DISTANCE;

    commands.spawn((
        IsoCamera,
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scale: DEFAULT_ZOOM_SCALE,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn setup_ground(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Ground plane
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(10.0)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.3, 0.5, 0.3),
            ..default()
        })),
    ));

    // Ambient light
    commands.insert_resource(GlobalAmbientLight {
        color: Color::WHITE,
        brightness: 500.0,
        affects_lightmapped_meshes: true,
    });

    // Directional light (sun-like, matching isometric angle)
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -45.0_f32.to_radians(),
            30.0_f32.to_radians(),
            0.0,
        )),
    ));
}

fn setup_hero(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let hero = commands
        .spawn((
            Hero,
            FacingAngle::default(),
            WalkAnimation::default(),
            Transform::from_translation(Vec3::ZERO),
            Visibility::default(),
        ))
        .id();

    let parts = [
        BodyPart::LeftLeg,
        BodyPart::RightLeg,
        BodyPart::Body,
        BodyPart::LeftArm,
        BodyPart::RightArm,
        BodyPart::Head,
        BodyPart::Hair,
        BodyPart::LeftEye,
        BodyPart::RightEye,
    ];

    for part in parts {
        spawn_part(&mut commands, hero, part, &mut meshes, &mut materials);
    }

    // Twin tails — two chains of segments, one on each side
    for &side in &[-1.0_f32, 1.0] {
        spawn_pigtail(&mut commands, hero, side, &mut meshes, &mut materials);
    }
}

fn spawn_part(
    commands: &mut Commands,
    hero: Entity,
    part: BodyPart,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let (mesh, material, pivot_offset, mesh_offset) = mesh_for_part(part, meshes, materials);

    // Pivot entity sits at the joint (shoulder/hip). The mesh is a child
    // offset downward so rotation swings from the top.
    let pivot = commands
        .spawn((
            part,
            Transform::from_translation(pivot_offset),
            Visibility::default(),
        ))
        .id();

    let mesh_child = commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(mesh_offset),
        ))
        .id();

    commands.entity(pivot).add_child(mesh_child);
    commands.entity(hero).add_child(pivot);
}

fn spawn_pigtail(
    commands: &mut Commands,
    hero: Entity,
    side: f32,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(PIGTAIL_WIDTH, PIGTAIL_SEGMENT_HEIGHT, PIGTAIL_DEPTH));
    let material = materials.add(StandardMaterial::from_color(COLOR_HAIR));

    // First segment's pivot is relative to the hero root, at the side of the head
    let mut parent = hero;
    for depth in 0..PIGTAIL_SEGMENTS {
        let pivot_offset = if depth == 0 {
            // Root segment: attached high on the side of the head
            Vec3::new(side * PIGTAIL_X, PIGTAIL_Y, 0.0)
        } else {
            // Subsequent segments: connect at the end of the previous
            // Segments extend upward in local space; rotation arcs them over
            Vec3::new(0.0, PIGTAIL_SEGMENT_HEIGHT, 0.0)
        };

        let pivot = commands
            .spawn((
                PigtailSegment { side, depth },
                Transform::from_translation(pivot_offset),
                Visibility::default(),
            ))
            .id();

        let mesh_child = commands
            .spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(material.clone()),
                // Mesh extends upward from pivot
                Transform::from_translation(Vec3::new(0.0, PIGTAIL_SEGMENT_HEIGHT / 2.0, 0.0)),
            ))
            .id();

        commands.entity(pivot).add_child(mesh_child);
        commands.entity(parent).add_child(pivot);
        parent = pivot;
    }
}

/// Returns (mesh, material, pivot_offset, mesh_offset).
/// pivot_offset: where the pivot entity sits relative to the hero root.
/// mesh_offset: where the mesh sits relative to the pivot (downward for limbs).
fn mesh_for_part(
    part: BodyPart,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> (Handle<Mesh>, Handle<StandardMaterial>, Vec3, Vec3) {
    match part {
        // Legs pivot at the hip — mesh hangs down
        BodyPart::LeftLeg => (
            meshes.add(Cuboid::new(LEG_WIDTH, LEG_HEIGHT, LEG_DEPTH)),
            materials.add(StandardMaterial::from_color(COLOR_PANTS)),
            Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
            Vec3::new(0.0, -LEG_HEIGHT / 2.0, 0.0),
        ),
        BodyPart::RightLeg => (
            meshes.add(Cuboid::new(LEG_WIDTH, LEG_HEIGHT, LEG_DEPTH)),
            materials.add(StandardMaterial::from_color(COLOR_PANTS)),
            Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
            Vec3::new(0.0, -LEG_HEIGHT / 2.0, 0.0),
        ),
        // Body — no pivot needed, centered
        BodyPart::Body => (
            meshes.add(Cuboid::new(BODY_WIDTH, BODY_HEIGHT, BODY_DEPTH)),
            materials.add(StandardMaterial::from_color(COLOR_TUNIC)),
            Vec3::new(0.0, BODY_Y, 0.0),
            Vec3::ZERO,
        ),
        // Arms pivot at the shoulder — mesh hangs down
        BodyPart::LeftArm => (
            meshes.add(Cuboid::new(ARM_WIDTH, ARM_HEIGHT, ARM_DEPTH)),
            materials.add(StandardMaterial::from_color(COLOR_SKIN)),
            Vec3::new(-ARM_X, ARM_Y, 0.0),
            Vec3::new(0.0, -ARM_HEIGHT / 2.0, 0.0),
        ),
        BodyPart::RightArm => (
            meshes.add(Cuboid::new(ARM_WIDTH, ARM_HEIGHT, ARM_DEPTH)),
            materials.add(StandardMaterial::from_color(COLOR_SKIN)),
            Vec3::new(ARM_X, ARM_Y, 0.0),
            Vec3::new(0.0, -ARM_HEIGHT / 2.0, 0.0),
        ),
        // Head, hair, eyes — no pivot offset needed
        BodyPart::Head => (
            meshes.add(Sphere::new(HEAD_RADIUS)),
            materials.add(StandardMaterial::from_color(COLOR_SKIN)),
            Vec3::new(0.0, HEAD_Y, 0.0),
            Vec3::ZERO,
        ),
        BodyPart::Hair => (
            meshes.add(Sphere::new(HAIR_RADIUS)),
            materials.add(StandardMaterial::from_color(COLOR_HAIR)),
            Vec3::new(0.0, HEAD_Y + 0.05, -0.05),
            Vec3::ZERO,
        ),
        BodyPart::LeftEye => (
            meshes.add(Sphere::new(EYE_RADIUS)),
            materials.add(StandardMaterial::from_color(COLOR_EYE)),
            Vec3::new(-EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, EYE_FORWARD),
            Vec3::ZERO,
        ),
        BodyPart::RightEye => (
            meshes.add(Sphere::new(EYE_RADIUS)),
            materials.add(StandardMaterial::from_color(COLOR_EYE)),
            Vec3::new(EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, EYE_FORWARD),
            Vec3::ZERO,
        ),
    }
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        HudLabel,
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
// Input — keyboard
// ---------------------------------------------------------------------------

fn read_keyboard_movement(
    keys: Res<ButtonInput<KeyCode>>,
    mut move_events: MessageWriter<MoveIntent>,
) {
    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::ArrowUp) || keys.pressed(KeyCode::KeyW) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) || keys.pressed(KeyCode::KeyS) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::KeyA) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::KeyD) {
        direction.x += 1.0;
    }

    if direction != Vec2::ZERO {
        move_events.write(MoveIntent(direction.normalize()));
    }
}

// ---------------------------------------------------------------------------
// Input — gamepad (XInput analog stick + d-pad)
// ---------------------------------------------------------------------------

fn read_gamepad_movement(
    mut move_events: MessageWriter<MoveIntent>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    let get_state = match *xinput_fn {
        Some(Some(f)) => f,
        Some(None) => return,
        None => {
            let loaded = load_xinput();
            if loaded.is_none() {
                warn!("Failed to load XInput DLL — gamepad input unavailable");
            }
            *xinput_fn = Some(loaded);
            match loaded {
                Some(f) => f,
                None => return,
            }
        }
    };

    let mut xinput_state = std::mem::MaybeUninit::<XInputState>::uninit();
    let result = unsafe { get_state(GAMEPAD_INDEX, xinput_state.as_mut_ptr()) };

    if result != XINPUT_SUCCESS {
        return;
    }

    let gp = unsafe { &xinput_state.assume_init().gamepad };

    // Analog stick
    let stick = Vec2::new(
        normalize_thumb(gp.thumb_lx),
        normalize_thumb(gp.thumb_ly),
    );

    if stick.length() > STICK_DEADZONE {
        move_events.write(MoveIntent(stick.normalize()));
        return;
    }

    // Fall back to d-pad
    let btn = |mask: u16| gp.buttons & mask != 0;
    let mut direction = Vec2::ZERO;

    if btn(XINPUT_GAMEPAD_DPAD_UP) {
        direction.y += 1.0;
    }
    if btn(XINPUT_GAMEPAD_DPAD_DOWN) {
        direction.y -= 1.0;
    }
    if btn(XINPUT_GAMEPAD_DPAD_LEFT) {
        direction.x -= 1.0;
    }
    if btn(XINPUT_GAMEPAD_DPAD_RIGHT) {
        direction.x += 1.0;
    }

    if direction != Vec2::ZERO {
        move_events.write(MoveIntent(direction.normalize()));
    }
}

// ---------------------------------------------------------------------------
// Input — camera controls
// ---------------------------------------------------------------------------

fn read_zoom_input(
    mut scroll_events: MessageReader<MouseWheel>,
    mut projection_query: Query<&mut Projection, With<IsoCamera>>,
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
    mut camera_query: Query<(&mut Transform, &Projection), With<IsoCamera>>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        motion_events.read();
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

    // Pan in the camera's local right/up plane
    let right = transform.right();
    let up = transform.up();
    let pan = (-drag_delta.x * right + drag_delta.y * up) * ortho.scale * 0.01;
    transform.translation += pan;
}

fn read_reset_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<IsoCamera>>,
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

    let pitch_rad = ISO_PITCH.to_radians();
    let yaw_rad = ISO_YAW.to_radians();
    let direction = Vec3::new(
        yaw_rad.cos() * pitch_rad.cos(),
        pitch_rad.sin(),
        yaw_rad.sin() * pitch_rad.cos(),
    );
    *transform = Transform::from_translation(direction * ISO_DISTANCE)
        .looking_at(Vec3::ZERO, Vec3::Y);
    ortho.scale = DEFAULT_ZOOM_SCALE;
}

fn read_debug_input(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<DebugSettings>) {
    if keys.just_pressed(KeyCode::F1) {
        debug.show_gizmos = !debug.show_gizmos;
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        let new_speed = debug.animation_speed_factor * ANIMATION_SPEED_STEP;
        debug.animation_speed_factor = new_speed.min(ANIMATION_SPEED_MAX);
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        let new_speed = debug.animation_speed_factor / ANIMATION_SPEED_STEP;
        debug.animation_speed_factor = new_speed.max(ANIMATION_SPEED_MIN);
    }
    if keys.just_pressed(KeyCode::Backslash) {
        debug.animation_speed_factor = 1.0;
    }
}

// ---------------------------------------------------------------------------
// Simulation — updates hero position and facing
// ---------------------------------------------------------------------------

fn apply_movement(
    mut move_events: MessageReader<MoveIntent>,
    mut hero_query: Query<
        (&mut Transform, &mut FacingAngle, &mut WalkAnimation),
        With<Hero>,
    >,
    camera_query: Query<&Transform, (With<IsoCamera>, Without<Hero>)>,
    debug: Res<DebugSettings>,
    time: Res<Time>,
) {
    let mut combined_direction = Vec2::ZERO;
    for event in move_events.read() {
        combined_direction += event.0;
    }

    let Ok((mut transform, mut facing, mut walk)) = hero_query.single_mut() else {
        return;
    };

    let is_moving = combined_direction.length() > 0.01;

    if is_moving {
        let normalized = combined_direction.normalize();

        // Project camera orientation onto the ground plane so screen directions
        // map to world directions correctly for the isometric view.
        let cam_transform = camera_query.single().unwrap();
        let cam_forward = cam_transform.forward().as_vec3();
        let cam_right = cam_transform.right().as_vec3();

        // Flatten onto XZ plane and normalize
        let ground_forward = Vec3::new(cam_forward.x, 0.0, cam_forward.z).normalize();
        let ground_right = Vec3::new(cam_right.x, 0.0, cam_right.z).normalize();

        // Screen input: x = right, y = up (which maps to camera forward on ground)
        let world_dir = ground_right * normalized.x + ground_forward * normalized.y;
        let world_movement = world_dir * MOVE_SPEED * time.delta_secs();
        transform.translation += world_movement;

        // Face the movement direction — atan2 on world XZ
        let target_angle = world_dir.x.atan2(world_dir.z);
        facing.0 = target_angle;

        // Rotate the hero entity around Y axis
        transform.rotation = Quat::from_rotation_y(target_angle);

        let effective_speed = WALK_CYCLE_SPEED * debug.animation_speed_factor;
        walk.phase += effective_speed * time.delta_secs();
        walk.active = true;
    } else {
        walk.phase = 0.0;
        walk.active = false;
    }
}

// ---------------------------------------------------------------------------
// Presentation — walk animation on body parts
// ---------------------------------------------------------------------------

fn animate_walk_cycle(
    hero_query: Query<&WalkAnimation, With<Hero>>,
    mut part_query: Query<(&BodyPart, &mut Transform), (Without<Hero>, Without<PigtailSegment>)>,
    mut pigtail_query: Query<(&PigtailSegment, &mut Transform), Without<BodyPart>>,
) {
    let Ok(walk) = hero_query.single() else {
        return;
    };

    for (part, mut transform) in &mut part_query {
        let base_offset = base_offset(*part);

        if walk.active {
            let anim = walk_offsets(*part, walk.phase);
            transform.translation = base_offset + anim.position;
            transform.rotation = anim.rotation;
        } else {
            transform.translation = base_offset;
            transform.rotation = Quat::IDENTITY;
        }
    }

    // Twin tail animation — each segment tilts 45° outward (rest pose),
    // walk adds cascading swing on top.
    for (segment, mut transform) in &mut pigtail_query {
        let rest_rot = Quat::from_rotation_z(-PIGTAIL_REST_ANGLE * segment.side);

        if walk.active {
            let depth_factor = (segment.depth + 1) as f32;
            let phase_delay = segment.depth as f32 * 0.4;
            let swing = (walk.phase + phase_delay).sin() * PIGTAIL_SWING_BASE * depth_factor;
            let walk_rot = Quat::from_rotation_z(swing * segment.side);
            transform.rotation = rest_rot * walk_rot;
        } else {
            transform.rotation = rest_rot;
        }
    }
}

fn base_offset(part: BodyPart) -> Vec3 {
    match part {
        BodyPart::LeftLeg => Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
        BodyPart::RightLeg => Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
        BodyPart::Body => Vec3::new(0.0, BODY_Y, 0.0),
        BodyPart::LeftArm => Vec3::new(-ARM_X, ARM_Y, 0.0),
        BodyPart::RightArm => Vec3::new(ARM_X, ARM_Y, 0.0),
        BodyPart::Head => Vec3::new(0.0, HEAD_Y, 0.0),
        BodyPart::Hair => Vec3::new(0.0, HEAD_Y + 0.05, -0.05),
        BodyPart::LeftEye => Vec3::new(-EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, EYE_FORWARD),
        BodyPart::RightEye => Vec3::new(EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, EYE_FORWARD),
    }
}

struct WalkOffsets {
    position: Vec3,
    rotation: Quat,
}

fn walk_offsets(part: BodyPart, phase: f32) -> WalkOffsets {
    match part {
        // Legs swing forward/back around the hip (X axis rotation)
        BodyPart::LeftLeg => {
            let swing = phase.sin() * LEG_SWING_AMPLITUDE;
            WalkOffsets {
                position: Vec3::ZERO,
                rotation: Quat::from_rotation_x(swing),
            }
        }
        BodyPart::RightLeg => {
            let swing = (phase + std::f32::consts::PI).sin() * LEG_SWING_AMPLITUDE;
            WalkOffsets {
                position: Vec3::ZERO,
                rotation: Quat::from_rotation_x(swing),
            }
        }

        // Arms swing opposite to legs
        BodyPart::LeftArm => {
            let swing = (phase + std::f32::consts::PI).sin() * ARM_SWING_AMPLITUDE;
            WalkOffsets {
                position: Vec3::ZERO,
                rotation: Quat::from_rotation_x(swing),
            }
        }
        BodyPart::RightArm => {
            let swing = phase.sin() * ARM_SWING_AMPLITUDE;
            WalkOffsets {
                position: Vec3::ZERO,
                rotation: Quat::from_rotation_x(swing),
            }
        }

        // Head and eyes bob
        BodyPart::Head | BodyPart::Hair | BodyPart::LeftEye | BodyPart::RightEye => {
            let bob = phase.sin().abs() * HEAD_BOB_AMPLITUDE;
            WalkOffsets {
                position: Vec3::new(0.0, bob, 0.0),
                rotation: Quat::IDENTITY,
            }
        }

        // Body bobs subtly
        BodyPart::Body => {
            let bob = phase.sin().abs() * BODY_BOB_AMPLITUDE;
            WalkOffsets {
                position: Vec3::new(0.0, bob, 0.0),
                rotation: Quat::IDENTITY,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Debug gizmos
// ---------------------------------------------------------------------------

fn draw_debug_gizmos(
    debug: Res<DebugSettings>,
    mut gizmos: Gizmos,
    hero_query: Query<&Transform, With<Hero>>,
) {
    if !debug.show_gizmos {
        return;
    }

    let Ok(hero_transform) = hero_query.single() else {
        return;
    };

    let pos = hero_transform.translation;

    // Draw axes at hero position
    let forward = hero_transform.forward();
    let right = hero_transform.right();
    gizmos.line(pos, pos + forward * 1.5, Color::srgb(0.0, 0.0, 1.0));
    gizmos.line(pos, pos + right * 1.0, Color::srgb(1.0, 0.0, 0.0));
    gizmos.line(pos, pos + Vec3::Y * 2.0, Color::srgb(0.0, 1.0, 0.0));

    // Ground circle around hero
    gizmos.circle(
        Isometry3d::new(pos, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        0.8,
        Color::srgb(1.0, 1.0, 0.0),
    );
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

fn update_hud_label(
    projection_query: Query<&Projection, With<IsoCamera>>,
    hero_query: Query<(&FacingAngle, &WalkAnimation), With<Hero>>,
    debug: Res<DebugSettings>,
    mut label_query: Query<&mut Text, With<HudLabel>>,
) {
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let Ok((facing, walk)) = hero_query.single() else {
        return;
    };
    let Ok(mut text) = label_query.single_mut() else {
        return;
    };

    let zoom_percentage = 1.0 / ortho.scale;
    let state_label = if walk.active { "Walking" } else { "Idle" };
    let angle_degrees = facing.0.to_degrees();
    let speed_label = format!("{:.2}x", debug.animation_speed_factor);

    *text = Text::new(format!(
        "Zoom: {:.0}%  Angle: {:.0}°  State: {}  Anim: {}  |  F1: gizmos  [/]: speed  \\: reset",
        zoom_percentage * 100.0,
        angle_degrees,
        state_label,
        speed_label,
    ));
}
