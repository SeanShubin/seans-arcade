//! Battle Arena prototype.
//!
//! Twin-stick 2D arena with melee and ranged enemies, dash, block, charge,
//! and passive/active attacks. See `docs/games/battle-arena/design.md`.
//!
//! Controls (gamepad):
//!   Left stick    — move
//!   Right stick   — aim direction
//!   Right trigger — active attack (medium range projectile)
//!   Left trigger  — block (hold)
//!   Left bumper   — dash (hold)
//!   A button      — charge (tap to target, hold to charge, release to fire)
//!
//! Run with: `cargo run --example battle_arena`

use bevy::prelude::*;
use rand::{Rng, RngExt};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ARENA_W: f32 = 1200.0;
const ARENA_H: f32 = 800.0;
const ARENA_WALL_THICKNESS: f32 = 12.0;

const PLAYER_SPEED: f32 = 200.0;
const PLAYER_RADIUS: f32 = 16.0;
const PLAYER_MELEE_RANGE: f32 = 50.0;
const PLAYER_MELEE_DAMAGE: f32 = 10.0;
const PLAYER_MELEE_COOLDOWN: f32 = 0.5;
const PLAYER_ACTIVE_SPEED: f32 = 500.0;
const PLAYER_ACTIVE_RANGE: f32 = 300.0;
const PLAYER_ACTIVE_DAMAGE: f32 = 20.0;

const DASH_MULTIPLIER: f32 = 4.0;
const DASH_DRAIN_RATE: f32 = 60.0;
const DASH_RECHARGE_RATE: f32 = 25.0;
const DASH_MAX: f32 = 100.0;

const BLOCK_RECHARGE_RATE: f32 = 15.0;
const BLOCK_MAX: f32 = 100.0;

const CHARGE_RATE: f32 = 50.0;
const CHARGE_MAX: f32 = 100.0;
const CHARGE_PROJECTILE_SPEED: f32 = 600.0;
const CHARGE_BASE_DAMAGE: f32 = 15.0;
const CHARGE_MAX_DAMAGE: f32 = 80.0;
const CHARGE_TARGET_RANGE: f32 = 500.0;

const ENEMY_COUNT: usize = 10;
const ENEMY_RADIUS: f32 = 14.0;
const ENEMY_SPEED: f32 = 100.0;
const ENEMY_DETECT_RANGE: f32 = 300.0;
const ENEMY_LOSE_RANGE: f32 = 400.0;
const ENEMY_HP: f32 = 50.0;
const ENEMY_MELEE_RANGE: f32 = 45.0;
const ENEMY_MELEE_DAMAGE: f32 = 8.0;
const ENEMY_MELEE_COOLDOWN: f32 = 1.0;
const ENEMY_RANGED_OPTIMAL: f32 = 200.0;
const ENEMY_RANGED_FIRE_RANGE: f32 = 280.0;
const ENEMY_RANGED_COOLDOWN: f32 = 1.5;
const ENEMY_RANGED_PROJ_SPEED: f32 = 350.0;
const ENEMY_RANGED_DAMAGE: f32 = 12.0;
const ENEMY_WANDER_INTERVAL: f32 = 2.0;

const OBSTACLE_COUNT: usize = 12;
const OBSTACLE_MIN_SIZE: f32 = 30.0;
const OBSTACLE_MAX_SIZE: f32 = 80.0;

const PROJECTILE_RADIUS: f32 = 5.0;
const PLAYER_HP: f32 = 100.0;

const STICK_DEADZONE: f32 = 0.2;
const HUD_HEIGHT: f32 = 40.0;

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: bevy::window::WindowResolution::new(1280, 900),
                title: "Battle Arena".to_string(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<GamepadInput>()
        .init_resource::<DamageQueue>()
        .init_resource::<ObstacleList>()
        .add_systems(Startup, (setup, build_obstacle_list).chain())
        .add_systems(
            Update,
            (
                read_gamepad,
                player_movement,
                player_aim,
                passive_melee_attack,
                active_attack,
                dash_system,
                block_system,
                block_shield_visual,
                charge_system,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                enemy_ai,
                enemy_ranged_fire,
                move_projectiles,
                projectile_hit_detection,
                apply_damage,
                despawn_dead,
                particle_system,
                update_hud,
            )
                .chain()
                .after(charge_system),
        )
        .run();
}

// ---------------------------------------------------------------------------
// XInput FFI (same pattern as dashboard example)
// ---------------------------------------------------------------------------

#[repr(C)]
struct XInputGamepad {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    thumb_lx: i16,
    thumb_ly: i16,
    thumb_rx: i16,
    thumb_ry: i16,
}

#[repr(C)]
struct XInputState {
    packet_number: u32,
    gamepad: XInputGamepad,
}

const XINPUT_GAMEPAD_A: u16 = 0x1000;
const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 0x0100;
const ERROR_SUCCESS: u32 = 0;

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
// Gamepad state resource
// ---------------------------------------------------------------------------

