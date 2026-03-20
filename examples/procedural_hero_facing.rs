//! Procedural hero — Example 2: Facing Directions
//!
//! Arrow keys change the hero's facing direction. Each direction rearranges
//! body parts — positions, z-order, and visibility change to show the correct
//! top-down perspective.
//!
//! Controls:
//!   - Arrow keys / gamepad d-pad: change facing direction
//!   - Scroll wheel: zoom in/out
//!   - Click + drag: pan camera
//!   - Home key: reset zoom and pan
//!
//! Run with: `cargo run --example procedural_hero_facing`

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

const ARM_SIDE_X: f32 = 0.0;
const ARM_SIDE_FRONT_Y: f32 = 12.0;
const ARM_SIDE_BACK_Y: f32 = 22.0;

const SHIELD_WIDTH: f32 = 8.0;
const SHIELD_HEIGHT: f32 = 12.0;
const SHIELD_X: f32 = -18.0;
const SHIELD_Y: f32 = 16.0;

const EYE_RADIUS: f32 = 1.5;
const EYE_SPREAD: f32 = 4.0;
const EYE_Y_OFFSET: f32 = -2.0;
const EYE_SIDE_X: f32 = -4.0;

// Hero palette
const COLOR_SKIN: Color = Color::srgb(0.93, 0.76, 0.57);
const COLOR_TUNIC: Color = Color::srgb(0.18, 0.55, 0.18);
const COLOR_PANTS: Color = Color::srgb(0.55, 0.40, 0.25);
const COLOR_SHIELD: Color = Color::srgb(0.25, 0.35, 0.65);
const COLOR_HAIR: Color = Color::srgb(0.75, 0.60, 0.20);
const COLOR_EYE: Color = Color::BLACK;

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
// Components & Events
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
    LeftArm,
    RightArm,
    Shield,
    Head,
    Hair,
    LeftEye,
    RightEye,
}

#[derive(Component)]
struct Hero;

#[derive(Component)]
struct ZoomLabel;

#[derive(Message)]
struct ChangeFacing(Facing);

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

/// Tracks previous frame's d-pad state for edge detection.
#[derive(Default)]
struct DpadState {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
}

