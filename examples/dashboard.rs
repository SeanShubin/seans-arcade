//! Displays live input and window state in a four-panel dashboard.
//!
//! Run with: `cargo run --example dashboard`
//!
//! Shows gamepad input (2 controllers), mouse state, and window info
//! side by side. Useful for diagnosing input and display configuration.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

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
        app.add_plugins((
            DualGamepadInputPlugin,
            MouseInputPlugin,
            WindowInfoPlugin,
            DualInputDisplayUiPlugin,
        ));
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
        app.add_systems(Startup, setup_ui).add_systems(
            Update,
            (
                update_display.after(read_gamepad_input),
                update_mouse_display.after(read_mouse_input),
                update_window_display.after(read_window_info),
            ),
        );
    }
}

#[derive(Component)]
struct GamepadDisplayText {
    index: usize,
}

const FONT_SIZE: f32 = 20.0;
const GAP: f32 = 10.0;
const MARGIN: f32 = 10.0;
const BORDER_WIDTH: f32 = 2.0;
const PANEL_PADDING: f32 = 12.0;
const BORDER_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);

fn panel_node() -> Node {
    Node {
        border: UiRect::all(Val::Px(BORDER_WIDTH)),
        padding: UiRect::all(Val::Px(PANEL_PADDING)),
        ..default()
    }
}

fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    let flex_root = commands
        .spawn(Node {
            display: Display::Flex,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::FlexStart,
            width: Val::Percent(100.0),
            padding: UiRect::all(Val::Px(MARGIN)),
            column_gap: Val::Px(GAP),
            row_gap: Val::Px(GAP),
            ..default()
        })
        .id();

    for index in 0..GAMEPAD_COUNT {
        let gamepad_number = index + 1;
        let header = format!("Gamepad {gamepad_number}: No gamepad detected");

        let panel = commands
            .spawn((panel_node(), BorderColor::all(BORDER_COLOR)))
            .id();
        let text = commands
            .spawn((
                Text::new(header),
                TextFont::from_font_size(FONT_SIZE),
                TextColor::WHITE,
                GamepadDisplayText { index },
            ))
            .id();
        commands.entity(panel).add_child(text);
        commands.entity(flex_root).add_child(panel);
    }

    let mouse_panel = commands
        .spawn((panel_node(), BorderColor::all(BORDER_COLOR)))
        .id();
    let mouse_text = commands
        .spawn((
            Text::new("Mouse\n\nWaiting for input..."),
            TextFont::from_font_size(FONT_SIZE),
            TextColor::WHITE,
            MouseDisplayText,
        ))
        .id();
    commands.entity(mouse_panel).add_child(mouse_text);
    commands.entity(flex_root).add_child(mouse_panel);

    let window_panel = commands
        .spawn((panel_node(), BorderColor::all(BORDER_COLOR)))
        .id();
    let window_text = commands
        .spawn((
            Text::new("Window\n\nLoading..."),
            TextFont::from_font_size(FONT_SIZE),
            TextColor::WHITE,
            WindowDisplayText,
        ))
        .id();
    commands.entity(window_panel).add_child(window_text);
    commands.entity(flex_root).add_child(window_panel);
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

// ---------------------------------------------------------------------------
// Mouse input plugin: reads mouse state into a resource
// ---------------------------------------------------------------------------

struct MouseInputPlugin;

impl Plugin for MouseInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MouseInputState>()
            .add_systems(Update, read_mouse_input);
    }
}

#[derive(Resource, Default)]
struct MouseInputState {
    cursor_position: Option<Vec2>,
    world_position: Option<Vec2>,
    left_button: bool,
    right_button: bool,
    middle_button: bool,
    scroll_delta: Vec2,
}