#[derive(Resource, Default)]
struct GamepadInput {
    connected: bool,
    left_stick: Vec2,
    right_stick: Vec2,
    left_trigger: f32,
    right_trigger: f32,
    a_button: bool,
    left_bumper: bool,
    prev_a: bool,
}

impl GamepadInput {
    fn a_just_pressed(&self) -> bool { self.a_button && !self.prev_a }
    fn a_just_released(&self) -> bool { !self.a_button && self.prev_a }
}

fn read_gamepad(
    mut state: ResMut<GamepadInput>,
    mut xinput_fn: Local<Option<Option<XInputGetStateFn>>>,
) {
    state.prev_a = state.a_button;

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
    let result = unsafe { get_state(0, xinput_state.as_mut_ptr()) };

    if result != ERROR_SUCCESS {
        *state = GamepadInput::default();
        return;
    }

    let xs = unsafe { xinput_state.assume_init() };
    let gp = &xs.gamepad;

    state.connected = true;
    state.left_stick = apply_deadzone(Vec2::new(
        normalize_thumb(gp.thumb_lx),
        normalize_thumb(gp.thumb_ly),
    ));
    state.right_stick = apply_deadzone(Vec2::new(
        normalize_thumb(gp.thumb_rx),
        normalize_thumb(gp.thumb_ry),
    ));
    state.left_trigger = gp.left_trigger as f32 / 255.0;
    state.right_trigger = gp.right_trigger as f32 / 255.0;
    state.a_button = gp.buttons & XINPUT_GAMEPAD_A != 0;
    state.left_bumper = gp.buttons & XINPUT_GAMEPAD_LEFT_SHOULDER != 0;
}

fn apply_deadzone(stick: Vec2) -> Vec2 {
    let len = stick.length();
    if len < STICK_DEADZONE {
        Vec2::ZERO
    } else {
        stick.normalize() * ((len - STICK_DEADZONE) / (1.0 - STICK_DEADZONE))
    }
}

// ---------------------------------------------------------------------------
// Components & resources
// ---------------------------------------------------------------------------

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Health {
    current: f32,
    max: f32,
}

#[derive(Component)]
struct DashMeter {
    current: f32,
}

#[derive(Component)]
struct BlockMeter {
    current: f32,
}

#[derive(Component)]
struct Blocking(bool);

#[derive(Component)]
struct ChargeMeter {
    current: f32,
    charging: bool,
    target: Option<Entity>,
}

#[derive(Component)]
struct MeleeCooldown(Timer);

#[derive(Component)]
struct ActiveAttackCooldown(Timer);

#[derive(Component)]
struct Facing(Vec2);

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct EnemyColor(Color);

#[derive(Component)]
enum EnemyType {
    Melee,
    Ranged,
}

#[derive(Component)]
enum EnemyState {
    Wander { dir: Vec2, timer: Timer },
    Chase,
}

#[derive(Component)]
struct AttackCooldown(Timer);

#[derive(Component)]
struct Projectile {
    velocity: Vec2,
    damage: f32,
    range_left: f32,
    friendly: bool,
}

#[derive(Component)]
struct Obstacle {
    half_size: Vec2,
}

#[derive(Component)]
struct Particle {
    velocity: Vec2,
    lifetime: Timer,
}

/// Simple damage queue — avoids Bevy event API changes.
#[derive(Resource, Default)]
struct DamageQueue {
    pending: Vec<(Entity, f32)>,
}

/// Cached obstacle bounds for collision (position + half_size).
#[derive(Resource, Default)]
struct ObstacleList {
    bounds: Vec<(Vec2, Vec2)>,
}

// HUD markers
#[derive(Component)]
struct HudDash;
#[derive(Component)]
struct HudBlock;
#[derive(Component)]
struct HudCharge;
#[derive(Component)]
struct HudHp;
#[derive(Component)]
struct HudTarget;

#[derive(Component)]
struct BlockShield;

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

