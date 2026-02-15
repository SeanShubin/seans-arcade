// Traits - Rust's approach to polymorphism
//
// Traits are like Java interfaces, but more powerful:
// - You can implement a trait for types you didn't write (orphan rules permitting)
// - Traits can have default method implementations
// - Generic bounds use traits: fn foo<T: MyTrait>(x: T)
// - Bevy uses traits everywhere: Component, Resource, Plugin, Bundle, States
//
// Key difference from Java: no inheritance. Traits define shared behavior,
// structs hold data, and the two are composed independently.

fn main() {
    basic_trait();
    default_methods();
    trait_bounds();
    multiple_traits();
    bevy_connection();
}

// ── BASIC TRAIT ──────────────────────────────────────────────────────
// Define shared behavior. Each type provides its own implementation.

trait Describable {
    fn describe(&self) -> String;
}

struct Sword {
    damage: f32,
}

struct Shield {
    armor: f32,
}

impl Describable for Sword {
    fn describe(&self) -> String {
        format!("Sword ({} damage)", self.damage)
    }
}

impl Describable for Shield {
    fn describe(&self) -> String {
        format!("Shield ({} armor)", self.armor)
    }
}

fn basic_trait() {
    let sword = Sword { damage: 25.0 };
    let shield = Shield { armor: 15.0 };

    println!("[basic] {}", sword.describe());
    println!("[basic] {}", shield.describe());
}

// ── DEFAULT METHODS ─────────────────────────────────────────────────
// Traits can provide default implementations. Types can override them.
// Similar to default methods in Java interfaces.

trait HasHealth {
    fn max_health(&self) -> f32;
    fn current_health(&self) -> f32;

    // Default implementation using the other methods
    fn health_percentage(&self) -> f32 {
        self.current_health() / self.max_health() * 100.0
    }

    fn is_alive(&self) -> bool {
        self.current_health() > 0.0
    }
}

struct Player {
    health: f32,
}

struct Boss {
    health: f32,
    phase: u8,
}

impl HasHealth for Player {
    fn max_health(&self) -> f32 { 100.0 }
    fn current_health(&self) -> f32 { self.health }
}

impl HasHealth for Boss {
    fn max_health(&self) -> f32 { 500.0 * self.phase as f32 }
    fn current_health(&self) -> f32 { self.health }
}

fn default_methods() {
    let player = Player { health: 72.0 };
    let boss = Boss { health: 300.0, phase: 2 };

    // Both use the default health_percentage() and is_alive()
    println!("[default] player: {:.0}% alive={}", player.health_percentage(), player.is_alive());
    println!("[default] boss: {:.0}% alive={}", boss.health_percentage(), boss.is_alive());
}

// ── TRAIT BOUNDS (GENERICS) ─────────────────────────────────────────
// "This function works with ANY type, as long as it implements this trait."
// Like Java's <T extends Interface> but more flexible.

fn print_health_report<T: HasHealth>(entity: &T) {
    println!(
        "[bounds] health: {}/{} ({:.0}%)",
        entity.current_health(),
        entity.max_health(),
        entity.health_percentage()
    );
}

// Alternative syntax with `where` clause - cleaner for multiple bounds
fn compare_health<A, B>(a: &A, b: &B)
where
    A: HasHealth + Describable,
    B: HasHealth + Describable,
{
    let winner = if a.current_health() > b.current_health() { "first" } else { "second" };
    println!(
        "[bounds] {} vs {} -> {winner} has more health",
        a.describe(),
        b.describe()
    );
}

// Make Player and Boss describable so we can use compare_health
impl Describable for Player {
    fn describe(&self) -> String {
        format!("Player ({}hp)", self.health)
    }
}

impl Describable for Boss {
    fn describe(&self) -> String {
        format!("Boss phase {} ({}hp)", self.phase, self.health)
    }
}

fn trait_bounds() {
    let player = Player { health: 72.0 };
    let boss = Boss { health: 300.0, phase: 2 };

    print_health_report(&player);
    print_health_report(&boss); // same function, different types

    compare_health(&player, &boss);
}

// ── MULTIPLE TRAITS ─────────────────────────────────────────────────
// A type can implement as many traits as needed. This replaces the
// "implements InterfaceA, InterfaceB" pattern from Java, but without
// any inheritance hierarchy.

trait Damageable {
    fn take_damage(&mut self, amount: f32);
}

impl Damageable for Player {
    fn take_damage(&mut self, amount: f32) {
        self.health = (self.health - amount).max(0.0);
    }
}

fn multiple_traits() {
    let mut player = Player { health: 100.0 };

    println!("[multi] before: {}", player.describe());
    player.take_damage(35.0);
    println!("[multi] after damage: {} alive={}", player.describe(), player.is_alive());
    player.take_damage(999.0);
    println!("[multi] overkill: {} alive={}", player.describe(), player.is_alive());
}

// ── CONNECTION TO BEVY ──────────────────────────────────────────────
// Bevy uses derive macros to auto-implement traits:
//
//   #[derive(Component)]  - marks a struct as attachable to entities
//   #[derive(Resource)]   - marks a struct as shared game state
//   #[derive(Event)]      - marks a struct as a typed event
//   #[derive(States)]     - marks an enum as a game state machine
//   #[derive(Bundle)]     - marks a struct as a group of components
//
// These are just traits. The derive macros generate the impl blocks for you.
// When you write #[derive(Component)], the compiler generates:
//
//   impl Component for MyStruct { ... }

fn bevy_connection() {
    println!("[bevy] In Bevy, you'd write:");
    println!("[bevy]   #[derive(Component)]");
    println!("[bevy]   struct Health {{ current: f32, max: f32 }}");
    println!("[bevy]");
    println!("[bevy] This auto-generates: impl Component for Health {{ ... }}");
    println!("[bevy] Which lets Bevy's ECS store, query, and manage Health data.");
}