impl DpadState {
    fn just_pressed(&self, previous: &DpadState) -> DpadState {
        DpadState {
            up: self.up && !previous.up,
            down: self.down && !previous.down,
            left: self.left && !previous.left,
            right: self.right && !previous.right,
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Procedural Hero — Facing".into(),
                resolution: bevy::window::WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT),
                ..default()
            }),
            ..default()
        }))
        .add_message::<ChangeFacing>()
        .add_systems(Startup, (setup_camera, setup_hero, setup_hud))
        .add_systems(
            Update,
            (
                read_facing_input,
                read_gamepad_facing_input,
                read_zoom_input,
                read_pan_input,
                read_reset_input,
                update_facing,
                arrange_body_parts,
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
        .spawn((Hero, Facing::default(), Transform::default(), Visibility::default()))
        .id();

    spawn_part(&mut commands, hero, BodyPart::LeftLeg, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::RightLeg, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::Body, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::LeftArm, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::RightArm, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::Shield, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::Head, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::Hair, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::LeftEye, &mut meshes, &mut materials);
    spawn_part(&mut commands, hero, BodyPart::RightEye, &mut meshes, &mut materials);
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

    let child = commands
        .spawn((
            part,
            Mesh2d(mesh),
            MeshMaterial2d(material),
            Transform::from_translation(layout.offset),
            if layout.visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
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
        BodyPart::LeftArm | BodyPart::RightArm => (
            meshes.add(Rectangle::new(ARM_WIDTH, ARM_HEIGHT)),
            materials.add(ColorMaterial::from_color(COLOR_SKIN)),
        ),
        BodyPart::Shield => (
            meshes.add(Rectangle::new(SHIELD_WIDTH, SHIELD_HEIGHT)),
            materials.add(ColorMaterial::from_color(COLOR_SHIELD)),
        ),
        BodyPart::Head => (
            meshes.add(Circle::new(HEAD_RADIUS)),
            materials.add(ColorMaterial::from_color(COLOR_SKIN)),
        ),
        BodyPart::Hair => {
            let hair_radius = HEAD_RADIUS * HAIR_RADIUS_RATIO;
            (
                meshes.add(Circle::new(hair_radius)),
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
// Input layer — translates raw input into events and camera changes
// ---------------------------------------------------------------------------

fn read_facing_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut facing_events: MessageWriter<ChangeFacing>,
) {
    if keys.just_pressed(KeyCode::ArrowDown) {
        facing_events.write(ChangeFacing(Facing::Down));
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        facing_events.write(ChangeFacing(Facing::Up));
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        facing_events.write(ChangeFacing(Facing::Left));
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        facing_events.write(ChangeFacing(Facing::Right));
    }
}

fn read_gamepad_facing_input(
    mut facing_events: MessageWriter<ChangeFacing>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
    mut previous_dpad: Local<DpadState>,
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
        *previous_dpad = DpadState::default();
        return;
    }

    let gp = unsafe { &xinput_state.assume_init().gamepad };
    let btn = |mask: u16| gp.buttons & mask != 0;

    let current_dpad = DpadState {
        up: btn(XINPUT_GAMEPAD_DPAD_UP),
        down: btn(XINPUT_GAMEPAD_DPAD_DOWN),
        left: btn(XINPUT_GAMEPAD_DPAD_LEFT),
        right: btn(XINPUT_GAMEPAD_DPAD_RIGHT),
    };

    let just_pressed = current_dpad.just_pressed(&previous_dpad);

    if just_pressed.down {
        facing_events.write(ChangeFacing(Facing::Down));
    }
    if just_pressed.up {
        facing_events.write(ChangeFacing(Facing::Up));
    }
    if just_pressed.left {
        facing_events.write(ChangeFacing(Facing::Left));
    }
    if just_pressed.right {
        facing_events.write(ChangeFacing(Facing::Right));
    }

    *previous_dpad = current_dpad;
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

// ---------------------------------------------------------------------------
// Simulation layer — updates hero state from events
// ---------------------------------------------------------------------------

fn update_facing(
    mut facing_events: MessageReader<ChangeFacing>,
    mut hero_query: Query<&mut Facing, With<Hero>>,
) {
    let Some(event) = facing_events.read().last() else {
        return;
    };
    let Ok(mut facing) = hero_query.single_mut() else {
        return;
    };
    *facing = event.0;
}

// ---------------------------------------------------------------------------
// Presentation layer — arranges body parts based on facing
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
        *visibility = if layout.visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

// ---------------------------------------------------------------------------
// Layout data — defines part positions per facing direction
// ---------------------------------------------------------------------------

struct PartLayout {
    offset: Vec3,
    visible: bool,
}

fn layout_for_part(part: BodyPart, facing: Facing) -> PartLayout {
    match (part, facing) {
        // ---- Down (facing toward camera) ----
        (BodyPart::LeftLeg, Facing::Down) => PartLayout {
            offset: Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::RightLeg, Facing::Down) => PartLayout {
            offset: Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::Body, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true,
        },
        (BodyPart::LeftArm, Facing::Down) => PartLayout {
            offset: Vec3::new(-ARM_X, ARM_Y, 2.0),
            visible: true,
        },
        (BodyPart::RightArm, Facing::Down) => PartLayout {
            offset: Vec3::new(ARM_X, ARM_Y, 2.0),
            visible: true,
        },
        (BodyPart::Shield, Facing::Down) => PartLayout {
            offset: Vec3::new(SHIELD_X, SHIELD_Y, 3.0),
            visible: true,
        },
        (BodyPart::Head, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 4.0),
            visible: true,
        },
        (BodyPart::Hair, Facing::Down) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y + HAIR_Y_OFFSET, 5.0),
            visible: true,
        },
        (BodyPart::LeftEye, Facing::Down) => PartLayout {
            offset: Vec3::new(-EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, 6.0),
            visible: true,
        },
        (BodyPart::RightEye, Facing::Down) => PartLayout {
            offset: Vec3::new(EYE_SPREAD, HEAD_Y + EYE_Y_OFFSET, 6.0),
            visible: true,
        },

        // ---- Up (facing away from camera) ----
        (BodyPart::LeftLeg, Facing::Up) => PartLayout {
            offset: Vec3::new(-LEG_SPREAD, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::RightLeg, Facing::Up) => PartLayout {
            offset: Vec3::new(LEG_SPREAD, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::Body, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true,
        },
        (BodyPart::LeftArm, Facing::Up) => PartLayout {
            offset: Vec3::new(-ARM_X, ARM_Y, 2.0),
            visible: true,
        },
        (BodyPart::RightArm, Facing::Up) => PartLayout {
            offset: Vec3::new(ARM_X, ARM_Y, 2.0),
            visible: true,
        },
        (BodyPart::Shield, Facing::Up) => PartLayout {
            offset: Vec3::new(-SHIELD_X, SHIELD_Y, 3.0),
            visible: true,
        },
        (BodyPart::Head, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 4.0),
            visible: true,
        },
        (BodyPart::Hair, Facing::Up) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y + HAIR_Y_OFFSET, 6.0),
            visible: true,
        },
        (BodyPart::LeftEye, Facing::Up) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },
        (BodyPart::RightEye, Facing::Up) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },

        // ---- Left (facing left) ----
        (BodyPart::LeftLeg, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::RightLeg, Facing::Left) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },
        (BodyPart::Body, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true,
        },
        (BodyPart::LeftArm, Facing::Left) => PartLayout {
            offset: Vec3::new(ARM_SIDE_X, ARM_SIDE_FRONT_Y, 3.0),
            visible: true,
        },
        (BodyPart::RightArm, Facing::Left) => PartLayout {
            offset: Vec3::new(ARM_SIDE_X, ARM_SIDE_BACK_Y, -1.0),
            visible: true,
        },
        (BodyPart::Shield, Facing::Left) => PartLayout {
            offset: Vec3::new(-ARM_X, SHIELD_Y, 4.0),
            visible: true,
        },
        (BodyPart::Head, Facing::Left) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true,
        },
        (BodyPart::Hair, Facing::Left) => PartLayout {
            offset: Vec3::new(3.0, HEAD_Y + HAIR_Y_OFFSET, 6.0),
            visible: true,
        },
        (BodyPart::LeftEye, Facing::Left) => PartLayout {
            offset: Vec3::new(EYE_SIDE_X, HEAD_Y + EYE_Y_OFFSET, 7.0),
            visible: true,
        },
        (BodyPart::RightEye, Facing::Left) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },

        // ---- Right (facing right — mirror of left) ----
        (BodyPart::LeftLeg, Facing::Right) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },
        (BodyPart::RightLeg, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, LEG_Y, 0.0),
            visible: true,
        },
        (BodyPart::Body, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, BODY_Y, 1.0),
            visible: true,
        },
        (BodyPart::LeftArm, Facing::Right) => PartLayout {
            offset: Vec3::new(-ARM_SIDE_X, ARM_SIDE_BACK_Y, -1.0),
            visible: true,
        },
        (BodyPart::RightArm, Facing::Right) => PartLayout {
            offset: Vec3::new(-ARM_SIDE_X, ARM_SIDE_FRONT_Y, 3.0),
            visible: true,
        },
        (BodyPart::Shield, Facing::Right) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },
        (BodyPart::Head, Facing::Right) => PartLayout {
            offset: Vec3::new(0.0, HEAD_Y, 5.0),
            visible: true,
        },
        (BodyPart::Hair, Facing::Right) => PartLayout {
            offset: Vec3::new(-3.0, HEAD_Y + HAIR_Y_OFFSET, 6.0),
            visible: true,
        },
        (BodyPart::LeftEye, Facing::Right) => PartLayout {
            offset: Vec3::ZERO,
            visible: false,
        },
        (BodyPart::RightEye, Facing::Right) => PartLayout {
            offset: Vec3::new(-EYE_SIDE_X, HEAD_Y + EYE_Y_OFFSET, 7.0),
            visible: true,
        },
    }
}

// ---------------------------------------------------------------------------
// HUD — displays current zoom and facing
// ---------------------------------------------------------------------------

fn update_hud_label(
    projection_query: Query<&Projection, With<Camera2d>>,
    hero_query: Query<&Facing, With<Hero>>,
    mut label_query: Query<&mut Text, With<ZoomLabel>>,
) {
    let Ok(projection) = projection_query.single() else {
        return;
    };
    let Projection::Orthographic(ref ortho) = *projection else {
        return;
    };
    let Ok(facing) = hero_query.single() else {
        return;
    };
    let Ok(mut text) = label_query.single_mut() else {
        return;
    };

    let zoom_percentage = 1.0 / ortho.scale;
    *text = Text::new(format!(
        "Zoom: {:.0}%  Facing: {}",
        zoom_percentage * 100.0,
        facing.label()
    ));
}
