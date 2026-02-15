//! Neon Noir Pong — a gaudy neon-glowing variant of two-player Pong.
//!
//! Run with: `cargo run --example neon_pong`
//!
//! Connect two gamepads and use the left stick Y-axis to move paddles.
//! Unconnected paddles simply stay still.
//!
//! Sound files must be generated once before first run:
//!   `cargo run --example generate_sounds`
//! The game works fine without them (just silent, with asset warnings).

use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, NeonPongPlugin))
        .run();
}

// ---------------------------------------------------------------------------
// Top-level plugin
// ---------------------------------------------------------------------------

struct NeonPongPlugin;

impl Plugin for NeonPongPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PongInputPlugin,
            NeonPongGamePlugin,
            NeonRenderPlugin,
            NeonEffectsPlugin,
            NeonAudioPlugin,
        ));
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
// Input plugin (identical to pong.rs)
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
// Cross-domain events (game -> effects/audio)
// ---------------------------------------------------------------------------

#[derive(Message)]
struct PaddleHitEvent {
    ball_position: Vec3,
}

#[derive(Message)]
struct WallBounceEvent {
    ball_position: Vec3,
}

#[derive(Message)]
struct ScoreEvent;

// ---------------------------------------------------------------------------
// Game plugin (pong.rs logic + event emission)
// ---------------------------------------------------------------------------

struct NeonPongGamePlugin;

impl Plugin for NeonPongGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .init_resource::<BallResetCounter>()
            .add_message::<PaddleHitEvent>()
            .add_message::<WallBounceEvent>()
            .add_message::<ScoreEvent>()
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

fn ball_wall_bounce(
    mut ball: Query<(&Transform, &mut Velocity), With<Ball>>,
    mut bounce_events: MessageWriter<WallBounceEvent>,
) {
    let max_ball_y = (ARENA_HEIGHT - BALL_SIZE) / 2.0;

    for (transform, mut velocity) in &mut ball {
        let y = transform.translation.y;
        if (y >= max_ball_y && velocity.0.y > 0.0)
            || (y <= -max_ball_y && velocity.0.y < 0.0)
        {
            velocity.0.y = -velocity.0.y;
            bounce_events.write(WallBounceEvent {
                ball_position: transform.translation,
            });
        }
    }
}

fn ball_paddle_bounce(
    mut ball_query: Query<(&Transform, &mut Velocity), With<Ball>>,
    paddle_query: Query<(&Transform, &Paddle), Without<Ball>>,
    input: Res<PaddleInput>,
    mut hit_events: MessageWriter<PaddleHitEvent>,
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

            hit_events.write(PaddleHitEvent {
                ball_position: ball_pos,
            });
        }
    }
}

fn check_scoring(
    mut ball_query: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
    mut reset_counter: ResMut<BallResetCounter>,
    mut score_events: MessageWriter<ScoreEvent>,
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

        score_events.write(ScoreEvent);
    }
}

// ---------------------------------------------------------------------------
// Neon render plugin: HDR camera, bloom, neon-colored sprites
// ---------------------------------------------------------------------------

struct NeonRenderPlugin;

impl Plugin for NeonRenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::linear_rgb(0.02, 0.01, 0.05)))
            .add_systems(Startup, setup_neon_pong)
            .add_systems(Update, update_score_display);
    }
}

#[derive(Component)]
struct ScoreText;

const SCORE_FONT_SIZE: f32 = 48.0;
const SCORE_TOP_MARGIN: f32 = 20.0;
const BORDER_THICKNESS: f32 = 4.0;

// Neon HDR colors (values > 1.0 trigger bloom)
const COLOR_CYAN_PADDLE: Color = Color::linear_rgb(0.0, 4.0, 4.0);
const COLOR_MAGENTA_PADDLE: Color = Color::linear_rgb(4.0, 0.0, 4.0);
const COLOR_HOT_PINK_BALL: Color = Color::linear_rgb(5.0, 0.5, 2.0);
const COLOR_DIM_BLUE_BORDER: Color = Color::linear_rgb(0.3, 0.1, 0.8);
const COLOR_DIM_PURPLE_CENTER: Color = Color::linear_rgb(0.2, 0.1, 0.5);
const COLOR_NEON_GREEN_SCORE: Color = Color::linear_rgb(0.0, 3.0, 0.0);
const COLOR_GRID: Color = Color::linear_rgba(0.1, 0.05, 0.2, 0.3);

