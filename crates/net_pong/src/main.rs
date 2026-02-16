//! Networked Pong — deterministic lockstep over UDP.
//!
//! Two clients connect to a relay server. Each client sends its local input
//! for the current tick; the relay broadcasts both inputs back. Both clients
//! then advance the simulation identically.
//!
//! Usage: `cargo run -p net_pong [relay_address]`
//! Default relay address: `127.0.0.1:7700`

use std::net::{SocketAddr, UdpSocket};

use bevy::prelude::*;
use relay::{ClientMessage, RelayMessage, Tick, deserialize, serialize};

fn main() {
    let relay_addr: SocketAddr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7700".into())
        .parse()
        .expect("invalid relay address");

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(RelayAddress(relay_addr))
        .add_plugins(NetPongPlugin)
        .run();
}

// ---------------------------------------------------------------------------
// Top-level plugin
// ---------------------------------------------------------------------------

struct NetPongPlugin;

impl Plugin for NetPongPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            NetPongConnectionPlugin,
            NetPongInputPlugin,
            NetPongGamePlugin,
            NetPongRenderPlugin,
        ));
    }
}

// ---------------------------------------------------------------------------
// Arena and gameplay constants (identical to examples/pong.rs)
// ---------------------------------------------------------------------------

const ARENA_WIDTH: f32 = 800.0;
const ARENA_HEIGHT: f32 = 500.0;
const PADDLE_WIDTH: f32 = 15.0;
const PADDLE_HEIGHT: f32 = 80.0;
const PADDLE_X_OFFSET: f32 = 30.0;
const PADDLE_SPEED: f32 = 400.0;
const BALL_SIZE: f32 = 12.0;
const BALL_INITIAL_SPEED: f32 = 300.0;
const BALL_SPEED_INCREASE: f32 = 25.0;
const PADDLE_HIT_ANGLE_FACTOR: f32 = 0.5;
const PLAYER_COUNT: usize = 2;

// ---------------------------------------------------------------------------
// Shared components and resources
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Paddle {
    player_index: usize,
}

#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Resource, Default)]
struct Score {
    points: [u32; PLAYER_COUNT],
}

#[derive(Resource, Default)]
struct BallResetCounter(u32);

#[derive(Resource, Default)]
struct PaddleInput {
    movement: [f32; PLAYER_COUNT],
}

// ---------------------------------------------------------------------------
// Connection plugin: UDP setup, handshake, message receive
// ---------------------------------------------------------------------------

struct NetPongConnectionPlugin;

impl Plugin for NetPongConnectionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ConnectionState::Connecting)
            .insert_resource(SimulationTick(0))
            .insert_resource(TickReady(false))
            .insert_resource(NeedToSendInput(false))
            .insert_resource(HelloTimer(Timer::from_seconds(0.5, TimerMode::Repeating)))
            .insert_resource(LocalPlayerSlot(0))
            .add_systems(Startup, setup_network)
            .add_systems(
                Update,
                (send_hello.run_if(is_connecting), receive_relay_messages),
            );
    }
}

#[derive(Resource)]
struct RelayAddress(SocketAddr);

#[derive(Resource)]
struct NetSocket {
    socket: UdpSocket,
    relay_addr: SocketAddr,
}

#[derive(Resource, PartialEq, Eq)]
enum ConnectionState {
    Connecting,
    WaitingForOpponent,
    Playing,
}

#[derive(Resource)]
struct SimulationTick(Tick);

#[derive(Resource)]
struct TickReady(bool);

#[derive(Resource)]
struct NeedToSendInput(bool);

#[derive(Resource)]
struct HelloTimer(Timer);

#[derive(Resource)]
struct LocalPlayerSlot(u8);

fn is_connecting(state: Res<ConnectionState>) -> bool {
    *state == ConnectionState::Connecting
}

fn is_playing(state: Res<ConnectionState>) -> bool {
    *state == ConnectionState::Playing
}

fn tick_is_ready(ready: Res<TickReady>) -> bool {
    ready.0
}

fn need_to_send(need: Res<NeedToSendInput>) -> bool {
    need.0
}

fn setup_network(mut commands: Commands, relay_addr: Res<RelayAddress>) {
    let socket =
        UdpSocket::bind("0.0.0.0:0").expect("failed to bind local UDP socket");
    socket
        .set_nonblocking(true)
        .expect("failed to set non-blocking");

    let addr = relay_addr.0;
    commands.insert_resource(NetSocket {
        socket,
        relay_addr: addr,
    });
}