fn read_mouse_input(
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut state: ResMut<MouseInputState>,
) {
    let cursor_pos = windows.single().ok().and_then(|w| w.cursor_position());
    state.cursor_position = cursor_pos;

    state.world_position = cursor_pos.and_then(|pos| {
        let (cam, transform) = camera.single().ok()?;
        cam.viewport_to_world_2d(transform, pos).ok()
    });

    state.left_button = buttons.pressed(MouseButton::Left);
    state.right_button = buttons.pressed(MouseButton::Right);
    state.middle_button = buttons.pressed(MouseButton::Middle);

    let mut scroll = Vec2::ZERO;
    for event in scroll_events.read() {
        scroll.x += event.x;
        scroll.y += event.y;
    }
    state.scroll_delta = scroll;
}

#[derive(Component)]
struct MouseDisplayText;

fn format_mouse_display(state: &MouseInputState) -> String {
    let format_pos = |pos: Option<Vec2>| match pos {
        Some(p) => format!("X: {:>7.1}  Y: {:>7.1}", p.x, p.y),
        None => "X:     ---  Y:     ---".to_string(),
    };

    let cursor = format_pos(state.cursor_position);
    let world = format_pos(state.world_position);

    let btn = |name: &str, pressed: bool| -> String {
        if pressed {
            format!("[{name}]")
        } else {
            format!(" {name} ")
        }
    };

    format!(
        "Mouse\n\
         \n\
         Cursor   {cursor}\n\
         World    {world}\n\
         \n\
         Buttons  {}  {}  {}\n\
         \n\
         Scroll   X: {:>6.2}  Y: {:>6.2}",
        btn("Left", state.left_button),
        btn("Mid", state.middle_button),
        btn("Right", state.right_button),
        state.scroll_delta.x,
        state.scroll_delta.y,
    )
}

fn update_mouse_display(
    mouse_state: Res<MouseInputState>,
    mut query: Query<&mut Text, With<MouseDisplayText>>,
) {
    for mut text in &mut query {
        **text = format_mouse_display(&mouse_state);
    }
}

// ---------------------------------------------------------------------------
// Window info plugin: reads window properties into a resource
// ---------------------------------------------------------------------------

struct WindowInfoPlugin;

impl Plugin for WindowInfoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WindowInfoState>()
            .add_systems(Update, read_window_info);
    }
}

#[derive(Resource, Default)]
struct WindowInfoState {
    logical_size: Vec2,
    physical_width: u32,
    physical_height: u32,
    position: Option<IVec2>,
    mode: String,
    focused: bool,
    scale_factor: f32,
}

fn read_window_info(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<WindowInfoState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };

    state.logical_size = Vec2::new(window.width(), window.height());
    state.physical_width = window.physical_width();
    state.physical_height = window.physical_height();
    state.position = match window.position {
        WindowPosition::At(pos) => Some(pos),
        _ => None,
    };
    state.mode = format!("{:?}", window.mode);
    state.focused = window.focused;
    state.scale_factor = window.scale_factor();
}

#[derive(Component)]
struct WindowDisplayText;

fn format_window_display(state: &WindowInfoState) -> String {
    let position = match state.position {
        Some(pos) => format!("X: {}  Y: {}", pos.x, pos.y),
        None => "Automatic".to_string(),
    };

    format!(
        "Window\n\
         \n\
         Logical   {:.0} x {:.0}\n\
         Physical  {} x {}\n\
         \n\
         Position  {position}\n\
         Mode      {}\n\
         Focused   {}\n\
         Scale     {:.2}",
        state.logical_size.x,
        state.logical_size.y,
        state.physical_width,
        state.physical_height,
        state.mode,
        state.focused,
        state.scale_factor,
    )
}

fn update_window_display(
    window_state: Res<WindowInfoState>,
    mut query: Query<&mut Text, With<WindowDisplayText>>,
) {
    for mut text in &mut query {
        **text = format_window_display(&window_state);
    }
}

// ---------------------------------------------------------------------------
// Gamepad display formatting helpers
// ---------------------------------------------------------------------------

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