const CENTER_LINE_DASH_COUNT: usize = 15;
const CENTER_LINE_DASH_WIDTH: f32 = 4.0;
const GRID_SPACING: f32 = 50.0;
const GRID_LINE_THICKNESS: f32 = 1.0;

fn setup_neon_pong(mut commands: Commands) {
    // HDR camera with bloom
    commands.spawn((
        Camera2d,
        Bloom::OLD_SCHOOL,
        Tonemapping::TonyMcMapface,
        DebandDither::Enabled,
    ));

    // Background grid
    spawn_background_grid(&mut commands);

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
                color: COLOR_DIM_PURPLE_CENTER,
                custom_size: Some(Vec2::new(CENTER_LINE_DASH_WIDTH, dash_height)),
                ..default()
            },
            Transform::from_xyz(0.0, y, 0.1),
        ));
    }

    // Paddles
    let left_paddle_x = -(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET);
    let right_paddle_x = ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET;

    spawn_neon_paddle(&mut commands, left_paddle_x, 0, COLOR_CYAN_PADDLE);
    spawn_neon_paddle(&mut commands, right_paddle_x, 1, COLOR_MAGENTA_PADDLE);

    // Ball
    let initial_direction = Vec2::new(1.0, 0.5).normalize();
    let initial_velocity = initial_direction * BALL_INITIAL_SPEED;

    commands.spawn((
        Ball,
        Velocity(initial_velocity),
        Sprite {
            color: COLOR_HOT_PINK_BALL,
            custom_size: Some(Vec2::splat(BALL_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    // Score text
    commands.spawn((
        ScoreText,
        Text::new("0  :  0"),
        TextFont::from_font_size(SCORE_FONT_SIZE),
        TextColor(COLOR_NEON_GREEN_SCORE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(SCORE_TOP_MARGIN),
            left: Val::Percent(50.0),
            ..default()
        },
    ));
}

fn spawn_background_grid(commands: &mut Commands) {
    let half_w = ARENA_WIDTH / 2.0;
    let half_h = ARENA_HEIGHT / 2.0;
    let z_depth = -0.1;

    // Vertical lines
    let mut x = -half_w;
    while x <= half_w {
        commands.spawn((
            Sprite {
                color: COLOR_GRID,
                custom_size: Some(Vec2::new(GRID_LINE_THICKNESS, ARENA_HEIGHT)),
                ..default()
            },
            Transform::from_xyz(x, 0.0, z_depth),
        ));
        x += GRID_SPACING;
    }

    // Horizontal lines
    let mut y = -half_h;
    while y <= half_h {
        commands.spawn((
            Sprite {
                color: COLOR_GRID,
                custom_size: Some(Vec2::new(ARENA_WIDTH, GRID_LINE_THICKNESS)),
                ..default()
            },
            Transform::from_xyz(0.0, y, z_depth),
        ));
        y += GRID_SPACING;
    }
}

fn spawn_border(commands: &mut Commands, position: Vec3, width: f32, height: f32) {
    commands.spawn((
        Sprite {
            color: COLOR_DIM_BLUE_BORDER,
            custom_size: Some(Vec2::new(width, height)),
            ..default()
        },
        Transform::from_translation(position),
    ));
}

fn spawn_neon_paddle(commands: &mut Commands, x: f32, player_index: usize, color: Color) {
    commands.spawn((
        Paddle { player_index },
        PaddleFlash {
            timer: Timer::from_seconds(0.1, TimerMode::Once),
            base_color: color,
            flash_color: Color::linear_rgb(10.0, 10.0, 10.0),
        },
        Sprite {
            color,
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(x, 0.0, 0.5),
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

// ---------------------------------------------------------------------------
// Effects plugin: trails, particles, screen shake, paddle flash
// ---------------------------------------------------------------------------

struct NeonEffectsPlugin;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
enum NeonEffectSet {
    Spawn,
    Update,
    Camera,
}

impl Plugin for NeonEffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenShake>()
            .init_resource::<FrameCount>()
            .configure_sets(
                Update,
                (
                    NeonEffectSet::Spawn,
                    NeonEffectSet::Update.after(NeonEffectSet::Spawn),
                    NeonEffectSet::Camera.after(NeonEffectSet::Update),
                ),
            )
            .add_systems(
                Update,
                (
                    spawn_ball_trail,
                    handle_paddle_hit_effects,
                    handle_wall_bounce_effects,
                    handle_score_effects,
                )
                    .in_set(NeonEffectSet::Spawn),
            )
            .add_systems(
                Update,
                (update_paddle_flash, update_trails, update_particles)
                    .in_set(NeonEffectSet::Update),
            )
            .add_systems(
                Update,
                apply_screen_shake.in_set(NeonEffectSet::Camera),
            );
    }
}

#[derive(Component)]
struct Trail {
    lifetime: Timer,
}

#[derive(Component)]
struct Particle {
    lifetime: Timer,
    initial_size: f32,
}

#[derive(Component)]
struct PaddleFlash {
    timer: Timer,
    base_color: Color,
    flash_color: Color,
}

#[derive(Resource, Default)]
struct ScreenShake {
    trauma: f32,
}

#[derive(Resource, Default)]
struct FrameCount(u32);

const TRAIL_LIFETIME_SECS: f32 = 0.3;
const TRAIL_SIZE_FACTOR: f32 = 0.8;
const TRAIL_ALPHA: f32 = 0.5;

const PARTICLE_LIFETIME_SECS: f32 = 0.4;
const PARTICLE_SIZE: f32 = 5.0;
const PARTICLE_SPEED: f32 = 300.0;

const PADDLE_HIT_PARTICLE_COUNT: usize = 10;
const WALL_BOUNCE_PARTICLE_COUNT: usize = 5;
const SCORE_PARTICLE_COUNT: usize = 18;

const PADDLE_HIT_TRAUMA: f32 = 0.3;
const SCORE_TRAUMA: f32 = 0.5;

const MAX_SHAKE_OFFSET: f32 = 8.0;
const TRAUMA_DECAY_RATE: f32 = 3.0;

const FLASH_DURATION_SECS: f32 = 0.1;

// --- Ball trail ---

fn spawn_ball_trail(
    mut commands: Commands,
    ball_query: Query<(&Transform, &Sprite), With<Ball>>,
) {
    for (transform, sprite) in &ball_query {
        let trail_size = BALL_SIZE * TRAIL_SIZE_FACTOR;
        let trail_color = with_alpha(sprite.color, TRAIL_ALPHA);

        commands.spawn((
            Trail {
                lifetime: Timer::from_seconds(TRAIL_LIFETIME_SECS, TimerMode::Once),
            },
            Sprite {
                color: trail_color,
                custom_size: Some(Vec2::splat(trail_size)),
                ..default()
            },
            Transform::from_translation(transform.translation.with_z(0.5)),
        ));
    }
}

fn update_trails(
    mut commands: Commands,
    time: Res<Time>,
    mut trails: Query<(Entity, &mut Trail, &mut Sprite, &mut Transform)>,
) {
    for (entity, mut trail, mut sprite, mut transform) in &mut trails {
        trail.lifetime.tick(time.delta());

        let progress = trail.lifetime.fraction();
        let remaining = 1.0 - progress;

        if let Some(size) = &mut sprite.custom_size {
            let base = BALL_SIZE * TRAIL_SIZE_FACTOR;
            *size = Vec2::splat(base * remaining);
        }
        sprite.color = with_alpha(sprite.color, TRAIL_ALPHA * remaining);
        transform.scale = Vec3::splat(remaining.max(0.01));

        if trail.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

// --- Particle bursts ---

fn handle_paddle_hit_effects(
    mut commands: Commands,
    mut events: MessageReader<PaddleHitEvent>,
    mut shake: ResMut<ScreenShake>,
    paddles: Query<(&Transform, &Paddle)>,
    mut flash_query: Query<(&mut PaddleFlash, &Paddle)>,
) {
    for event in events.read() {
        shake.trauma = (shake.trauma + PADDLE_HIT_TRAUMA).min(1.0);

        // Determine spray direction (away from the paddle that was hit)
        let spray_center_angle = closest_paddle_spray_angle(&paddles, event.ball_position);

        spawn_particles(
            &mut commands,
            event.ball_position,
            PADDLE_HIT_PARTICLE_COUNT,
            spray_center_angle,
            std::f32::consts::PI,
            COLOR_HOT_PINK_BALL,
        );

        // Flash the closest paddle
        for (mut flash, paddle) in &mut flash_query {
            let paddle_x = if paddle.player_index == 0 {
                -(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET)
            } else {
                ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET
            };
            let distance = (event.ball_position.x - paddle_x).abs();
            let close_threshold = PADDLE_X_OFFSET + PADDLE_WIDTH;
            if distance < close_threshold {
                flash.timer = Timer::from_seconds(FLASH_DURATION_SECS, TimerMode::Once);
            }
        }
    }
}

fn closest_paddle_spray_angle(
    paddles: &Query<(&Transform, &Paddle)>,
    ball_position: Vec3,
) -> f32 {
    let mut closest_paddle_x = 0.0_f32;
    let mut closest_distance = f32::MAX;

    for (transform, _) in paddles.iter() {
        let distance = (ball_position.x - transform.translation.x).abs();
        if distance < closest_distance {
            closest_distance = distance;
            closest_paddle_x = transform.translation.x;
        }
    }

    // Spray away from the paddle
    if ball_position.x > closest_paddle_x {
        0.0 // spray right
    } else {
        std::f32::consts::PI // spray left
    }
}

fn handle_wall_bounce_effects(
    mut commands: Commands,
    mut events: MessageReader<WallBounceEvent>,
) {
    for event in events.read() {
        // Spray downward if at top wall, upward if at bottom wall
        let spray_angle = if event.ball_position.y > 0.0 {
            -std::f32::consts::FRAC_PI_2 // spray downward
        } else {
            std::f32::consts::FRAC_PI_2 // spray upward
        };

        spawn_particles(
            &mut commands,
            event.ball_position,
            WALL_BOUNCE_PARTICLE_COUNT,
            spray_angle,
            std::f32::consts::PI,
            COLOR_DIM_BLUE_BORDER,
        );
    }
}

fn handle_score_effects(
    mut commands: Commands,
    mut events: MessageReader<ScoreEvent>,
    mut shake: ResMut<ScreenShake>,
) {
    for _event in events.read() {
        shake.trauma = (shake.trauma + SCORE_TRAUMA).min(1.0);

        let full_circle = std::f32::consts::TAU;
        spawn_particles(
            &mut commands,
            Vec3::ZERO,
            SCORE_PARTICLE_COUNT,
            0.0,
            full_circle,
            COLOR_NEON_GREEN_SCORE,
        );
    }
}

fn spawn_particles(
    commands: &mut Commands,
    position: Vec3,
    count: usize,
    center_angle: f32,
    spread: f32,
    color: Color,
) {
    let half_spread = spread / 2.0;

    for i in 0..count {
        let fraction = if count <= 1 {
            0.5
        } else {
            i as f32 / (count - 1) as f32
        };
        let angle = center_angle - half_spread + spread * fraction;
        let direction = Vec2::new(angle.cos(), angle.sin());
        let velocity = direction * PARTICLE_SPEED;

        commands.spawn((
            Particle {
                lifetime: Timer::from_seconds(PARTICLE_LIFETIME_SECS, TimerMode::Once),
                initial_size: PARTICLE_SIZE,
            },
            Velocity(velocity),
            Sprite {
                color,
                custom_size: Some(Vec2::splat(PARTICLE_SIZE)),
                ..default()
            },
            Transform::from_translation(position.with_z(2.0)),
        ));
    }
}

fn update_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &Velocity, &mut Sprite, &mut Transform)>,
) {
    let dt = time.delta_secs();

    for (entity, mut particle, velocity, mut sprite, mut transform) in &mut particles {
        particle.lifetime.tick(time.delta());
        transform.translation += velocity.0.extend(0.0) * dt;

        let remaining = 1.0 - particle.lifetime.fraction();
        let size = particle.initial_size * remaining;
        sprite.custom_size = Some(Vec2::splat(size.max(0.1)));
        sprite.color = with_alpha(sprite.color, remaining);

        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

// --- Paddle flash ---

fn update_paddle_flash(time: Res<Time>, mut query: Query<(&mut PaddleFlash, &mut Sprite)>) {
    for (mut flash, mut sprite) in &mut query {
        flash.timer.tick(time.delta());

        if flash.timer.is_finished() {
            sprite.color = flash.base_color;
        } else {
            let progress = flash.timer.fraction();
            sprite.color = lerp_color(flash.flash_color, flash.base_color, progress);
        }
    }
}

// --- Screen shake ---

fn apply_screen_shake(
    time: Res<Time>,
    mut shake: ResMut<ScreenShake>,
    mut frame_count: ResMut<FrameCount>,
    mut camera: Query<&mut Transform, With<Camera2d>>,
) {
    frame_count.0 = frame_count.0.wrapping_add(1);
    let dt = time.delta_secs();

    shake.trauma = (shake.trauma - TRAUMA_DECAY_RATE * dt).max(0.0);

    for mut transform in &mut camera {
        if shake.trauma > 0.001 {
            let intensity = shake.trauma * shake.trauma;
            let frame = frame_count.0 as f32;

            // Deterministic pseudo-random offset using sin of frame * primes
            let offset_x = (frame * 97.0).sin() * MAX_SHAKE_OFFSET * intensity;
            let offset_y = (frame * 53.0).sin() * MAX_SHAKE_OFFSET * intensity;

            transform.translation.x = offset_x;
            transform.translation.y = offset_y;
        } else {
            transform.translation.x = 0.0;
            transform.translation.y = 0.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Audio plugin: plays sounds on events
// ---------------------------------------------------------------------------

struct NeonAudioPlugin;

impl Plugin for NeonAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_sound_assets)
            .add_systems(
                Update,
                (play_hit_sound, play_bounce_sound, play_score_sound),
            );
    }
}

#[derive(Resource)]
struct SoundAssets {
    hit: Handle<AudioSource>,
    bounce: Handle<AudioSource>,
    score: Handle<AudioSource>,
}

fn load_sound_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(SoundAssets {
        hit: asset_server.load("sounds/hit.wav"),
        bounce: asset_server.load("sounds/bounce.wav"),
        score: asset_server.load("sounds/score.wav"),
    });
}

fn play_hit_sound(
    mut commands: Commands,
    mut events: MessageReader<PaddleHitEvent>,
    sounds: Res<SoundAssets>,
) {
    for _event in events.read() {
        commands.spawn((
            AudioPlayer::new(sounds.hit.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

fn play_bounce_sound(
    mut commands: Commands,
    mut events: MessageReader<WallBounceEvent>,
    sounds: Res<SoundAssets>,
) {
    for _event in events.read() {
        commands.spawn((
            AudioPlayer::new(sounds.bounce.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

fn play_score_sound(
    mut commands: Commands,
    mut events: MessageReader<ScoreEvent>,
    sounds: Res<SoundAssets>,
) {
    for _event in events.read() {
        commands.spawn((
            AudioPlayer::new(sounds.score.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

fn with_alpha(color: Color, alpha: f32) -> Color {
    let linear = LinearRgba::from(color);
    Color::linear_rgba(linear.red, linear.green, linear.blue, alpha)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a_lin = LinearRgba::from(a);
    let b_lin = LinearRgba::from(b);
    let t_clamped = t.clamp(0.0, 1.0);

    Color::linear_rgba(
        a_lin.red + (b_lin.red - a_lin.red) * t_clamped,
        a_lin.green + (b_lin.green - a_lin.green) * t_clamped,
        a_lin.blue + (b_lin.blue - a_lin.blue) * t_clamped,
        a_lin.alpha + (b_lin.alpha - a_lin.alpha) * t_clamped,
    )
}
