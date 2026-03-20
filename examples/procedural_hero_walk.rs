//! Procedural hero — Example 3: Walk Cycle
//!
//! Arrow keys move the hero. Walking triggers a procedural animation:
//! legs change height, fists bob, head bobs. No rotation — all animation
//! comes from reshaping and repositioning parts.
//!
//! Controls:
//!   - Arrow keys / gamepad d-pad: move and face direction
//!   - Scroll wheel: zoom in/out
//!   - Click + drag: pan camera
//!   - Home key: reset zoom and pan
//!   - F1: toggle debug bounding boxes
//!   - [ / ]: decrease / increase animation speed
//!   - \: reset animation speed
//!
//! Run with: `cargo run --example procedural_hero_walk`

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

const HAIR_RADIUS: f32 = 10.0;
const HAIR_HALF_ANGLE: f32 = 1.3; // slightly less than PI/2 (1.57) for just under half

const LEG_WIDTH: f32 = 7.0;
const LEG_HEIGHT: f32 = 10.0;
const LEG_SPREAD: f32 = 5.0;
const LEG_Y: f32 = 3.0;

const FIST_RADIUS: f32 = 4.0;
const FIST_X: f32 = 15.0;
const FIST_Y: f32 = 16.0;
const FIST_SIDE_X: f32 = 0.0;

const EYE_RADIUS: f32 = 1.5;
const EYE_SPREAD: f32 = 4.0;
const EYE_Y_OFFSET: f32 = -2.0;
const EYE_SIDE_X: f32 = -4.0;

// Hero palette
const COLOR_SKIN: Color = Color::srgb(0.93, 0.76, 0.57);
const COLOR_TUNIC: Color = Color::srgb(0.18, 0.55, 0.18);
const COLOR_PANTS: Color = Color::srgb(0.55, 0.40, 0.25);
const COLOR_HAIR: Color = Color::srgb(0.75, 0.60, 0.20);
const COLOR_EYE: Color = Color::BLACK;

// Movement
const MOVE_SPEED: f32 = 400.0;

// Hero bounding box margin from screen edge (pixels)
const HERO_SCREEN_MARGIN: f32 = 0.0;

// How far the hero's visual extends from the origin (feet)
const HERO_EXTENT_TOP: f32 = HEAD_Y + HEAD_RADIUS;
const HERO_EXTENT_LEFT: f32 = FIST_X + FIST_RADIUS;
const HERO_EXTENT_RIGHT: f32 = FIST_X + FIST_RADIUS;
const HERO_EXTENT_BOTTOM: f32 = LEG_HEIGHT / 2.0 - LEG_Y;

// Walk animation
const WALK_CYCLE_SPEED: f32 = 10.0;
const LEG_HEIGHT_AMPLITUDE: f32 = 3.0;
const LEG_CIRCLE_RADIUS_X: f32 = 3.0;
const LEG_CIRCLE_RADIUS_Y: f32 = 2.0;
const FIST_BOB_AMPLITUDE: f32 = 3.0;
const HEAD_BOB_AMPLITUDE: f32 = 1.5;
const BODY_BOB_AMPLITUDE: f32 = 1.0;

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
// Components & Messages
// ---------------------------------------------------------------------------

#[derive(Component, Clone, Copy, PartialEq, Eq, Default)]
enum Facing {
    #[default]
    Down,
    Up,
    Left,
    Right,
}