const ACTIVE_ATTACK_COOLDOWN: f32 = 0.15;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    let mut rng = rand::rng();

    let wall_color = Color::srgb(0.4, 0.4, 0.5);
    let half_w = ARENA_W / 2.0;
    let half_h = ARENA_H / 2.0;
    let t = ARENA_WALL_THICKNESS;

    spawn_wall(&mut commands, Vec2::new(0.0, half_h + t / 2.0), Vec2::new(ARENA_W + t * 2.0, t), wall_color);
    spawn_wall(&mut commands, Vec2::new(0.0, -half_h - t / 2.0), Vec2::new(ARENA_W + t * 2.0, t), wall_color);
    spawn_wall(&mut commands, Vec2::new(-half_w - t / 2.0, 0.0), Vec2::new(t, ARENA_H + t * 2.0), wall_color);
    spawn_wall(&mut commands, Vec2::new(half_w + t / 2.0, 0.0), Vec2::new(t, ARENA_H + t * 2.0), wall_color);

    for _ in 0..OBSTACLE_COUNT {
        let w = rng.random_range(OBSTACLE_MIN_SIZE..OBSTACLE_MAX_SIZE);
        let h = rng.random_range(OBSTACLE_MIN_SIZE..OBSTACLE_MAX_SIZE);
        let x = rng.random_range((-half_w + w)..(half_w - w));
        let y = rng.random_range((-half_h + h)..(half_h - h));

        commands.spawn((
            Sprite {
                color: Color::srgb(0.3, 0.35, 0.3),
                custom_size: Some(Vec2::new(w, h)),
                ..default()
            },
            Transform::from_xyz(x, y, 0.0),
            Obstacle { half_size: Vec2::new(w / 2.0, h / 2.0) },
        ));
    }

    // Player — triangle pointing right (+X direction)
    let r = PLAYER_RADIUS;
    let player_tri = Triangle2d::new(
        Vec2::new(r * 1.3, 0.0),    // tip (front)
        Vec2::new(-r, r * 0.8),     // back-left
        Vec2::new(-r, -r * 0.8),    // back-right
    );
    let player_mesh = meshes.add(player_tri);
    let player_material = materials.add(ColorMaterial::from_color(Color::srgb(0.2, 0.6, 1.0)));

    let shield_mesh = meshes.add(Annulus::new(PLAYER_RADIUS * 1.8, PLAYER_RADIUS * 2.2));
    let shield_material = materials.add(ColorMaterial::from_color(Color::srgba(0.4, 0.7, 1.0, 0.0)));

    commands.spawn((
        Mesh2d(player_mesh),
        MeshMaterial2d(player_material),
        Transform::from_xyz(0.0, 0.0, 1.0),
        Player,
        Health { current: PLAYER_HP, max: PLAYER_HP },
        DashMeter { current: DASH_MAX },
        BlockMeter { current: BLOCK_MAX },
        Blocking(false),
        ChargeMeter { current: 0.0, charging: false, target: None },
        MeleeCooldown(Timer::from_seconds(PLAYER_MELEE_COOLDOWN, TimerMode::Once)),
        ActiveAttackCooldown(Timer::from_seconds(ACTIVE_ATTACK_COOLDOWN, TimerMode::Once)),
        Facing(Vec2::X),
    )).with_children(|parent| {
        parent.spawn((
            Mesh2d(shield_mesh),
            MeshMaterial2d(shield_material),
            Transform::from_xyz(0.0, 0.0, 0.5),
            BlockShield,
        ));
    });

    // Enemy triangle mesh (directional, like player)
    let er = ENEMY_RADIUS;
    let enemy_tri = Triangle2d::new(
        Vec2::new(er * 1.3, 0.0),
        Vec2::new(-er, er * 0.8),
        Vec2::new(-er, -er * 0.8),
    );
    let enemy_mesh = meshes.add(enemy_tri);
    let melee_material = materials.add(ColorMaterial::from_color(Color::srgb(0.9, 0.6, 0.2)));
    let ranged_material = materials.add(ColorMaterial::from_color(Color::srgb(0.9, 0.3, 0.3)));

    // Enemies
    for i in 0..ENEMY_COUNT {
        let is_ranged = i % 2 == 0;
        let x = rng.random_range(-half_w + 50.0..half_w - 50.0);
        let y = rng.random_range(-half_h + 50.0..half_h - 50.0);
        let (x, y) = if x.abs() < 100.0 && y.abs() < 100.0 {
            (x + 200.0, y + 200.0)
        } else {
            (x, y)
        };

        let color = if is_ranged {
            Color::srgb(0.9, 0.3, 0.3)
        } else {
            Color::srgb(0.9, 0.6, 0.2)
        };
        let material = if is_ranged { ranged_material.clone() } else { melee_material.clone() };

        let enemy_type = if is_ranged { EnemyType::Ranged } else { EnemyType::Melee };
        let cooldown = if is_ranged { ENEMY_RANGED_COOLDOWN } else { ENEMY_MELEE_COOLDOWN };

        commands.spawn((
            Mesh2d(enemy_mesh.clone()),
            MeshMaterial2d(material),
            Transform::from_xyz(x, y, 1.0),
            Enemy,
            enemy_type,
            EnemyColor(color),
            Health { current: ENEMY_HP, max: ENEMY_HP },
            EnemyState::Wander {
                dir: random_dir(&mut rng),
                timer: Timer::from_seconds(ENEMY_WANDER_INTERVAL, TimerMode::Once),
            },
            AttackCooldown(Timer::from_seconds(cooldown, TimerMode::Once)),
            Facing(Vec2::X),
        ));
    }

    // Build obstacle list resource for collision queries
    // (done after spawning obstacles so we can collect them)
    // We'll populate it in a startup system that runs after setup.

    // HUD — horizontal bar along the bottom, separated from the arena
    let hud_style = TextFont {
        font_size: 16.0,
        ..default()
    };

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            height: Val::Px(HUD_HEIGHT),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceEvenly,
            padding: UiRect::horizontal(Val::Px(20.0)),
            ..default()
        })
        .insert(BackgroundColor(Color::srgb(0.1, 0.1, 0.15)))
        .with_children(|parent| {
            parent.spawn((Text::new("HP: 100"), hud_style.clone(), HudHp));
            parent.spawn((Text::new("Dash: 100"), hud_style.clone(), HudDash));
            parent.spawn((Text::new("Block: 100"), hud_style.clone(), HudBlock));
            parent.spawn((Text::new("Charge: ready"), hud_style.clone(), HudCharge));
            parent.spawn((Text::new("Target: none"), hud_style.clone(), HudTarget));
        });
}