fn send_hello(
    net: Res<NetSocket>,
    mut timer: ResMut<HelloTimer>,
    time: Res<Time>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        let msg = serialize(&ClientMessage::Hello);
        let _ = net.socket.send_to(&msg, net.relay_addr);
    }
}

fn receive_relay_messages(
    net: Res<NetSocket>,
    mut state: ResMut<ConnectionState>,
    mut local_slot: ResMut<LocalPlayerSlot>,
    mut tick_ready: ResMut<TickReady>,
    mut need_send: ResMut<NeedToSendInput>,
    mut input: ResMut<PaddleInput>,
    sim_tick: Res<SimulationTick>,
) {
    let mut buf = [0u8; 1024];
    loop {
        let len = match net.socket.recv(&mut buf) {
            Ok(len) => len,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => {
                eprintln!("net_pong: recv error: {e}");
                break;
            }
        };

        let Some(msg) = deserialize::<RelayMessage>(&buf[..len]) else {
            continue;
        };

        match msg {
            RelayMessage::Welcome { player_slot } => {
                local_slot.0 = player_slot;
                if *state == ConnectionState::Connecting {
                    *state = ConnectionState::WaitingForOpponent;
                    println!("net_pong: assigned slot {player_slot}");
                }
            }
            RelayMessage::GameStart => {
                if *state != ConnectionState::Playing {
                    *state = ConnectionState::Playing;
                    need_send.0 = true;
                    println!("net_pong: game starting!");
                }
            }
            RelayMessage::TickInputs { tick, inputs } => {
                if tick != sim_tick.0 {
                    continue;
                }
                // Apply inputs from both players.
                for (i, payload) in inputs.iter().enumerate() {
                    if let Some(movement) = deserialize::<f32>(payload) {
                        input.movement[i] = movement;
                    }
                }
                tick_ready.0 = true;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Input plugin: local keyboard + gamepad -> send to relay
// ---------------------------------------------------------------------------

struct NetPongInputPlugin;

impl Plugin for NetPongInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaddleInput>().add_systems(
            Update,
            read_and_send_local_input.run_if(is_playing).run_if(need_to_send),
        );
    }
}

fn read_and_send_local_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    net: Res<NetSocket>,
    sim_tick: Res<SimulationTick>,
    mut need: ResMut<NeedToSendInput>,
) {
    // Keyboard input
    let up = keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp);
    let down =
        keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown);
    let keyboard_input = (up as i8 - down as i8) as f32;

    // Gamepad input (first connected gamepad)
    let gamepad_input = gamepads.iter().next().map_or(0.0, |gp| gp.left_stick().y);

    let combined = (keyboard_input + gamepad_input).clamp(-1.0, 1.0);

    let payload = serialize(&combined);
    let msg = serialize(&ClientMessage::Input {
        tick: sim_tick.0,
        payload,
    });
    let _ = net.socket.send_to(&msg, net.relay_addr);

    need.0 = false;
}

// ---------------------------------------------------------------------------
// Game plugin: deterministic simulation (lockstep-gated FixedUpdate)
// ---------------------------------------------------------------------------

struct NetPongGamePlugin;

impl Plugin for NetPongGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .init_resource::<BallResetCounter>()
            .add_systems(
                FixedUpdate,
                (
                    move_paddles,
                    move_ball,
                    ball_wall_bounce,
                    ball_paddle_bounce,
                    check_scoring,
                    post_tick_advance,
                )
                    .chain()
                    .run_if(is_playing)
                    .run_if(tick_is_ready),
            );
    }
}

fn move_paddles(
    input: Res<PaddleInput>,
    time: Res<Time>,
    mut paddles: Query<(&mut Transform, &Paddle)>,
) {
    let dt = time.delta_secs();
    let max_paddle_y = (ARENA_HEIGHT - PADDLE_HEIGHT) / 2.0;

    for (mut transform, paddle) in &mut paddles {
        let movement = input.movement[paddle.player_index];
        transform.translation.y += movement * PADDLE_SPEED * dt;
        transform.translation.y =
            transform.translation.y.clamp(-max_paddle_y, max_paddle_y);
    }
}

