#![allow(dead_code)] // teaching example: not all variants/fields are exercised
// Enums & Pattern Matching - Used heavily in game logic
//
// Rust enums are NOT like Java enums (which are just named constants).
// Rust enums are "algebraic data types" - each variant can carry different data.
// Combined with `match`, they replace the if/else chains and instanceof checks
// you'd use in JVM/JS for state machines, commands, and game events.

fn main() {
    basic_enums();
    enums_with_data();
    option_is_an_enum();
    game_state_machine();
}

// ── BASIC ENUMS ─────────────────────────────────────────────────────
// Like Java enums at first glance, but match forces you to handle every variant.

#[derive(Debug)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

fn basic_enums() {
    let facing = Direction::Left;

    // match is exhaustive - comment out any arm and it won't compile
    let label = match facing {
        Direction::Up => "north",
        Direction::Down => "south",
        Direction::Left => "west",
        Direction::Right => "east",
    };

    println!("[basic] facing {facing:?} = {label}");
}

// ── ENUMS WITH DATA ─────────────────────────────────────────────────
// Each variant can hold different types and amounts of data.
// This replaces inheritance hierarchies and marker interfaces.

#[derive(Debug)]
enum DamageSource {
    Melee { weapon: String, critical: bool },
    Ranged { weapon: String, distance: f32 },
    Fire { duration_secs: f32 },
    Falling,
}

fn enums_with_data() {
    let sources = vec![
        DamageSource::Melee {
            weapon: "Sword".into(),
            critical: true,
        },
        DamageSource::Ranged {
            weapon: "Bow".into(),
            distance: 45.0,
        },
        DamageSource::Fire { duration_secs: 3.0 },
        DamageSource::Falling,
    ];

    for source in &sources {
        let description = describe_damage(source);
        println!("[data] {description}");
    }
}

fn describe_damage(source: &DamageSource) -> String {
    // match destructures the enum, extracting the inner data
    match source {
        DamageSource::Melee { weapon, critical } => {
            if *critical {
                format!("CRITICAL hit with {weapon}!")
            } else {
                format!("Hit with {weapon}")
            }
        }
        DamageSource::Ranged { weapon, distance } => {
            format!("Shot by {weapon} from {distance}m away")
        }
        DamageSource::Fire { duration_secs } => {
            format!("Burning for {duration_secs}s")
        }
        DamageSource::Falling => "Fell to their doom".to_string(),
    }
}

// ── OPTION IS JUST AN ENUM ──────────────────────────────────────────
// Rust has no null. Instead, Option<T> is an enum: Some(value) or None.
// The compiler forces you to handle the None case.

fn option_is_an_enum() {
    let inventory = vec!["Sword", "Shield", "Potion"];

    // .get() returns Option<&&str>, not &str - might be out of bounds
    let first = inventory.get(0);
    let missing = inventory.get(99);

    println!("[option] first: {first:?}");   // Some("Sword")
    println!("[option] missing: {missing:?}"); // None

    // if let - convenient when you only care about one variant
    if let Some(item) = inventory.get(0) {
        println!("[option] found: {item}");
    }

    // unwrap_or - provide a default for None
    let item = inventory.get(99).unwrap_or(&&"empty slot");
    println!("[option] with fallback: {item}");
}

// ── GAME STATE MACHINE ──────────────────────────────────────────────
// Enums + match naturally model state machines - very common in games.
// In Bevy, this pattern appears as the States derive macro.

#[derive(Debug)]
enum EnemyState {
    Idle,
    Patrol { waypoint_index: usize },
    Chase { target_distance: f32 },
    Attack { cooldown: f32 },
    Dead,
}

fn game_state_machine() {
    let states = vec![
        EnemyState::Idle,
        EnemyState::Patrol { waypoint_index: 3 },
        EnemyState::Chase { target_distance: 15.0 },
        EnemyState::Chase { target_distance: 4.0 },
        EnemyState::Attack { cooldown: 0.5 },
        EnemyState::Dead,
    ];

    for state in &states {
        let next = next_state(state);
        println!("[state] {state:?} -> {next:?}");
    }
}

fn next_state(current: &EnemyState) -> EnemyState {
    match current {
        EnemyState::Idle => EnemyState::Patrol { waypoint_index: 0 },
        EnemyState::Patrol { .. } => EnemyState::Chase { target_distance: 20.0 },
        // Guards: match arms can have conditions
        EnemyState::Chase { target_distance } if *target_distance <= 5.0 => {
            EnemyState::Attack { cooldown: 1.0 }
        }
        EnemyState::Chase { target_distance } => {
            EnemyState::Chase {
                target_distance: target_distance - 5.0,
            }
        }
        EnemyState::Attack { .. } => EnemyState::Idle,
        EnemyState::Dead => EnemyState::Dead, // dead stays dead
    }
}
