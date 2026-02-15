//! Displays detected gamepad input values on screen.
//!
//! Run with: `cargo run --example input_display`
//!
//! Connect a gamepad to see live analog stick, trigger, and button values.
//! Designed so additional input sources (keyboard, second gamepad, etc.)
//! can be added as separate plugins without modifying existing code.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, InputDisplayPlugin))
        .run();
}

// ---------------------------------------------------------------------------
// Top-level plugin: wires sub-plugins together
// ---------------------------------------------------------------------------

struct InputDisplayPlugin;

impl Plugin for InputDisplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((GamepadInputPlugin, InputDisplayUiPlugin));
    }
}

// ---------------------------------------------------------------------------
// Gamepad input plugin: reads gamepad state into a resource
// ---------------------------------------------------------------------------

struct GamepadInputPlugin;

impl Plugin for GamepadInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GamepadInputState>()
            .add_systems(Update, read_gamepad_input);
    }
}

#[derive(Resource, Default)]
struct GamepadInputState {
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
    mut state: ResMut<GamepadInputState>,
) {
    let Some(gamepad) = gamepads.iter().next() else {
        state.connected = false;
        return;
    };

    state.connected = true;
    state.left_stick = gamepad.left_stick();
    state.right_stick = gamepad.right_stick();
    state.left_trigger = gamepad.get(GamepadButton::LeftTrigger2).unwrap_or(0.0);
    state.right_trigger = gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);

    state.buttons = GamepadButtonStates {
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

// ---------------------------------------------------------------------------
// Display UI plugin: reads input state resources and renders text
// ---------------------------------------------------------------------------

struct InputDisplayUiPlugin;

impl Plugin for InputDisplayUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_ui)
            .add_systems(Update, update_display.after(read_gamepad_input));
    }
}

#[derive(Component)]
struct InputDisplayText;

const HEADER_FONT_SIZE: f32 = 28.0;

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        Text::new("No gamepad detected"),
        TextFont::from_font_size(HEADER_FONT_SIZE),
        TextColor::WHITE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
        InputDisplayText,
    ));
}

fn update_display(
    gamepad_state: Res<GamepadInputState>,
    mut query: Query<&mut Text, With<InputDisplayText>>,
) {
    let Ok(mut text) = query.single_mut() else {
        return;
    };

    if !gamepad_state.connected {
        **text = "No gamepad detected\n\nConnect a controller to see input values.".into();
        return;
    }

    let display = format_gamepad_display(&gamepad_state);
    **text = display;
}

fn format_gamepad_display(state: &GamepadInputState) -> String {
    let left = state.left_stick;
    let right = state.right_stick;
    let buttons = &state.buttons;

    let left_stick_visual = stick_visual(left);
    let right_stick_visual = stick_visual(right);

    format!(
        "Gamepad Connected\n\
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
