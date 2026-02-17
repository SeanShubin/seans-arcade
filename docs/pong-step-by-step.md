# Pong Step-by-Step

Build `examples/pong.rs` from scratch, one small step at a time.

Each step tells you:
1. **What you'll see** — the observable change
2. **Code** — what to type
3. **What you learned** — the Bevy concept this step teaches

Run `cargo run --example pong` after every step.

---

## Phase 1: Get Something on Screen

### Step 1: Empty window

**What you'll see:** A dark window opens.

Create `examples/pong.rs`:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}
```

**What you learned:** `App` is Bevy's entry point. `DefaultPlugins` bundles a window, renderer, and input.

### Step 2: A white square

**What you'll see:** A white square appears at the center of the window.

Replace `main` and add `setup`:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite {
            custom_size: Some(Vec2::splat(100.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
```

**What you learned:** `Startup` systems run once at launch. `Commands` spawns entities. `Camera2d` is required to render anything. A `Sprite` + `Transform` produces a colored rectangle.

### Step 3: Paddle shape

**What you'll see:** The square becomes a tall, thin rectangle.

Add above `main`:

```rust
const PADDLE_WIDTH: f32 = 15.0;
const PADDLE_HEIGHT: f32 = 80.0;
```

Change the sprite's `custom_size` in `setup`:

```rust
custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
```

**What you learned:** `custom_size` controls sprite dimensions in pixels. Constants keep magic numbers out of your code.

### Step 4: Left paddle position

**What you'll see:** The rectangle moves to the left side of the window.

Add constants:

```rust
const ARENA_WIDTH: f32 = 800.0;
const PADDLE_X_OFFSET: f32 = 30.0;
```

Change the Transform in `setup`:

```rust
Transform::from_xyz(-(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET), 0.0, 0.0),
```

**What you learned:** Bevy's 2D origin is the window center. Negative X is left, positive X is right.

### Step 5: Right paddle

**What you'll see:** A second paddle appears on the right side.

Add after the left paddle spawn in `setup`:

```rust
    // Right paddle
    commands.spawn((
        Sprite {
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET, 0.0, 0.0),
    ));
```

**What you learned:** Each `commands.spawn(...)` creates a separate entity. Entities with the same component types are independent.

### Step 6: Ball

**What you'll see:** A small white square appears at the center between the paddles.

Add constant:

```rust
const BALL_SIZE: f32 = 12.0;
```

Add after the paddles in `setup`:

```rust
    // Ball
    commands.spawn((
        Sprite {
            custom_size: Some(Vec2::splat(BALL_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
```

**What you learned:** `Vec2::splat(n)` is shorthand for `Vec2::new(n, n)`.

---

## Phase 2: Build the Arena

### Step 7: Top and bottom borders

**What you'll see:** Thin grey lines appear at the top and bottom edges of the play area.

Add constants:

```rust
const ARENA_HEIGHT: f32 = 500.0;
const BORDER_THICKNESS: f32 = 4.0;
const BORDER_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
```

Add a helper function after `setup`:

```rust
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
```

Call it from `setup`, before the paddles:

```rust
    // Top and bottom borders
    spawn_border(&mut commands, Vec3::new(0.0, ARENA_HEIGHT / 2.0, 0.0), ARENA_WIDTH, BORDER_THICKNESS);
    spawn_border(&mut commands, Vec3::new(0.0, -ARENA_HEIGHT / 2.0, 0.0), ARENA_WIDTH, BORDER_THICKNESS);
```

**What you learned:** Helper functions that take `&mut Commands` let you reuse spawn patterns. `Color::srgb(r, g, b)` creates a color with 0.0-1.0 channels.

### Step 8: Side borders

**What you'll see:** Vertical grey lines complete the arena rectangle.

Add to `setup` after the top/bottom borders:

```rust
    // Left and right borders
    spawn_border(&mut commands, Vec3::new(-ARENA_WIDTH / 2.0, 0.0, 0.0), BORDER_THICKNESS, ARENA_HEIGHT);
    spawn_border(&mut commands, Vec3::new(ARENA_WIDTH / 2.0, 0.0, 0.0), BORDER_THICKNESS, ARENA_HEIGHT);
```

