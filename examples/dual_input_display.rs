//! Displays detected gamepad input values from 2 controllers simultaneously.
//!
//! Run with: `cargo run --example dual_input_display`
//!
//! Connect one or two gamepads to see live analog stick, trigger, and button
//! values displayed side by side. Useful for diagnosing local multiplayer setups.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DualInputDisplayPlugin))
        .run();
}

// ---------------------------------------------------------------------------
// Top-level plugin: wires sub-plugins together
// ---------------------------------------------------------------------------

struct DualInputDisplayPlugin;

impl Plugin for DualInputDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((DualGamepadInputPlugin, DualInputDisplayUiPlugin));
    }
}

// ---------------------------------------------------------------------------
// Gamepad input plugin: reads gamepad state into a resource
// ---------------------------------------------------------------------------

struct DualGamepadInputPlugin;

impl Plugin for DualGamepadInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DualGamepadInputState>()
            .add_systems(Update, read_gamepad_input);
    }
}

const GAMEPAD_COUNT: usize = 2;

#[derive(Resource, Default)]
struct DualGamepadInputState {
    gamepads: [SingleGamepadState; GAMEPAD_COUNT],
}

#[derive(Default)]
struct SingleGamepadState {
    connected: bool,
    left_stick: Vec2,
    right_stick: Vec2,
    left_trigger: f32,
    right_trigger: f32,
    buttons: GamepadButtonStates,
}

#[derive(Default)]
struct GamepadButtonStates {
    south: bool,
    east: bool,
    north: bool,
    west: bool,
    left_bumper: bool,
    right_bumper: bool,
    start: bool,
    select: bool,
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
}

fn read_gamepad_input(
    gamepads: Query<&Gamepad>,
    mut state: ResMut<DualGamepadInputState>,
) {
    let mut gamepad_iter = gamepads.iter();

    for slot in &mut state.gamepads {
        let Some(gamepad) = gamepad_iter.next() else {
            slot.connected = false;
            slot.left_stick = Vec2::ZERO;
            slot.right_stick = Vec2::ZERO;
            slot.left_trigger = 0.0;
            slot.right_trigger = 0.0;
            slot.buttons = GamepadButtonStates::default();
            continue;
        };

        slot.connected = true;
        slot.left_stick = gamepad.left_stick();
        slot.right_stick = gamepad.right_stick();
        slot.left_trigger = gamepad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
        slot.right_trigger = gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);

        slot.buttons = GamepadButtonStates {
            south: gamepad.pressed(GamepadButton::South),
            east: gamepad.pressed(GamepadButton::East),
            north: gamepad.pressed(GamepadButton::North),
            west: gamepad.pressed(GamepadButton::West),
            left_bumper: gamepad.pressed(GamepadButton::LeftTrigger),
            right_bumper: gamepad.pressed(GamepadButton::RightTrigger),
            start: gamepad.pressed(GamepadButton::Start),
            select: gamepad.pressed(GamepadButton::Select),
            dpad_up: gamepad.pressed(GamepadButton::DPadUp),
            dpad_down: gamepad.pressed(GamepadButton::DPadDown),
            dpad_left: gamepad.pressed(GamepadButton::DPadLeft),
            dpad_right: gamepad.pressed(GamepadButton::DPadRight),
        };
    }
}

// ---------------------------------------------------------------------------
// Display UI plugin: reads input state resources and renders text
// ---------------------------------------------------------------------------

struct DualInputDisplayUiPlugin;

impl Plugin for DualInputDisplayUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(Update, update_display.after(read_gamepad_input));
    }
}

#[derive(Component)]
struct GamepadDisplayText {
    index: usize,
}

const FONT_SIZE: f32 = 20.0;

// Layout derivation: position columns so they cannot overlap.
//
// Widest content line (characters):
//   "Connect a controller to see input values." = 42 chars
//   "Left Stick   X: -1.000  Y: -1.000"        = 34 chars
//   "  L [##########]  R [##########]"           = 32 chars
//
// Bevy's default font at a given size has approximately 0.6x character width,
// so each character is roughly (FONT_SIZE * 0.6) pixels wide.
const MAX_CONTENT_CHARS: f32 = 42.0;
const ESTIMATED_CHAR_WIDTH: f32 = FONT_SIZE * 0.6;
const ESTIMATED_COLUMN_WIDTH: f32 = MAX_CONTENT_CHARS * ESTIMATED_CHAR_WIDTH;
const COLUMN_GAP: f32 = 40.0;
const MARGIN: f32 = 20.0;