impl Facing {
    fn label(self) -> &'static str {
        match self {
            Facing::Down => "Down",
            Facing::Up => "Up",
            Facing::Left => "Left",
            Facing::Right => "Right",
        }
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum BodyPart {
    LeftLeg,
    RightLeg,
    Body,
    LeftFist,
    RightFist,
    Head,
    Hair,
    LeftEye,
    RightEye,
}

#[derive(Component, Default)]
struct WalkAnimation {
    phase: f32,
    active: bool,
}

#[derive(Component)]
struct Hero;

#[derive(Component)]
struct HudLabel;

/// Input protocol: the direction the player wants to move this frame.
#[derive(Message)]
struct MoveIntent(Vec2);

#[derive(Resource)]
struct DebugSettings {
    show_bounds: bool,
    animation_speed_factor: f32,
}

impl Default for DebugSettings {
    fn default() -> Self {
        Self {
            show_bounds: true,
            animation_speed_factor: 1.0,
        }
    }
}

const ANIMATION_SPEED_STEP: f32 = 2.0;
const ANIMATION_SPEED_MIN: f32 = 0.0625;
const ANIMATION_SPEED_MAX: f32 = 16.0;

// ---------------------------------------------------------------------------
// XInput FFI — gamepad access bypassing Bevy's gilrs
// (see docs/research/gilrs-dual-gamepad-bug.md)
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    _left_trigger: u8,
    _right_trigger: u8,
    _thumb_lx: i16,
    _thumb_ly: i16,
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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Procedural Hero — Walk".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_message::<MoveIntent>()
        .init_resource::<DebugSettings>()
        .add_systems(Startup, (setup_camera, setup_hero, setup_hud))
        .add_systems(
            Update,
            (
                // Input
                read_keyboard_movement,
                read_gamepad_movement,
                read_zoom_input,
                read_pan_input,
                read_reset_input,
                read_debug_input,
                // Simulation
                apply_movement,
                // Presentation (animation must run after arrangement)
                arrange_body_parts,
                animate_walk_cycle.after(arrange_body_parts),
                draw_debug_bounds,
                update_hud_label,
            ),
        )
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
    let hero = commands
        .spawn((
            Hero,
            Facing::default(),
            WalkAnimation::default(),
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    let parts = [
        BodyPart::LeftLeg,
        BodyPart::RightLeg,
        BodyPart::Body,
        BodyPart::LeftFist,
        BodyPart::RightFist,
        BodyPart::Head,
        BodyPart::Hair,
        BodyPart::LeftEye,
        BodyPart::RightEye,
    ];

    for part in parts {
        spawn_part(&mut commands, hero, part, &mut meshes, &mut materials);
    }
}

fn spawn_part(
    commands: &mut Commands,
    hero: Entity,
    part: BodyPart,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) {
    let (mesh, material) = mesh_for_part(part, meshes, materials);
    let layout = layout_for_part(part, Facing::default());
    let initial_visibility = if layout.visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let child = commands
        .spawn((
            part,
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform {
                translation: layout.offset,
                rotation: Quat::from_rotation_z(layout.rotation),
                ..default()
            },
            initial_visibility,
        ))
        .id();

    commands.entity(hero).add_child(child);
}

fn mesh_for_part(
    part: BodyPart,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
) -> (Handle<Mesh>, Handle<ColorMaterial>) {
    match part {
        BodyPart::LeftLeg | BodyPart::RightLeg => (
            meshes.add(Rectangle::new(LEG_WIDTH, LEG_HEIGHT)),
            materials.add(ColorMaterial::from_color(COLOR_PANTS)),
        ),
        BodyPart::Body => (
            meshes.add(Rectangle::new(BODY_WIDTH, BODY_HEIGHT)),
            materials.add(ColorMaterial::from_color(COLOR_TUNIC)),
        ),
        BodyPart::LeftFist | BodyPart::RightFist => (
            meshes.add(Circle::new(FIST_RADIUS)),
            materials.add(ColorMaterial::from_color(COLOR_SKIN)),
        ),
        BodyPart::Head => (
            meshes.add(Circle::new(HEAD_RADIUS)),
            materials.add(ColorMaterial::from_color(COLOR_SKIN)),
        ),
        BodyPart::Hair => {
            let segment = CircularSegment::new(HAIR_RADIUS, HAIR_HALF_ANGLE);
            (
                meshes.add(segment),
                materials.add(ColorMaterial::from_color(COLOR_HAIR)),
            )
        }
        BodyPart::LeftEye | BodyPart::RightEye => (
            meshes.add(Circle::new(EYE_RADIUS)),
            materials.add(ColorMaterial::from_color(COLOR_EYE)),
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
// Input layer — translates raw input into MoveIntent messages
// ---------------------------------------------------------------------------

fn read_keyboard_movement(
    keys: Res<ButtonInput<KeyCode>>,
    mut move_events: MessageWriter<MoveIntent>,
) {
    let mut direction = Vec2::ZERO;

    if keys.pressed(KeyCode::ArrowUp) {
        direction.y += 1.0;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        direction.y -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    move_events.write(MoveIntent(direction));
}

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
        move_events.write(MoveIntent(direction));
    }
}

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

fn read_debug_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut debug: ResMut<DebugSettings>,
) {
    if keys.just_pressed(KeyCode::F1) {
        debug.show_bounds = !debug.show_bounds;
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
// Simulation layer — updates hero position, facing, and walk state
// ---------------------------------------------------------------------------

fn apply_movement(
    mut move_events: MessageReader<MoveIntent>,
    mut hero_query: Query<(&mut Transform, &mut Facing, &mut WalkAnimation), With<Hero>>,
    camera_query: Query<(&Transform, &Projection), (With<Camera2d>, Without<Hero>)>,
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

    let is_moving = combined_direction != Vec2::ZERO;

    if is_moving {
        let normalized = combined_direction.normalize();
        let movement = normalized * MOVE_SPEED * time.delta_secs();
        transform.translation.x += movement.x;
        transform.translation.y += movement.y;

        // Clamp to visible screen bounds, accounting for hero extents
        if let Ok((cam_transform, projection)) = camera_query.single() {
            if let Projection::Orthographic(ref ortho) = *projection {
                let cam_x = cam_transform.translation.x;
                let cam_y = cam_transform.translation.y;
                let min_x = cam_x + ortho.area.min.x + HERO_SCREEN_MARGIN + HERO_EXTENT_LEFT;
                let max_x = cam_x + ortho.area.max.x - HERO_SCREEN_MARGIN - HERO_EXTENT_RIGHT;
                let min_y = cam_y + ortho.area.min.y + HERO_SCREEN_MARGIN + HERO_EXTENT_BOTTOM;
                let max_y = cam_y + ortho.area.max.y - HERO_SCREEN_MARGIN - HERO_EXTENT_TOP;
                transform.translation.x = transform.translation.x.clamp(min_x, max_x);
                transform.translation.y = transform.translation.y.clamp(min_y, max_y);
            }
        }

        let new_facing = facing_from_direction(normalized);
        if *facing != new_facing {
            *facing = new_facing;
        }

        let effective_speed = WALK_CYCLE_SPEED * debug.animation_speed_factor;
        walk.phase += effective_speed * time.delta_secs();
        walk.active = true;
    } else {
        walk.phase = 0.0;
        walk.active = false;
    }
}

fn facing_from_direction(direction: Vec2) -> Facing {
    if direction.x.abs() > direction.y.abs() {
        if direction.x > 0.0 {
            Facing::Right
        } else {
            Facing::Left
        }
    } else if direction.y > 0.0 {
        Facing::Up
    } else {
        Facing::Down
    }
}

// ---------------------------------------------------------------------------
// Presentation layer — arranges and animates body parts
// ---------------------------------------------------------------------------

fn arrange_body_parts(
    hero_query: Query<&Facing, (With<Hero>, Changed<Facing>)>,
    mut part_query: Query<(&BodyPart, &mut Transform, &mut Visibility)>,
) {
    let Ok(facing) = hero_query.single() else {
        return;
    };

    for (part, mut transform, mut visibility) in &mut part_query {
        let layout = layout_for_part(*part, *facing);
        transform.translation = layout.offset;
        transform.rotation = Quat::from_rotation_z(layout.rotation);
        *visibility = if layout.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn animate_walk_cycle(
    hero_query: Query<(&Facing, &WalkAnimation), With<Hero>>,
    mut part_query: Query<(&BodyPart, &mut Transform, &Visibility)>,
) {
    let Ok((facing, walk)) = hero_query.single() else {
        return;
    };

    for (part, mut transform, visibility) in &mut part_query {
        if *visibility == Visibility::Hidden {
            continue;
        }

        let base = layout_for_part(*part, *facing);

        transform.rotation = Quat::from_rotation_z(base.rotation);

        if walk.active {
            let anim = walk_animation(*part, *facing, walk.phase);
            transform.translation = base.offset + anim.position_offset;
            transform.scale = anim.scale;
        } else {
            transform.translation = base.offset;
            transform.scale = Vec3::ONE;
        }
    }
}

// ---------------------------------------------------------------------------
// Walk animation data
// ---------------------------------------------------------------------------

struct WalkAnimationResult {
    position_offset: Vec3,
    scale: Vec3,
}

fn walk_animation(part: BodyPart, facing: Facing, phase: f32) -> WalkAnimationResult {
    match (part, facing) {
        // ---- Legs facing Down/Up: alternate height (scale Y) ----
        (BodyPart::LeftLeg, Facing::Down | Facing::Up) => {
            let height_factor = leg_height_factor(phase);
            let y_offset = leg_y_offset_from_scale(height_factor);
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, y_offset, 0.0),
                scale: Vec3::new(1.0, height_factor, 1.0),
            }
        }
        (BodyPart::RightLeg, Facing::Down | Facing::Up) => {
            let height_factor = leg_height_factor(phase + std::f32::consts::PI);
            let y_offset = leg_y_offset_from_scale(height_factor);
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, y_offset, 0.0),
                scale: Vec3::new(1.0, height_factor, 1.0),
            }
        }

        // ---- Legs facing Left/Right: small circular motion ----
        (BodyPart::LeftLeg, Facing::Left | Facing::Right) => {
            let offset = leg_circle_offset(phase, facing);
            WalkAnimationResult {
                position_offset: offset,
                scale: Vec3::ONE,
            }
        }
        (BodyPart::RightLeg, Facing::Left | Facing::Right) => {
            let offset = leg_circle_offset(phase + std::f32::consts::PI, facing);
            WalkAnimationResult {
                position_offset: offset,
                scale: Vec3::ONE,
            }
        }

        // ---- Fists facing Down/Up: bob up/down ----
        (BodyPart::LeftFist, Facing::Down | Facing::Up) => {
            let bob = phase.sin() * FIST_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, bob, 0.0),
                scale: Vec3::ONE,
            }
        }
        (BodyPart::RightFist, Facing::Down | Facing::Up) => {
            let bob = (phase + std::f32::consts::PI).sin() * FIST_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, bob, 0.0),
                scale: Vec3::ONE,
            }
        }