fn move_ball(
    time: Res<Time>,
    mut ball: Query<(&mut Transform, &Velocity), With<Ball>>,
) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut ball {
        transform.translation += velocity.0.extend(0.0) * dt;
    }
}

fn ball_wall_bounce(mut ball: Query<(&Transform, &mut Velocity), With<Ball>>) {
    let max_ball_y = (ARENA_HEIGHT - BALL_SIZE) / 2.0;

    for (transform, mut velocity) in &mut ball {
        let y = transform.translation.y;
        if (y >= max_ball_y && velocity.0.y > 0.0)
            || (y <= -max_ball_y && velocity.0.y < 0.0)
        {
            velocity.0.y = -velocity.0.y;
        }
    }
}

fn ball_paddle_bounce(
    mut ball_query: Query<(&Transform, &mut Velocity), With<Ball>>,
    paddle_query: Query<(&Transform, &Paddle), Without<Ball>>,
    input: Res<PaddleInput>,
) {
    let paddle_half_w = PADDLE_WIDTH / 2.0;
    let paddle_half_h = PADDLE_HEIGHT / 2.0;
    let ball_half = BALL_SIZE / 2.0;

    for (ball_transform, mut ball_velocity) in &mut ball_query {
        let ball_pos = ball_transform.translation;

        for (paddle_transform, paddle) in &paddle_query {
            let paddle_pos = paddle_transform.translation;

            let overlap_x =
                (ball_pos.x - paddle_pos.x).abs() < paddle_half_w + ball_half;
            let overlap_y =
                (ball_pos.y - paddle_pos.y).abs() < paddle_half_h + ball_half;

            if !overlap_x || !overlap_y {
                continue;
            }

            let ball_moving_toward_paddle = if paddle_pos.x < 0.0 {
                ball_velocity.0.x < 0.0
            } else {
                ball_velocity.0.x > 0.0
            };

            if !ball_moving_toward_paddle {
                continue;
            }

            ball_velocity.0.x = -ball_velocity.0.x;

            let paddle_movement = input.movement[paddle.player_index];
            ball_velocity.0.y +=
                paddle_movement * PADDLE_SPEED * PADDLE_HIT_ANGLE_FACTOR;

            let current_speed = ball_velocity.0.length();
            let new_speed = current_speed + BALL_SPEED_INCREASE;
            ball_velocity.0 = ball_velocity.0.normalize() * new_speed;
        }
    }
}

fn check_scoring(
    mut ball_query: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
    mut reset_counter: ResMut<BallResetCounter>,
) {
    let score_boundary_x = ARENA_WIDTH / 2.0 + BALL_SIZE;

    for (mut transform, mut velocity) in &mut ball_query {
        let x = transform.translation.x;

        let scoring_player = if x < -score_boundary_x {
            Some(1)
        } else if x > score_boundary_x {
            Some(0)
        } else {
            None
        };

        let Some(scorer) = scoring_player else {
            continue;
        };

        score.points[scorer] += 1;
        reset_counter.0 += 1;

        transform.translation = Vec3::ZERO;

        let direction_x = if scorer == 0 { -1.0 } else { 1.0 };
        let direction_y = if reset_counter.0.is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let direction = Vec2::new(direction_x, direction_y * 0.5).normalize();
        velocity.0 = direction * BALL_INITIAL_SPEED;
    }
}

fn post_tick_advance(
    mut sim_tick: ResMut<SimulationTick>,
    mut tick_ready: ResMut<TickReady>,
    mut need_send: ResMut<NeedToSendInput>,
) {
    sim_tick.0 += 1;
    tick_ready.0 = false;
    need_send.0 = true;
}

// ---------------------------------------------------------------------------
// Render plugin: sprites, score display, connection status
// ---------------------------------------------------------------------------

struct NetPongRenderPlugin;

impl Plugin for NetPongRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_pong)
            .add_systems(
                Update,
                (update_score_display, update_connection_status),
            );
    }
}

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct ConnectionStatusText;

const SCORE_FONT_SIZE: f32 = 48.0;
const SCORE_TOP_MARGIN: f32 = 20.0;
const BORDER_THICKNESS: f32 = 4.0;
const BORDER_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
const PADDLE_COLOR: Color = Color::WHITE;
const BALL_COLOR: Color = Color::WHITE;
const CENTER_LINE_DASH_COUNT: usize = 15;
const CENTER_LINE_DASH_WIDTH: f32 = 4.0;
const STATUS_FONT_SIZE: f32 = 32.0;