const LEFT_COLUMN_X: f32 = MARGIN;
const RIGHT_COLUMN_X: f32 = MARGIN + ESTIMATED_COLUMN_WIDTH + COLUMN_GAP;
const COLUMN_TOP: f32 = 20.0;

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    let column_positions = [LEFT_COLUMN_X, RIGHT_COLUMN_X];

    for (index, &x_offset) in column_positions.iter().enumerate() {
        let gamepad_number = index + 1;
        let header = format!("Gamepad {gamepad_number}: No gamepad detected");

        commands.spawn((
            Text::new(header),
            TextFont::from_font_size(FONT_SIZE),
            TextColor::WHITE,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(COLUMN_TOP),
                left: Val::Px(x_offset),
                ..default()
            },
            GamepadDisplayText { index },
        ));
    }
}

fn update_display(
    gamepad_state: Res<DualGamepadInputState>,
    mut query: Query<(&mut Text, &GamepadDisplayText)>,
) {
    for (mut text, display_marker) in &mut query {
        let pad = &gamepad_state.gamepads[display_marker.index];
        let gamepad_number = display_marker.index + 1;

        if !pad.connected {
            **text = format!(
                "Gamepad {gamepad_number}\n\nNo gamepad detected\n\nConnect a controller to see input values."
            );
            continue;
        }

        let display = format_gamepad_display(gamepad_number, pad);
        **text = display;
    }
}

fn format_gamepad_display(gamepad_number: usize, state: &SingleGamepadState) -> String {
    let left = state.left_stick;
    let right = state.right_stick;
    let buttons = &state.buttons;

    let left_stick_visual = stick_visual(left);
    let right_stick_visual = stick_visual(right);

    format!(
        "Gamepad {gamepad_number}\n\
         \n\
         Left Stick   X: {:>6.3}  Y: {:>6.3}\n\
         {left_stick_visual}\n\
         \n\
         Right Stick  X: {:>6.3}  Y: {:>6.3}\n\
         {right_stick_visual}\n\
         \n\
         Triggers     L: {:>5.3}   R: {:>5.3}\n\
         {}\n\
         \n\
         Buttons\n\
         {}",
        left.x,
        left.y,
        right.x,
        right.y,
        state.left_trigger,
        state.right_trigger,
        trigger_bar(state.left_trigger, state.right_trigger),
        format_buttons(buttons),
    )
}

fn stick_visual(stick: Vec2) -> String {
    let grid_size = 9;
    let center = grid_size / 2;
    let col = ((stick.x + 1.0) / 2.0 * (grid_size - 1) as f32).round() as usize;
    let row = ((-stick.y + 1.0) / 2.0 * (grid_size - 1) as f32).round() as usize;
    let col = col.min(grid_size - 1);
    let row = row.min(grid_size - 1);

    let mut lines = Vec::with_capacity(grid_size);
    for r in 0..grid_size {
        let mut line = String::with_capacity(grid_size * 2 + 4);
        line.push_str("  ");
        for c in 0..grid_size {
            if r == row && c == col {
                line.push_str("@ ");
            } else if r == center && c == center {
                line.push_str("+ ");
            } else if r == 0 || r == grid_size - 1 || c == 0 || c == grid_size - 1 {
                line.push_str(". ");
            } else {
                line.push_str("  ");
            }
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn trigger_bar(left: f32, right: f32) -> String {
    let bar_width = 10;
    let left_filled = (left * bar_width as f32).round() as usize;
    let right_filled = (right * bar_width as f32).round() as usize;

    let left_bar = format!(
        "[{}{}]",
        "#".repeat(left_filled.min(bar_width)),
        "-".repeat(bar_width - left_filled.min(bar_width)),
    );
    let right_bar = format!(
        "[{}{}]",
        "#".repeat(right_filled.min(bar_width)),
        "-".repeat(bar_width - right_filled.min(bar_width)),
    );

    format!("  L {left_bar}  R {right_bar}")
}

fn format_buttons(buttons: &GamepadButtonStates) -> String {
    let btn = |name: &str, pressed: bool| -> String {
        if pressed {
            format!("[{name}]")
        } else {
            format!(" {name} ")
        }
    };

    let face = format!(
        "  Face:    {}  {}  {}  {}",
        btn("S", buttons.south),
        btn("E", buttons.east),
        btn("N", buttons.north),
        btn("W", buttons.west),
    );

    let shoulders = format!(
        "  Bumpers: {}  {}",
        btn("LB", buttons.left_bumper),
        btn("RB", buttons.right_bumper),
    );

    let menu = format!(
        "  Menu:    {}  {}",
        btn("Start", buttons.start),
        btn("Select", buttons.select),
    );

    let dpad = format!(
        "  D-Pad:   {}  {}  {}  {}",
        btn("U", buttons.dpad_up),
        btn("D", buttons.dpad_down),
        btn("L", buttons.dpad_left),
        btn("R", buttons.dpad_right),
    );

    format!("{face}\n{shoulders}\n{menu}\n{dpad}")
}
