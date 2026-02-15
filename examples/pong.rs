//! Two-player Pong using gamepads.
//!
//! Run with: `cargo run --example pong`
//!
//! Connect two gamepads and use the left stick Y-axis to move paddles.
//! Unconnected paddles simply stay still.

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PongPlugin))
        .run();
}

// ---------------------------------------------------------------------------
// Top-level plugin: wires sub-plugins together
// ---------------------------------------------------------------------------

struct PongPlugin;

impl Plugin for PongPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PongInputPlugin, PongGamePlugin, PongRenderPlugin));
    }
}

// ---------------------------------------------------------------------------
// Arena and gameplay constants
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
// Input plugin: reads gamepads into paddle movement intent
// ---------------------------------------------------------------------------

struct PongInputPlugin;

impl Plugin for PongInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaddleInput>()
            .add_systems(Update, read_paddle_input);
    }
}

#[derive(Resource, Default)]
struct PaddleInput {
    movement: [f32; PLAYER_COUNT],
}

fn read_paddle_input(gamepads: Query<&Gamepad>, mut input: ResMut<PaddleInput>) {
    let mut gamepad_iter = gamepads.iter();

    for slot in &mut input.movement {
        let Some(gamepad) = gamepad_iter.next() else {
            *slot = 0.0;
            continue;
        };
        *slot = gamepad.left_stick().y;
    }
}

// ---------------------------------------------------------------------------
// Game plugin: ball physics, collisions, scoring
// ---------------------------------------------------------------------------

struct PongGamePlugin;

impl Plugin for PongGamePlugin {
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
                )
                    .chain(),
            );
    }
}

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
        transform.translation.y = transform.translation.y.clamp(-max_paddle_y, max_paddle_y);
    }
}

fn move_ball(time: Res<Time>, mut ball: Query<(&mut Transform, &Velocity), With<Ball>>) {
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

            let overlap_x = (ball_pos.x - paddle_pos.x).abs() < paddle_half_w + ball_half;
            let overlap_y = (ball_pos.y - paddle_pos.y).abs() < paddle_half_h + ball_half;

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
            ball_velocity.0.y += paddle_movement * PADDLE_SPEED * PADDLE_HIT_ANGLE_FACTOR;

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
        let direction_y = if reset_counter.0.is_multiple_of(2) { 1.0 } else { -1.0 };
        let direction = Vec2::new(direction_x, direction_y * 0.5).normalize();
        velocity.0 = direction * BALL_INITIAL_SPEED;
    }
}

// ---------------------------------------------------------------------------
// Render plugin: spawns sprites and score text, updates visuals
// ---------------------------------------------------------------------------

struct PongRenderPlugin;

impl Plugin for PongRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_pong)
            .add_systems(Update, update_score_display);
    }
}

#[derive(Component)]
struct ScoreText;

const SCORE_FONT_SIZE: f32 = 48.0;
const SCORE_TOP_MARGIN: f32 = 20.0;
const BORDER_THICKNESS: f32 = 4.0;
const BORDER_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
const PADDLE_COLOR: Color = Color::WHITE;
const BALL_COLOR: Color = Color::WHITE;
const CENTER_LINE_DASH_COUNT: usize = 15;
const CENTER_LINE_DASH_WIDTH: f32 = 4.0;

fn setup_pong(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Arena borders
    spawn_border(&mut commands, Vec3::new(0.0, ARENA_HEIGHT / 2.0, 0.0), ARENA_WIDTH, BORDER_THICKNESS);
    spawn_border(&mut commands, Vec3::new(0.0, -ARENA_HEIGHT / 2.0, 0.0), ARENA_WIDTH, BORDER_THICKNESS);
    spawn_border(&mut commands, Vec3::new(-ARENA_WIDTH / 2.0, 0.0, 0.0), BORDER_THICKNESS, ARENA_HEIGHT);
    spawn_border(&mut commands, Vec3::new(ARENA_WIDTH / 2.0, 0.0, 0.0), BORDER_THICKNESS, ARENA_HEIGHT);

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

fn update_score_display(score: Res<Score>, mut query: Query<&mut Text, With<ScoreText>>) {
    if !score.is_changed() {
        return;
    }
    for mut text in &mut query {
        **text = format!("{}  :  {}", score.points[0], score.points[1]);
    }
}