fn spawn_wall(commands: &mut Commands, pos: Vec2, size: Vec2, color: Color) {
    commands.spawn((
        Sprite {
            color,
            custom_size: Some(size),
            ..default()
        },
        Transform::from_translation(pos.extend(0.0)),
        Obstacle { half_size: size / 2.0 },
    ));
}

fn build_obstacle_list(
    mut list: ResMut<ObstacleList>,
    query: Query<(&Transform, &Obstacle)>,
) {
    list.bounds = query.iter().map(|(tf, obs)| {
        (tf.translation.truncate(), obs.half_size)
    }).collect();
}

/// Push a circle out of all obstacles. Returns corrected position.
fn resolve_obstacles(pos: Vec2, radius: f32, obstacles: &ObstacleList) -> Vec2 {
    let mut result = pos;
    for &(obs_pos, half_size) in &obstacles.bounds {
        let half = half_size + Vec2::splat(radius);
        let diff = result - obs_pos;
        if diff.x.abs() < half.x && diff.y.abs() < half.y {
            let overlap_x = half.x - diff.x.abs();
            let overlap_y = half.y - diff.y.abs();
            if overlap_x < overlap_y {
                result.x += overlap_x * diff.x.signum();
            } else {
                result.y += overlap_y * diff.y.signum();
            }
        }
    }
    result
}

/// Check if a point is inside any obstacle.
fn point_in_obstacle(pos: Vec2, obstacles: &ObstacleList) -> bool {
    for &(obs_pos, half_size) in &obstacles.bounds {
        let diff = pos - obs_pos;
        if diff.x.abs() < half_size.x && diff.y.abs() < half_size.y {
            return true;
        }
    }
    false
}

fn random_dir(rng: &mut impl Rng) -> Vec2 {
    let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
    Vec2::new(angle.cos(), angle.sin())
}

// ---------------------------------------------------------------------------
// Player systems
// ---------------------------------------------------------------------------

fn player_movement(
    gamepad: Res<GamepadInput>,
    time: Res<Time>,
    obstacles: Res<ObstacleList>,
    mut query: Query<(&mut Transform, &DashMeter, &Blocking), With<Player>>,
) {
    let Ok((mut transform, dash, blocking)) = query.single_mut() else { return };

    let dir = gamepad.left_stick;
    if dir == Vec2::ZERO { return; }

    let dashing = gamepad.left_bumper && dash.current > 0.0;
    let speed = if dashing { PLAYER_SPEED * DASH_MULTIPLIER } else { PLAYER_SPEED };
    let speed = if blocking.0 { speed * 0.4 } else { speed };

    let new_pos = transform.translation.truncate() + dir * speed * time.delta_secs();
    let half_w = ARENA_W / 2.0 - PLAYER_RADIUS;
    let half_h = ARENA_H / 2.0 - PLAYER_RADIUS;
    let clamped = new_pos.clamp(Vec2::new(-half_w, -half_h), Vec2::new(half_w, half_h));
    let resolved = resolve_obstacles(clamped, PLAYER_RADIUS, &obstacles);
    transform.translation.x = resolved.x;
    transform.translation.y = resolved.y;
}