        // ---- Fists facing Left/Right: bob left/right ----
        (BodyPart::LeftFist, Facing::Left | Facing::Right) => {
            let bob = phase.sin() * FIST_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(bob, 0.0, 0.0),
                scale: Vec3::ONE,
            }
        }
        (BodyPart::RightFist, Facing::Left | Facing::Right) => {
            let bob = (phase + std::f32::consts::PI).sin() * FIST_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(bob, 0.0, 0.0),
                scale: Vec3::ONE,
            }
        }

        // ---- Head and hair: bob up/down ----
        (BodyPart::Head | BodyPart::Hair | BodyPart::LeftEye | BodyPart::RightEye, _) => {
            let bob = phase.sin().abs() * HEAD_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, bob, 0.0),
                scale: Vec3::ONE,
            }
        }

        // ---- Body: subtle bob ----
        (BodyPart::Body, _) => {
            let bob = phase.sin().abs() * BODY_BOB_AMPLITUDE;
            WalkAnimationResult {
                position_offset: Vec3::new(0.0, bob, 0.0),
                scale: Vec3::ONE,
            }
        }
    }
}

/// Leg height oscillates between shorter and taller.
/// Returns a scale factor around 1.0.
fn leg_height_factor(phase: f32) -> f32 {
    let normalized_amplitude = LEG_HEIGHT_AMPLITUDE / LEG_HEIGHT;
    1.0 + phase.sin() * normalized_amplitude
}