**What you learned:** Same helper, different arguments. Swap width and height for vertical borders.

### Step 9: Center dashed line

**What you'll see:** A dashed vertical line divides the arena into two halves.

Add constants:

```rust
const CENTER_LINE_DASH_COUNT: usize = 15;
const CENTER_LINE_DASH_WIDTH: f32 = 4.0;
```

Add to `setup`, after the borders and before the paddles:

```rust
    // Center line
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
```

**What you learned:** Entities can be spawned in a loop. There's no "dashed line" primitive — just many small rectangles.

---

## Phase 3: Movement

### Step 10: Ball moves

**What you'll see:** The ball launches toward the upper-right, passes through the paddle, and disappears off-screen.

Add above `main`:

```rust
#[derive(Component)]
struct Ball;

#[derive(Component)]
struct Velocity(Vec2);

const BALL_INITIAL_SPEED: f32 = 300.0;
```

Replace the ball spawn in `setup`:

```rust
    // Ball
    let initial_direction = Vec2::new(1.0, 0.5).normalize();
    commands.spawn((
        Ball,
        Velocity(initial_direction * BALL_INITIAL_SPEED),
        Sprite {
            custom_size: Some(Vec2::splat(BALL_SIZE)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
```

Add a new system:

```rust
fn move_ball(time: Res<Time>, mut ball: Query<(&mut Transform, &Velocity), With<Ball>>) {
    let dt = time.delta_secs();
    for (mut transform, velocity) in &mut ball {
        transform.translation += velocity.0.extend(0.0) * dt;
    }
}
```

Register it in `main`:

```rust
        .add_systems(FixedUpdate, move_ball)
```

**What you learned:** `#[derive(Component)]` makes a struct attachable to entities. `Ball` is a *marker* (no data) for filtering queries. `Velocity(Vec2)` holds data. `Query` finds entities with matching components. `With<Ball>` filters without borrowing. `Res<Time>` provides frame timing. `FixedUpdate` runs at a fixed timestep, ideal for physics.

### Step 11: Wall bounce

**What you'll see:** The ball bounces off the top and bottom borders instead of flying through them.

Add a new system:

```rust
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
```

Update the FixedUpdate registration to chain both systems:

```rust
        .add_systems(FixedUpdate, (move_ball, ball_wall_bounce).chain())
```

**What you learned:** `.chain()` forces systems to run in declared order. The bounce checks both position AND velocity direction to prevent the ball from getting stuck.

### Step 12: Paddle bounce

**What you'll see:** The ball bounces off both paddles and rattles around the arena endlessly.

Add above `main`:

```rust
#[derive(Component)]
struct Paddle {
    player_index: usize,
}
```

Update both paddle spawns in `setup` to include the component. Left paddle:

```rust
    // Left paddle
    commands.spawn((
        Paddle { player_index: 0 },
        Sprite {
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(-(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET), 0.0, 0.0),
    ));
```

Right paddle:

```rust
    // Right paddle
    commands.spawn((
        Paddle { player_index: 1 },
        Sprite {
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET, 0.0, 0.0),
    ));
```

Add a new system:

```rust
fn ball_paddle_bounce(
    mut ball_query: Query<(&Transform, &mut Velocity), With<Ball>>,
    paddle_query: Query<&Transform, With<Paddle>>,
) {
    let paddle_half_w = PADDLE_WIDTH / 2.0;
    let paddle_half_h = PADDLE_HEIGHT / 2.0;
    let ball_half = BALL_SIZE / 2.0;

    for (ball_transform, mut ball_velocity) in &mut ball_query {
        let ball_pos = ball_transform.translation;

        for paddle_transform in &paddle_query {
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
        }
    }
}
```

Add it to the chain:

```rust
        .add_systems(FixedUpdate, (move_ball, ball_wall_bounce, ball_paddle_bounce).chain())
```

**What you learned:** AABB collision: two rectangles overlap when they overlap on *both* axes. The "moving toward" check prevents the ball from bouncing multiple times while overlapping a paddle.

---

## Phase 4: Input

### Step 13: Paddles respond to gamepads

**What you'll see:** Connect a gamepad — the left stick moves the left paddle up and down. A second gamepad controls the right paddle.

Add constants:

```rust
const PADDLE_SPEED: f32 = 400.0;
const PLAYER_COUNT: usize = 2;
```

Add above `main`:

```rust
#[derive(Resource, Default)]
struct PaddleInput {
    movement: [f32; PLAYER_COUNT],
}
```

Add two new systems:

```rust
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

fn move_paddles(
    input: Res<PaddleInput>,
    time: Res<Time>,
    mut paddles: Query<(&mut Transform, &Paddle)>,
) {
    let dt = time.delta_secs();

    for (mut transform, paddle) in &mut paddles {
        let movement = input.movement[paddle.player_index];
        transform.translation.y += movement * PADDLE_SPEED * dt;
    }
}
```

Register in `main`:

```rust
        .init_resource::<PaddleInput>()
        .add_systems(Update, read_paddle_input)
```

And add `move_paddles` to the front of the FixedUpdate chain:

```rust
        .add_systems(FixedUpdate, (move_paddles, move_ball, ball_wall_bounce, ball_paddle_bounce).chain())
```

**What you learned:** `#[derive(Resource)]` creates shared state accessible from any system. `init_resource` inserts a `Default` instance. `Res<T>` borrows immutably; `ResMut<T>` borrows mutably. Input is read in `Update` (every frame) and consumed in `FixedUpdate` (fixed timestep).

### Step 14: Clamp paddle movement

**What you'll see:** Paddles stop at the top and bottom borders instead of sliding off-screen.

Replace `move_paddles`:

```rust
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
```

**What you learned:** `.clamp(min, max)` keeps a value within bounds. The limit accounts for paddle height so the *edge* stops at the border, not the center.

---

## Phase 5: Scoring

### Step 15: Ball resets when it exits

**What you'll see:** When the ball passes a paddle and exits the arena, it reappears at center and launches again.

Add a new system:

```rust
fn check_scoring(
    mut ball_query: Query<(&mut Transform, &mut Velocity), With<Ball>>,
) {
    let score_boundary_x = ARENA_WIDTH / 2.0 + BALL_SIZE;

    for (mut transform, mut velocity) in &mut ball_query {
        let x = transform.translation.x;

        if x.abs() < score_boundary_x {
            continue;
        }

        transform.translation = Vec3::ZERO;

        let direction_x = if x < 0.0 { 1.0 } else { -1.0 };
        let direction = Vec2::new(direction_x, 0.5).normalize();
        velocity.0 = direction * BALL_INITIAL_SPEED;
    }
}
```

Add to the FixedUpdate chain:

```rust
        .add_systems(FixedUpdate, (move_paddles, move_ball, ball_wall_bounce, ball_paddle_bounce, check_scoring).chain())
```

**What you learned:** The boundary is slightly beyond the arena so the ball visually exits before resetting. The ball launches away from the side it exited.

### Step 16: Score display

**What you'll see:** "0  :  0" appears at the top center. Numbers increment when the ball exits.

Add above `main`:

```rust
#[derive(Resource, Default)]
struct Score {
    points: [u32; PLAYER_COUNT],
}

#[derive(Component)]
struct ScoreText;

const SCORE_FONT_SIZE: f32 = 48.0;
const SCORE_TOP_MARGIN: f32 = 20.0;
```

Register the resource in `main`:

```rust
        .init_resource::<Score>()
```

Spawn score text at the end of `setup`:

```rust
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
```

Replace `check_scoring` to track score:

```rust
fn check_scoring(
    mut ball_query: Query<(&mut Transform, &mut Velocity), With<Ball>>,
    mut score: ResMut<Score>,
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

        transform.translation = Vec3::ZERO;

        let direction_x = if scorer == 0 { -1.0 } else { 1.0 };
        let direction = Vec2::new(direction_x, 0.5).normalize();
        velocity.0 = direction * BALL_INITIAL_SPEED;
    }
}
```

Add a display update system:

```rust
fn update_score_display(score: Res<Score>, mut query: Query<&mut Text, With<ScoreText>>) {
    if !score.is_changed() {
        return;
    }
    for mut text in &mut query {
        **text = format!("{}  :  {}", score.points[0], score.points[1]);
    }
}
```

Register it in `main` alongside `read_paddle_input`:

```rust
        .add_systems(Update, (read_paddle_input, update_score_display))
```

