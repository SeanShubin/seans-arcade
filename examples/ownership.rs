// Ownership & Borrowing - The big paradigm shift from JVM/JS/TS
//
// In JVM/JS: objects live on the heap, garbage collector cleans them up.
// In Rust: every value has exactly one owner. When the owner goes out of scope, the value is dropped.
// No garbage collector. No manual free. Compiler enforces it at compile time.

fn main() {
    moving();
    borrowing();
    mutable_borrowing();
    lifetimes_intro();
}

// ── MOVE SEMANTICS ──────────────────────────────────────────────────
// In JVM/JS, assigning an object copies the reference. Both variables point to the same object.
// In Rust, assigning a String MOVES ownership. The original variable is invalidated.

fn moving() {
    let name = String::from("Bevy");
    let moved_name = name; // ownership moves to moved_name

    // println!("{name}");  // WON'T COMPILE: name was moved

    println!("[move] moved_name = {moved_name}");

    // Primitives (i32, f32, bool) implement Copy - they're cheap to duplicate.
    // Assignment copies instead of moving.
    let score = 42;
    let copied_score = score;
    println!("[move] both valid: score = {score}, copied_score = {copied_score}");

    // To explicitly duplicate heap data, use .clone()
    let original = String::from("clone me");
    let cloned = original.clone();
    println!("[move] both valid: original = {original}, cloned = {cloned}");
}

// ── BORROWING (IMMUTABLE REFERENCES) ────────────────────────────────
// Instead of moving, you can lend access with & (a reference).
// The borrower can read but not modify. The owner retains ownership.
// You can have unlimited simultaneous immutable borrows.

fn borrowing() {
    let player_name = String::from("Player 1");

    // &player_name borrows it - print_name can read it, doesn't take ownership
    print_name(&player_name);
    print_name(&player_name); // can borrow again, still valid

    println!("[borrow] still mine: {player_name}");
}

fn print_name(name: &str) {
    println!("[borrow] name is: {name}");
}

// ── MUTABLE BORROWING ───────────────────────────────────────────────
// &mut gives a mutable reference - the borrower can modify the value.
// Rule: you can have EITHER one &mut OR any number of & at a time, never both.
// This prevents data races at compile time.

fn mutable_borrowing() {
    let mut health = 100;

    apply_damage(&mut health, 30);
    println!("[mut borrow] health after damage: {health}");

    apply_healing(&mut health, 10);
    println!("[mut borrow] health after healing: {health}");

    // This illustrates the exclusivity rule:
    // let r1 = &health;       // immutable borrow
    // let r2 = &mut health;   // WON'T COMPILE: can't have &mut while & exists
}

fn apply_damage(health: &mut i32, amount: i32) {
    *health -= amount; // *health dereferences the pointer to modify the value
}

fn apply_healing(health: &mut i32, amount: i32) {
    *health += amount;
}

// ── OWNERSHIP IN FUNCTIONS ──────────────────────────────────────────
// When you pass a value (not a reference) to a function, ownership moves into the function.
// When the function ends, the value is dropped - unless the function returns it.

fn lifetimes_intro() {
    let weapon = String::from("Sword");

    // This takes ownership and gives it back
    let weapon = upgrade(weapon);
    println!("[lifetime] upgraded: {weapon}");

    // This borrows - cheaper, no ownership transfer needed
    let length = measure(&weapon);
    println!("[lifetime] length of '{weapon}': {length}");
}

fn upgrade(mut weapon: String) -> String {
    weapon.push_str(" +1"); // takes ownership, modifies, returns
    weapon
}

fn measure(text: &str) -> usize {
    text.len() // borrows, just reads, doesn't need ownership
}