/// When a leg scales from its center, it grows/shrinks equally top and bottom.
/// This offset corrects so the top stays anchored and only the bottom moves.
/// Scaling from center moves both top and bottom equally.
/// This offset keeps the hip (top) anchored by shifting down when the leg
/// grows, so only the foot (bottom) moves.
fn leg_y_offset_from_scale(height_factor: f32) -> f32 {
    let height_change = (height_factor - 1.0) * LEG_HEIGHT;
    -height_change / 2.0
}

/// Legs trace a small circular path when facing left/right.
fn leg_circle_offset(phase: f32, facing: Facing) -> Vec3 {
    let x_offset = phase.cos() * LEG_CIRCLE_RADIUS_X;
    let y_offset = phase.sin() * LEG_CIRCLE_RADIUS_Y;
    let direction_sign = match facing {
        Facing::Left => 1.0,
        Facing::Right => -1.0,
        _ => 1.0,
    };
    Vec3::new(x_offset * direction_sign, y_offset, 0.0)
}

// ---------------------------------------------------------------------------
// Layout data — defines part positions per facing direction
// ---------------------------------------------------------------------------

struct PartLayout {
    offset: Vec3,
    visible: bool,
    rotation: f32,
}

fn layout_for_part(part: BodyPart, facing: Facing) -> PartLayout {
    match (part, facing) {
        // ---- Down (facing toward camera) ----
        (BodyPart::LeftLeg, Facing::Down) => PartLayout {
            offset: Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightLeg, Facing::Down) => PartLayout {
            offset: Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Body, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftFist, Facing::Down) => PartLayout {
            offset: Vec3::new(-FIST_X, FIST_Y, 2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightFist, Facing::Down) => PartLayout {
            offset: Vec3::new(FIST_X, FIST_Y, 2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Head, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 4.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Hair, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftEye, Facing::Down) => PartLayout {
            offset: Vec3::new(-EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, 6.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightEye, Facing::Down) => PartLayout {
            offset: Vec3::new(EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, 6.0),
            visible: true, rotation: 0.0,
        },

        // ---- Up (facing away from camera) ----
        (BodyPart::LeftLeg, Facing::Up) => PartLayout {
            offset: Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightLeg, Facing::Up) => PartLayout {
            offset: Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Body, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftFist, Facing::Up) => PartLayout {
            offset: Vec3::new(-FIST_X, FIST_Y, 2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightFist, Facing::Up) => PartLayout {
            offset: Vec3::new(FIST_X, FIST_Y, 2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Head, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 4.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Hair, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftEye, Facing::Up) => PartLayout {
            offset: Vec3::ZERO,
            visible: false, rotation: 0.0,
        },
        (BodyPart::RightEye, Facing::Up) => PartLayout {
            offset: Vec3::ZERO,
            visible: false, rotation: 0.0,
        },

        // ---- Left (facing left) ----
        (BodyPart::LeftLeg, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, 1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightLeg, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, -1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Body, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftFist, Facing::Left) => PartLayout {
            offset: Vec3::new(FIST_SIDE_X, FIST_Y, 3.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightFist, Facing::Left) => PartLayout {
            offset: Vec3::new(FIST_SIDE_X, FIST_Y, -2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Head, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Hair, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftEye, Facing::Left) => PartLayout {
            offset: Vec3::new(EYE_SIDE_X, HEAD_Y + EYE_Y_OFFSET, 7.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightEye, Facing::Left) => PartLayout {
            offset: Vec3::ZERO,
            visible: false, rotation: 0.0,
        },

        // ---- Right (facing right — mirror of left) ----
        (BodyPart::LeftLeg, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, -1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightLeg, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, 1.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Body, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 0.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftFist, Facing::Right) => PartLayout {
            offset: Vec3::new(-FIST_SIDE_X, FIST_Y, -2.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::RightFist, Facing::Right) => PartLayout {
            offset: Vec3::new(-FIST_SIDE_X, FIST_Y, 3.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Head, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::Hair, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true, rotation: 0.0,
        },
        (BodyPart::LeftEye, Facing::Right) => PartLayout {
            offset: Vec3::ZERO,
            visible: false, rotation: 0.0,
        },
        (BodyPart::RightEye, Facing::Right) => PartLayout {
            offset: Vec3::new(-EYE_SIDE_X, HEAD_Y + EYE_Y_OFFSET, 7.0),
            visible: true, rotation: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// Debug overlay — Gizmos showing bounding boxes
// ---------------------------------------------------------------------------

const DEBUG_COLOR_HERO_BOUNDS: Color = Color::srgb(0.0, 1.0, 0.0);
const DEBUG_COLOR_WORLD_BOUNDS: Color = Color::srgb(1.0, 1.0, 0.0);

fn draw_debug_bounds(
    debug: Res<DebugSettings>,
    mut gizmos: Gizmos,
    hero_query: Query<&Transform, With<Hero>>,
    camera_query: Query<(&Transform, &Projection), (With<Camera2d>, Without<Hero>)>,
) {
    if !debug.show_bounds {
        return;
    }
    // Hero bounding box
    let Ok(hero_transform) = hero_query.single() else {
        return;
    };
    let hero_pos = hero_transform.translation.truncate();
    let hero_center_y = (HERO_EXTENT_TOP - HERO_EXTENT_BOTTOM) / 2.0;
    let hero_center = hero_pos + Vec2::new(0.0, hero_center_y);
    let hero_size = Vec2::new(HERO_EXTENT_LEFT + HERO_EXTENT_RIGHT, HERO_EXTENT_TOP + HERO_EXTENT_BOTTOM);
    gizmos.rect_2d(hero_center, hero_size, DEBUG_COLOR_HERO_BOUNDS);

    // World bounds rectangle (screen edge minus margin — the green box should stay inside)
    let Ok((cam_transform, projection)) = camera_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let cam_x = cam_transform.translation.x;
    let cam_y = cam_transform.translation.y;
    let border_inset = 1.0;
    let bounds_min_x = cam_x + ortho.area.min.x + border_inset;
    let bounds_max_x = cam_x + ortho.area.max.x - border_inset;
    let bounds_min_y = cam_y + ortho.area.min.y + border_inset;
    let bounds_max_y = cam_y + ortho.area.max.y - border_inset;

    let bounds_center = Vec2::new(
        (bounds_min_x + bounds_max_x) / 2.0,
        (bounds_min_y + bounds_max_y) / 2.0,
    );
    let bounds_size = Vec2::new(
        bounds_max_x - bounds_min_x,
        bounds_max_y - bounds_min_y,
    );
    gizmos.rect_2d(bounds_center, bounds_size, DEBUG_COLOR_WORLD_BOUNDS);
}

// ---------------------------------------------------------------------------
// HUD — displays current state
// ---------------------------------------------------------------------------

fn update_hud_label(
    projection_query: Query<&Projection, With<Camera2d>>,
    hero_query: Query<(&Facing, &WalkAnimation), With<Hero>>,
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
    let speed_label = format!("{:.2}x", debug.animation_speed_factor);
    *text = Text::new(format!(
        "Zoom: {:.0}%  Facing: {}  State: {}  Anim: {}  |  F1: bounds  [/]: speed  \\: reset",
        zoom_percentage * 100.0,
        facing.label(),
        state_label,
        speed_label,
    ));
}