fn setup_pong(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Arena borders
    spawn_border(
        &mut commands,
        Vec3::new(0.0, ARENA_HEIGHT / 2.0, 0.0),
        ARENA_WIDTH,
        BORDER_THICKNESS,
    );
    spawn_border(
        &mut commands,
        Vec3::new(0.0, -ARENA_HEIGHT / 2.0, 0.0),
        ARENA_WIDTH,
        BORDER_THICKNESS,
    );
    spawn_border(
        &mut commands,
        Vec3::new(-ARENA_WIDTH / 2.0, 0.0, 0.0),
        BORDER_THICKNESS,
        ARENA_HEIGHT,
    );
    spawn_border(
        &mut commands,
        Vec3::new(ARENA_WIDTH / 2.0, 0.0, 0.0),
        BORDER_THICKNESS,
        ARENA_HEIGHT,
    );

    // Center line (dashed)
    let dash_spacing = ARENA_HEIGHT / CENTER_LINE_DASH_COUNT as f32;
    let dash_height = dash_spacing * 0.5;
    for i in 0..CENTER_LINE_DASH_COUNT {
        let y = -ARENA_HEIGHT / 2.0 + dash_spacing * (i as f32 + 0.5);
        commands.spawn((
            Sprite {
                color: BORDER_COLOR,
                custom_size: Some(Vec2::new(CENTER_LINE_DASH_WIDTH, dash_height)),
                ..default()
            },
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }

    // Paddles
    let left_paddle_x = -(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET);
    let right_paddle_x = ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET;
    spawn_paddle(&mut commands, left_paddle_x, 0);
    spawn_paddle(&mut commands, right_paddle_x, 1);

    // Ball
    let initial_direction = Vec2::new(1.0, 0.5).normalize();
    let initial_velocity = initial_direction * BALL_INITIAL_SPEED;
    commands.spawn((
        Ball,
        Velocity(initial_velocity),
        Sprite {
            color: BALL_COLOR,
            custom_size: Some(Vec2::splat(BALL_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Score text
    commands.spawn((
        ScoreText,
        Text::new("0  :  0"),
        TextFont::from_font_size(SCORE_FONT_SIZE),
        TextColor::WHITE,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(SCORE_TOP_MARGIN),
            left: Val::Percent(50.0),
            ..default()
        },
    ));

    // Connection status text (centered)
    commands.spawn((
        ConnectionStatusText,
        Text::new("Connecting to relay..."),
        TextFont::from_font_size(STATUS_FONT_SIZE),
        TextColor(Color::srgb(1.0, 1.0, 0.5)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(50.0),
            left: Val::Percent(50.0),
            ..default()
        },
    ));
}

fn spawn_border(commands: &mut Commands, position: Vec3, width: f32, height: f32) {
    commands.spawn((
        Sprite {
            color: BORDER_COLOR,
            custom_size: Some(Vec2::new(width, height)),
            ..default()
        },
        Transform::from_translation(position),
    ));
}

fn spawn_paddle(commands: &mut Commands, x: f32, player_index: usize) {
    commands.spawn((
        Paddle { player_index },
        Sprite {
            color: PADDLE_COLOR,
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(x, 0.0, 0.0),
    ));
}

fn update_score_display(
    score: Res<Score>,
    mut query: Query<&mut Text, With<ScoreText>>,
) {
    if !score.is_changed() {
        return;
    }
    for mut text in &mut query {
        **text = format!("{}  :  {}", score.points[0], score.points[1]);
    }
}

fn update_connection_status(
    state: Res<ConnectionState>,
    mut query: Query<(&mut Text, &mut Visibility), With<ConnectionStatusText>>,
) {
    if !state.is_changed() {
        return;
    }
    for (mut text, mut visibility) in &mut query {
        match *state {
            ConnectionState::Connecting => {
                **text = "Connecting to relay...".into();
                *visibility = Visibility::Visible;
            }
            ConnectionState::WaitingForOpponent => {
                **text = "Waiting for opponent...".into();
                *visibility = Visibility::Visible;
            }
            ConnectionState::Playing => {
                *visibility = Visibility::Hidden;
            }
        }
    }
}