**What you learned:** UI text uses `Text` + `Node` for layout. `is_changed()` skips work when the resource hasn't been modified. `**text` dereferences through `Mut<Text>` and the `Text` newtype to reach the inner `String`.

### Step 17: Alternate reset direction

**What you'll see:** After each score, the ball launches at a different vertical angle — alternating up and down.

Add above `main`:

```rust
#[derive(Resource, Default)]
struct BallResetCounter(u32);
```

Register in `main`:

```rust
        .init_resource::<BallResetCounter>()
```

Update `check_scoring` — add the parameter and change the reset logic:

```rust
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
```

**What you learned:** Small state counters make behavior feel less robotic. `is_multiple_of(2)` is a readable way to alternate.

---

## Phase 6: Polish

### Step 18: Ball speeds up on paddle hit

**What you'll see:** The ball gets slightly faster each time it hits a paddle. Rallies become increasingly frantic.

Add constant:

```rust
const BALL_SPEED_INCREASE: f32 = 25.0;
```

In `ball_paddle_bounce`, add after `ball_velocity.0.x = -ball_velocity.0.x;`:

```rust
            let current_speed = ball_velocity.0.length();
            let new_speed = current_speed + BALL_SPEED_INCREASE;
            ball_velocity.0 = ball_velocity.0.normalize() * new_speed;
```

**What you learned:** Normalize-then-scale changes speed without changing direction. Escalating speed creates natural tension in each rally.

### Step 19: Paddle steers the ball

**What you'll see:** Moving a paddle while hitting the ball changes the ball's vertical angle. Skilled players can aim their shots.

Add constant:

```rust
const PADDLE_HIT_ANGLE_FACTOR: f32 = 0.5;
```

Replace `ball_paddle_bounce` — the signature changes to include `Paddle` data and `PaddleInput`:

```rust
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
```

**What you learned:** `Without<Ball>` tells Bevy the paddle query will never match the same entity as the ball query, proving the queries are disjoint. Mixing input with physics creates emergent gameplay.

### Step 20: Organize into plugins

**What you'll see:** The game behaves identically. The code is now organized into focused plugins.

Replace your `main`:

```rust
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PongPlugin))
        .run();
}
```

Add the plugin structs. Each plugin owns its resources, systems, and schedule registrations:

```rust
struct PongPlugin;

impl Plugin for PongPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((PongInputPlugin, PongGamePlugin, PongRenderPlugin));
    }
}

struct PongInputPlugin;

impl Plugin for PongInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PaddleInput>()
            .add_systems(Update, read_paddle_input);
    }
}

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

struct PongRenderPlugin;

impl Plugin for PongRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_pong)
            .add_systems(Update, update_score_display);
    }
}
```

Rename `setup` to `setup_pong`. Remove all `.init_resource` and `.add_systems` calls from the old `main` — the plugins handle everything now.

Extract a `spawn_paddle` helper to reduce duplication:

```rust
fn spawn_paddle(commands: &mut Commands, x: f32, player_index: usize) {
    commands.spawn((
        Paddle { player_index },
        Sprite {
            color: Color::WHITE,
            custom_size: Some(Vec2::new(PADDLE_WIDTH, PADDLE_HEIGHT)),
            ..default()
        },
        Transform::from_xyz(x, 0.0, 0.0),
    ));
}
```

Replace the two inline paddle spawns in `setup_pong`:

```rust
    let left_x = -(ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET);
    let right_x = ARENA_WIDTH / 2.0 - PADDLE_X_OFFSET;

    spawn_paddle(&mut commands, left_x, 0);
    spawn_paddle(&mut commands, right_x, 1);
```

Add the doc comment at the top of the file:

```rust
//! Two-player Pong using gamepads.
//!
//! Run with: `cargo run --example pong`
//!
//! Connect two gamepads and use the left stick Y-axis to move paddles.
//! Unconnected paddles simply stay still.
```

**What you learned:** Plugins group related resources and systems into self-contained units. A top-level plugin composes sub-plugins. This is Bevy's primary organizational pattern — as a game grows, each feature becomes its own plugin.

---

## Done

Your `examples/pong.rs` should now match the final version in the repository. Compare your code to verify.