fn player_aim(
    gamepad: Res<GamepadInput>,
    mut query: Query<(&mut Transform, &mut Facing), With<Player>>,
) {
    let Ok((mut transform, mut facing)) = query.single_mut() else { return };

    if gamepad.right_stick != Vec2::ZERO {
        let dir = gamepad.right_stick.normalize();
        facing.0 = dir;
        let angle = dir.y.atan2(dir.x);
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

fn passive_melee_attack(
    time: Res<Time>,
    mut damage: ResMut<DamageQueue>,
    mut player_q: Query<(&Transform, &mut MeleeCooldown), With<Player>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    let Ok((player_tf, mut cooldown)) = player_q.single_mut() else { return };
    cooldown.0.tick(time.delta());
    if !cooldown.0.is_finished() { return; }

    let player_pos = player_tf.translation.truncate();
    for (entity, enemy_tf) in &enemies {
        let dist = player_pos.distance(enemy_tf.translation.truncate());
        if dist < PLAYER_MELEE_RANGE {
            damage.pending.push((entity, PLAYER_MELEE_DAMAGE));
            cooldown.0.reset();
            break;
        }
    }
}

fn active_attack(
    gamepad: Res<GamepadInput>,
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(&Transform, &Facing, &mut ActiveAttackCooldown), With<Player>>,
) {
    let Ok((player_tf, facing, mut cooldown)) = query.single_mut() else { return };
    cooldown.0.tick(time.delta());

    // Fully automatic: fires repeatedly while right trigger is held
    if gamepad.right_trigger <= 0.5 { return; }
    if !cooldown.0.is_finished() { return; }

    cooldown.0.reset();
    let dir = facing.0;
    let pos = player_tf.translation.truncate() + dir * PLAYER_RADIUS;
    commands.spawn((
        Sprite {
            color: Color::srgb(0.5, 0.8, 1.0),
            custom_size: Some(Vec2::splat(PROJECTILE_RADIUS * 2.0)),
            ..default()
        },
        Transform::from_translation(pos.extend(2.0)),
        Projectile {
            velocity: dir * PLAYER_ACTIVE_SPEED,
            damage: PLAYER_ACTIVE_DAMAGE,
            range_left: PLAYER_ACTIVE_RANGE,
            friendly: true,
        },
    ));
}

fn dash_system(
    gamepad: Res<GamepadInput>,
    time: Res<Time>,
    mut query: Query<&mut DashMeter, With<Player>>,
) {
    let Ok(mut dash) = query.single_mut() else { return };
    let dt = time.delta_secs();

    if gamepad.left_bumper {
        dash.current = (dash.current - DASH_DRAIN_RATE * dt).max(0.0);
    } else {
        dash.current = (dash.current + DASH_RECHARGE_RATE * dt).min(DASH_MAX);
    }
}

fn block_system(
    gamepad: Res<GamepadInput>,
    time: Res<Time>,
    mut query: Query<(&mut BlockMeter, &mut Blocking), With<Player>>,
) {
    let Ok((mut block, mut blocking)) = query.single_mut() else { return };
    let dt = time.delta_secs();

    if gamepad.left_trigger > 0.5 && block.current > 0.0 {
        blocking.0 = true;
        block.current = (block.current - 2.0 * dt).max(0.0);
    } else {
        blocking.0 = false;
        block.current = (block.current + BLOCK_RECHARGE_RATE * dt).min(BLOCK_MAX);
    }
}

fn block_shield_visual(
    player_q: Query<(&Blocking, &BlockMeter), With<Player>>,
    shield_q: Query<&MeshMaterial2d<ColorMaterial>, With<BlockShield>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Ok((blocking, meter)) = player_q.single() else { return };
    let Ok(mat_handle) = shield_q.single() else { return };

    if let Some(mat) = materials.get_mut(&mat_handle.0) {
        let alpha = if blocking.0 {
            (meter.current / BLOCK_MAX) * 0.7
        } else {
            0.0
        };
        mat.color = Color::srgba(0.4, 0.7, 1.0, alpha);
    }
}

fn charge_system(
    gamepad: Res<GamepadInput>,
    time: Res<Time>,
    mut commands: Commands,
    mut player_q: Query<(&Transform, &Facing, &mut ChargeMeter), With<Player>>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
) {
    let Ok((player_tf, facing, mut charge)) = player_q.single_mut() else { return };
    let player_pos = player_tf.translation.truncate();
    let aim = facing.0;

    if gamepad.a_just_pressed() {
        let mut best: Option<(Entity, f32)> = None;
        for (entity, enemy_tf) in &enemies {
            let to_enemy = enemy_tf.translation.truncate() - player_pos;
            let dist = to_enemy.length();
            if dist > CHARGE_TARGET_RANGE { continue; }
            let dot = to_enemy.normalize_or_zero().dot(aim);
            if dot < 0.5 { continue; }
            if best.is_none() || dist < best.unwrap().1 {
                best = Some((entity, dist));
            }
        }
        charge.target = best.map(|(e, _)| e);
        charge.charging = true;
        charge.current = 0.0;
    }

    if gamepad.a_button && charge.charging {
        charge.current = (charge.current + CHARGE_RATE * time.delta_secs()).min(CHARGE_MAX);
    }

    if gamepad.a_just_released() && charge.charging {
        let charge_pct = charge.current / CHARGE_MAX;
        let damage = CHARGE_BASE_DAMAGE + (CHARGE_MAX_DAMAGE - CHARGE_BASE_DAMAGE) * charge_pct;

        let dir = if let Some(target) = charge.target {
            if let Ok((_, enemy_tf)) = enemies.get(target) {
                (enemy_tf.translation.truncate() - player_pos).normalize_or_zero()
            } else {
                aim
            }
        } else {
            aim
        };

        let pos = player_pos + dir * PLAYER_RADIUS;
        commands.spawn((
            Sprite {
                color: Color::srgb(1.0, 1.0, 0.3),
                custom_size: Some(Vec2::splat(PROJECTILE_RADIUS * 2.0 + charge_pct * 8.0)),
                ..default()
            },
            Transform::from_translation(pos.extend(2.0)),
            Projectile {
                velocity: dir * CHARGE_PROJECTILE_SPEED,
                damage,
                range_left: CHARGE_TARGET_RANGE,
                friendly: true,
            },
        ));

        charge.charging = false;
        charge.current = 0.0;
        charge.target = None;
    }
}

// ---------------------------------------------------------------------------
// Enemy AI
// ---------------------------------------------------------------------------

fn enemy_ai(
    time: Res<Time>,
    mut damage: ResMut<DamageQueue>,
    obstacles: Res<ObstacleList>,
    player_q: Query<&Transform, With<Player>>,
    mut enemies: Query<(
        &mut Transform,
        &mut EnemyState,
        &EnemyType,
        &mut AttackCooldown,
        &mut Facing,
    ), (With<Enemy>, Without<Player>)>,
    player_entity: Query<Entity, With<Player>>,
) {
    let Ok(player_tf) = player_q.single() else { return };
    let player_pos = player_tf.translation.truncate();
    let Ok(player_ent) = player_entity.single() else { return };
    let dt = time.delta_secs();

    let mut rng = rand::rng();

    for (mut transform, mut state, enemy_type, mut cooldown, mut facing) in &mut enemies {
        let pos = transform.translation.truncate();
        let to_player = player_pos - pos;
        let dist = to_player.length();
        let dir_to_player = to_player.normalize_or_zero();

        cooldown.0.tick(time.delta());

        match state.as_ref() {
            EnemyState::Wander { .. } => {
                if dist < ENEMY_DETECT_RANGE {
                    *state = EnemyState::Chase;
                    continue;
                }

                if let EnemyState::Wander { dir, timer } = state.as_mut() {
                    timer.tick(time.delta());
                    let new_pos = pos + *dir * ENEMY_SPEED * 0.5 * dt;
                    let half_w = ARENA_W / 2.0 - ENEMY_RADIUS;
                    let half_h = ARENA_H / 2.0 - ENEMY_RADIUS;

                    if new_pos.x.abs() > half_w || new_pos.y.abs() > half_h || timer.is_finished() {
                        *dir = random_dir(&mut rng);
                        timer.reset();
                    }

                    let clamped = new_pos.clamp(Vec2::new(-half_w, -half_h), Vec2::new(half_w, half_h));
                    let resolved = resolve_obstacles(clamped, ENEMY_RADIUS, &obstacles);
                    transform.translation.x = resolved.x;
                    transform.translation.y = resolved.y;
                    facing.0 = *dir;
                }
            }
            EnemyState::Chase => {
                if dist > ENEMY_LOSE_RANGE {
                    *state = EnemyState::Wander {
                        dir: random_dir(&mut rng),
                        timer: Timer::from_seconds(ENEMY_WANDER_INTERVAL, TimerMode::Once),
                    };
                    continue;
                }

                facing.0 = dir_to_player;

                match enemy_type {
                    EnemyType::Melee => {
                        if dist < ENEMY_MELEE_RANGE {
                            if cooldown.0.is_finished() {
                                damage.pending.push((player_ent, ENEMY_MELEE_DAMAGE));
                                cooldown.0.reset();
                            }
                        } else {
                            let new_pos = pos + dir_to_player * ENEMY_SPEED * dt;
                            let resolved = resolve_obstacles(new_pos, ENEMY_RADIUS, &obstacles);
                            transform.translation.x = resolved.x;
                            transform.translation.y = resolved.y;
                        }
                    }
                    EnemyType::Ranged => {
                        let move_dir = if dist < ENEMY_RANGED_OPTIMAL * 0.7 {
                            -dir_to_player
                        } else if dist > ENEMY_RANGED_OPTIMAL * 1.3 {
                            dir_to_player
                        } else {
                            Vec2::new(-dir_to_player.y, dir_to_player.x)
                        };

                        let new_pos = pos + move_dir * ENEMY_SPEED * dt;
                        let half_w = ARENA_W / 2.0 - ENEMY_RADIUS;
                        let half_h = ARENA_H / 2.0 - ENEMY_RADIUS;
                        let clamped = new_pos.clamp(Vec2::new(-half_w, -half_h), Vec2::new(half_w, half_h));
                        let resolved = resolve_obstacles(clamped, ENEMY_RADIUS, &obstacles);
                        transform.translation.x = resolved.x;
                        transform.translation.y = resolved.y;
                    }
                }
            }
        }

        // Rotate to match facing direction
        let angle = facing.0.y.atan2(facing.0.x);
        transform.rotation = Quat::from_rotation_z(angle);
    }
}

fn enemy_ranged_fire(
    mut commands: Commands,
    player_q: Query<&Transform, With<Player>>,
    mut enemies: Query<
        (&Transform, &EnemyType, &mut AttackCooldown),
        With<Enemy>,
    >,
) {
    let Ok(player_tf) = player_q.single() else { return };
    let player_pos = player_tf.translation.truncate();

    for (transform, enemy_type, mut cooldown) in &mut enemies {
        if !matches!(enemy_type, EnemyType::Ranged) { continue; }
        if !cooldown.0.is_finished() { continue; }

        let pos = transform.translation.truncate();
        let dist = pos.distance(player_pos);
        if dist > ENEMY_RANGED_FIRE_RANGE { continue; }
        if dist > ENEMY_LOSE_RANGE { continue; }

        let dir = (player_pos - pos).normalize_or_zero();
        commands.spawn((
            Sprite {
                color: Color::srgb(1.0, 0.4, 0.4),
                custom_size: Some(Vec2::splat(PROJECTILE_RADIUS * 2.0)),
                ..default()
            },
            Transform::from_translation(pos.extend(2.0)),
            Projectile {
                velocity: dir * ENEMY_RANGED_PROJ_SPEED,
                damage: ENEMY_RANGED_DAMAGE,
                range_left: ENEMY_RANGED_FIRE_RANGE * 1.5,
                friendly: false,
            },
        ));

        cooldown.0.reset();
    }
}

// ---------------------------------------------------------------------------
// Projectile systems
// ---------------------------------------------------------------------------

fn move_projectiles(
    time: Res<Time>,
    obstacles: Res<ObstacleList>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut proj) in &mut projectiles {
        let movement = proj.velocity * dt;
        transform.translation += movement.extend(0.0);
        proj.range_left -= movement.length();

        let pos = transform.translation.truncate();
        let half_w = ARENA_W / 2.0;
        let half_h = ARENA_H / 2.0;
        if proj.range_left <= 0.0 || pos.x.abs() > half_w || pos.y.abs() > half_h
            || point_in_obstacle(pos, &obstacles)
        {
            commands.entity(entity).despawn();
        }
    }
}

fn projectile_hit_detection(
    mut commands: Commands,
    mut damage: ResMut<DamageQueue>,
    projectiles: Query<(Entity, &Transform, &Projectile)>,
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    player_q: Query<(Entity, &Transform), With<Player>>,
) {
    for (proj_entity, proj_tf, proj) in &projectiles {
        let proj_pos = proj_tf.translation.truncate();

        if proj.friendly {
            for (enemy_entity, enemy_tf) in &enemies {
                let dist = proj_pos.distance(enemy_tf.translation.truncate());
                if dist < ENEMY_RADIUS + PROJECTILE_RADIUS {
                    damage.pending.push((enemy_entity, proj.damage));
                    commands.entity(proj_entity).despawn();
                    break;
                }
            }
        } else {
            if let Ok((player_entity, player_tf)) = player_q.single() {
                let dist = proj_pos.distance(player_tf.translation.truncate());
                if dist < PLAYER_RADIUS + PROJECTILE_RADIUS {
                    damage.pending.push((player_entity, proj.damage));
                    commands.entity(proj_entity).despawn();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Damage
// ---------------------------------------------------------------------------

fn apply_damage(
    mut damage: ResMut<DamageQueue>,
    mut health_q: Query<(&mut Health, Option<&Blocking>, Option<&mut BlockMeter>)>,
) {
    for (target, amount) in damage.pending.drain(..) {
        let Ok((mut health, blocking, block_meter)) = health_q.get_mut(target) else {
            continue;
        };

        if let (Some(blocking), Some(mut meter)) = (blocking, block_meter) {
            if blocking.0 && meter.current > 0.0 {
                meter.current = (meter.current - amount).max(0.0);
                continue;
            }
        }

        health.current = (health.current - amount).max(0.0);
    }
}

fn despawn_dead(
    mut commands: Commands,
    query: Query<(Entity, &Health, &Transform, &EnemyColor), (With<Enemy>, Without<Player>)>,
) {
    let mut rng = rand::rng();
    for (entity, health, transform, enemy_color) in &query {
        if health.current <= 0.0 {
            let pos = transform.translation.truncate();
            let base = enemy_color.0.to_srgba();

            // Firework explosion — particles burst outward in all directions
            // Ring 1: fast bright core burst
            for _ in 0..16 {
                let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                let speed: f32 = rng.random_range(200.0..400.0);
                let vel = Vec2::new(angle.cos(), angle.sin()) * speed;
                let size = rng.random_range(3.0..6.0);
                commands.spawn((
                    Sprite {
                        color: Color::srgb(1.0, 1.0, 0.9), // bright white-yellow flash
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    },
                    Transform::from_translation(pos.extend(4.0)),
                    Particle {
                        velocity: vel,
                        lifetime: Timer::from_seconds(rng.random_range(0.15..0.35), TimerMode::Once),
                    },
                ));
            }

            // Ring 2: medium enemy-colored sparks
            for _ in 0..24 {
                let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                let speed: f32 = rng.random_range(100.0..280.0);
                let vel = Vec2::new(angle.cos(), angle.sin()) * speed;
                let size = rng.random_range(2.0..5.0);
                // Vary the color slightly for sparkle
                let r = (base.red + rng.random_range(-0.1..0.15)).clamp(0.0, 1.0);
                let g = (base.green + rng.random_range(-0.1..0.15)).clamp(0.0, 1.0);
                let b = (base.blue + rng.random_range(-0.1..0.15)).clamp(0.0, 1.0);
                commands.spawn((
                    Sprite {
                        color: Color::srgb(r, g, b),
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    },
                    Transform::from_translation(pos.extend(3.0)),
                    Particle {
                        velocity: vel,
                        lifetime: Timer::from_seconds(rng.random_range(0.3..0.7), TimerMode::Once),
                    },
                ));
            }

            // Ring 3: slow drifting embers
            for _ in 0..12 {
                let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                let speed: f32 = rng.random_range(30.0..90.0);
                let vel = Vec2::new(angle.cos(), angle.sin()) * speed;
                let size = rng.random_range(1.5..3.5);
                commands.spawn((
                    Sprite {
                        color: Color::srgb(
                            base.red * 0.6,
                            base.green * 0.4,
                            base.blue * 0.3,
                        ),
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    },
                    Transform::from_translation(pos.extend(3.0)),
                    Particle {
                        velocity: vel,
                        lifetime: Timer::from_seconds(rng.random_range(0.5..1.0), TimerMode::Once),
                    },
                ));
            }

            commands.entity(entity).despawn();
        }
    }
}

fn particle_system(
    time: Res<Time>,
    mut commands: Commands,
    mut particles: Query<(Entity, &mut Transform, &mut Sprite, &mut Particle)>,
) {
    let dt = time.delta_secs();
    for (entity, mut transform, mut sprite, mut particle) in &mut particles {
        particle.lifetime.tick(time.delta());
        if particle.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        // Move
        transform.translation.x += particle.velocity.x * dt;
        transform.translation.y += particle.velocity.y * dt;

        // Slow down
        particle.velocity *= 0.95;

        // Fade out
        let frac = particle.lifetime.fraction_remaining();
        sprite.color = sprite.color.with_alpha(frac);
    }
}

// ---------------------------------------------------------------------------
// HUD (screen-space UI)
// ---------------------------------------------------------------------------

fn update_hud(
    player_q: Query<(&Health, &DashMeter, &BlockMeter, &ChargeMeter), With<Player>>,
    mut hp_text: Query<&mut Text, (With<HudHp>, Without<HudDash>, Without<HudBlock>, Without<HudCharge>, Without<HudTarget>)>,
    mut dash_text: Query<&mut Text, (With<HudDash>, Without<HudHp>, Without<HudBlock>, Without<HudCharge>, Without<HudTarget>)>,
    mut block_text: Query<&mut Text, (With<HudBlock>, Without<HudHp>, Without<HudDash>, Without<HudCharge>, Without<HudTarget>)>,
    mut charge_text: Query<&mut Text, (With<HudCharge>, Without<HudHp>, Without<HudDash>, Without<HudBlock>, Without<HudTarget>)>,
    mut target_text: Query<&mut Text, (With<HudTarget>, Without<HudHp>, Without<HudDash>, Without<HudBlock>, Without<HudCharge>)>,
) {
    let Ok((health, dash, block, charge)) = player_q.single() else { return };

    if let Ok(mut text) = hp_text.single_mut() {
        **text = format!("HP: {:.0}/{:.0}", health.current, health.max);
    }
    if let Ok(mut text) = dash_text.single_mut() {
        **text = format!("Dash: {:.0}/{:.0}", dash.current, DASH_MAX);
    }
    if let Ok(mut text) = block_text.single_mut() {
        **text = format!("Block: {:.0}/{:.0}", block.current, BLOCK_MAX);
    }
    if let Ok(mut text) = charge_text.single_mut() {
        if charge.charging {
            **text = format!("Charge: {:.0}%", charge.current / CHARGE_MAX * 100.0);
        } else {
            **text = "Charge: ready".to_string();
        }
    }
    if let Ok(mut text) = target_text.single_mut() {
        if charge.target.is_some() {
            **text = "Target: LOCKED".to_string();
        } else {
            **text = "Target: none".to_string();
        }
    }
}
