# Rust Implicit Conventions

A reference for developers who prefer explicit configuration over magic. Rust, Cargo, and Bevy rely heavily on convention-over-configuration. This document catalogs the implicit behaviors so you can understand what's happening without reading framework source code.

---

## Cargo: Project Structure

### Hardcoded Directory and File Paths

Cargo auto-discovers targets based on file locations relative to a package's `Cargo.toml`. None of these directories are configurable without explicit `[[target]]` declarations.

| File/Directory | What Cargo sees | Run with |
|---------------|----------------|----------|
| `src/main.rs` | Binary target (named after package) | `cargo run` |
| `src/lib.rs` | Library target | Used as dependency |
| `src/bin/*.rs` | Additional binary targets | `cargo run --bin name` |
| `src/bin/name/main.rs` | Multi-file binary target | `cargo run --bin name` |
| `examples/*.rs` | Example targets | `cargo run --example name` |
| `examples/name/main.rs` | Multi-file example target | `cargo run --example name` |
| `tests/*.rs` | Integration test targets | `cargo test --test name` |
| `benches/*.rs` | Benchmark targets | `cargo bench --bench name` |
| `build.rs` | Build script — **runs automatically before compilation** | Automatic |

**The burn:** `build.rs` at the crate root runs code at compile time with no declaration. If you see a `build.rs` in a project, it is executing during every build. It can generate source files, set environment variables, link native libraries, and print `cargo:` directives that change compilation behavior.

**Override any of these** with explicit declarations in `Cargo.toml`:
```toml
[[bin]]
name = "my_binary"
path = "wherever/main.rs"

[[example]]
name = "my_example"
path = "somewhere_else/demo.rs"
```

### Cargo.lock

- **Auto-generated** from `Cargo.toml`. You never edit it by hand.
- `cargo build` creates or updates it automatically.
- Libraries (crates published to crates.io) conventionally don't commit `Cargo.lock`. Applications do.
- If `Cargo.lock` exists, `cargo build` uses the exact versions pinned in it. `cargo update` refreshes it.

### Workspace Auto-Discovery

```toml
[workspace]
members = ["crates/*"]  # glob patterns work
```

Cargo reads each member directory's `Cargo.toml` and applies all the file conventions above within each member independently.

### Default Features

Dependencies can have optional "features" that add functionality. Some are enabled by default:

```toml
[dependencies]
bevy = "0.18"           # includes default features (rendering, audio, etc.)
bevy = { version = "0.18", default-features = false, features = ["..."] }  # explicit
```

**The burn:** Adding a dependency can pull in far more code than expected. `bevy` with default features includes windowing, rendering, audio, asset loading, and more. Check a crate's `Cargo.toml` or docs.rs page to see what `default` includes.

### Version Resolution

```toml
bevy = "0.18.0"   # actually means >=0.18.0, <0.19.0 (semver compatible)
bevy = "=0.18.0"  # exactly 0.18.0
```

**The burn:** `"0.18.0"` is not an exact pin. It's a compatibility range. Cargo may resolve to `0.18.3` if available. Use `=0.18.0` or rely on `Cargo.lock` for exact versions.

---

## Rust: Module System

### `mod` Declarations Map to Files

```rust
mod combat;  // looks for EITHER:
             //   combat.rs        (file)
             //   combat/mod.rs    (directory with entry point)
             // Both relative to the file containing the `mod` declaration
```

There is no import path configuration. The compiler determines file locations from the module name and the declaring file's location.

**The burn:** If you have both `combat.rs` and `combat/mod.rs`, the compiler rejects it as ambiguous. Pick one convention and stick with it.

### Visibility Defaults to Private

```rust
struct Foo;           // private to current module
pub struct Bar;       // visible to parent and beyond
pub(crate) fn baz();  // visible within the crate only
pub(super) fn qux();  // visible to parent module only
```

**The burn:** If you define a type in a module and can't use it elsewhere, you forgot `pub`. No error at the definition site — the error appears at the use site.

### The Prelude

Every Rust file implicitly imports `std::prelude::v1::*`, which includes `Option`, `Result`, `Vec`, `String`, `Clone`, `Copy`, `Send`, `Sync`, and more. You never see these imports, but they're there.

Bevy adds its own prelude:
```rust
use bevy::prelude::*;  // imports ~100+ types, traits, and functions
```

**The burn:** You can't grep for where `Transform` or `Query` is defined by looking at imports — it comes through the prelude glob. Use your IDE's "go to definition" or check Bevy's `prelude` module docs.

---

## Rust: Language-Level Implicit Behavior

### Implicit Return

The last expression in a function (without a semicolon) is the return value:

```rust
fn add(a: i32, b: i32) -> i32 {
    a + b    // no semicolon = return value
}

fn oops(a: i32, b: i32) -> i32 {
    a + b;   // semicolon = statement returning (), compile error
}
```

**The burn:** Adding a semicolon to the last line changes it from a return value to a statement. The compiler error message is helpful, but the cause is non-obvious.

### Deref Coercion

Rust automatically dereferences types through the `Deref` trait chain:

```rust
let s: String = String::from("hello");
let r: &str = &s;  // String → &str automatically via Deref
```

In Bevy, this shows up with smart pointers and ECS types:
```rust
// Res<T> derefs to &T, ResMut<T> derefs to &mut T
fn my_system(time: Res<Time>) {
    let dt = time.delta_secs();  // calling Time::delta_secs through Res's Deref
}
```

**The burn:** You'll see method calls on wrapper types that seem to have methods they don't define. The methods come from the inner type via automatic dereferencing.

### Auto-Referencing in Method Calls

When you call `foo.bar()`, Rust tries `foo.bar()`, then `(&foo).bar()`, then `(&mut foo).bar()`, then `(*foo).bar()`, and so on. It automatically adds references or dereferences to make the method call work.

**The burn:** You can call a method that takes `&self` on an owned value without explicitly borrowing. This is convenient but hides whether you're moving, borrowing, or mutably borrowing.

### `?` Operator

```rust
let file = File::open("foo.txt")?;
// is sugar for:
let file = match File::open("foo.txt") {
    Ok(f) => f,
    Err(e) => return Err(e.into()),
};
```

**The burn:** The `?` also calls `.into()` on the error, which can silently convert between error types. If a function returns `Result<T, MyError>`, the `?` on a `std::io::Error` will try to convert it via the `From` trait. If that trait isn't implemented, you get a confusing compile error.

### Trait Methods Appear from Nowhere

If a trait is in scope, its methods become available on types that implement it:

```rust
use std::io::Read;  // brings .read() method into scope

let mut buf = [0u8; 1024];
file.read(&mut buf);  // only works because `Read` is imported
```

**The burn:** If a method call stops compiling, check whether a `use` for its trait was removed. The error message says "method not found" rather than "trait not in scope."

---

## Rust: Derive Macros

### `#[derive(...)]` Generates Hidden Code

```rust
#[derive(Debug, Clone, PartialEq)]
struct Foo { x: i32 }
```

This generates `impl Debug for Foo`, `impl Clone for Foo`, and `impl PartialEq for Foo` automatically. You never see this code.

**Common derives and what they silently add:**

| Derive | What it generates |
|--------|------------------|
| `Debug` | `fmt::Debug` implementation for `{:?}` printing |
| `Clone` | `.clone()` method |
| `Copy` | Implicit copy on assignment (value is duplicated, not moved) |
| `Default` | `Default::default()` constructor |
| `PartialEq` / `Eq` | `==` operator |
| `Hash` | Makes type usable as HashMap key |
| `Serialize` / `Deserialize` | Serde: automatic serialization to JSON, postcard, etc. |

**The burn:** `#[derive(Copy)]` silently changes the semantics of assignment from move to copy. If you're debugging why a value is still available after being "moved," check if the type derives `Copy`.

---

## Bevy-Specific Conventions

### Derive Macros for ECS Registration

```rust
#[derive(Component)]    // makes this a component (attachable to entities)
struct Health(f32);

#[derive(Resource)]     // makes this a resource (global singleton)
struct Score(u32);

#[derive(Event)]        // makes this an event type
struct DamageEvent { amount: f32 }

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState { #[default] Menu, Playing }
```

**The burn:** These derives generate trait implementations and sometimes registration code. Without `#[derive(Component)]`, you can't use a struct in a `Query`. The compile error will say the trait bound isn't satisfied, not "you forgot the derive."

### System Functions are Detected by Signature

Bevy systems are plain functions, but the framework interprets their parameter types:

```rust
fn my_system(
    query: Query<&Transform>,  // Bevy sees this and provides entity data
    time: Res<Time>,           // Bevy sees this and provides the Time resource
) { }
```

There is no registration annotation. Bevy uses Rust's type system at compile time to determine what data each system needs. If you add a parameter type that Bevy doesn't recognize as a valid system parameter, you get a compile error.

**The burn:** The error messages when a system parameter is invalid are notoriously hard to read. They involve deeply nested trait bounds. If you see an error about `SystemParam` or `IntoSystem`, check that every parameter is a valid Bevy system parameter type.

### `DefaultPlugins` Includes a Lot

```rust
app.add_plugins(DefaultPlugins);
```

This single line adds: windowing, rendering, input handling, asset loading, audio, UI, diagnostics, time management, transform propagation, and more. It's ~20 plugins.

**The burn:** If you only want a headless simulation (e.g., for a server), `DefaultPlugins` opens a window. Use `MinimalPlugins` instead.

### `default()` Comes from the `Default` Trait

```rust
Sprite {
    color: Color::WHITE,
    ..default()  // fills remaining fields with Default values
}
```

`default()` is shorthand for `Default::default()`, brought in by the prelude. The `..` syntax is Rust's struct update syntax.

**The burn:** If a field's type doesn't implement `Default`, this fails. The error points at `default()` rather than telling you which specific field is the problem.

### Bevy's Plugin `build()` Runs at App Construction

```rust
impl Plugin for MyPlugin {
    fn build(&self, app: &mut App) {
        // This runs when add_plugins() is called, BEFORE app.run()
    }
}
```

**The burn:** `build()` executes during app setup, not during the game loop. Resources inserted here are available immediately. But if one plugin depends on a resource from another, the order of `add_plugins()` calls matters.

---

## Rust: Lifetime and Borrow Conventions

### Lifetime Elision

The compiler infers lifetimes in function signatures by following three rules:

```rust
fn foo(s: &str) -> &str { s }
// The compiler infers this as:
fn foo<'a>(s: &'a str) -> &'a str { s }
```

**The burn:** When elision can't figure it out, you get lifetime errors that seem to come from nowhere. The fix is adding explicit lifetime annotations, but the error messages don't always make clear which lifetime needs annotating.

### Closures Capture by Inference

Closures automatically decide whether to capture variables by reference, mutable reference, or by move:

```rust
let name = String::from("hello");
let closure = || println!("{name}");  // captures &name (immutable borrow)
let closure = || { drop(name); };     // captures name by move
```

**The burn:** You can't always tell from reading a closure whether it borrows or moves. Add the `move` keyword to force move capture: `move || println!("{name}")`.

---

## Cargo: Commands with Hidden Behavior

| Command | Hidden behavior |
|---------|----------------|
| `cargo build` | Creates/updates `Cargo.lock`, creates `target/` directory, runs `build.rs` |
| `cargo test` | Compiles with `cfg(test)` enabled, runs `#[test]` functions it auto-discovers, also runs doc-tests found in `///` comments |
| `cargo doc` | Extracts `///` and `//!` comments as documentation, renders as HTML |
| `cargo clippy` | Runs the compiler plus extra lint passes — can catch different things than `cargo build` |
| `cargo fmt` | Reads `rustfmt.toml` if present (another implicit config file) |
| `cargo publish` | Reads `Cargo.toml` metadata, packages based on `.gitignore` and include/exclude rules |

### `cfg(test)` Conditional Compilation

```rust
#[cfg(test)]
mod tests {
    // This entire module only exists during `cargo test`
    // It is invisible to `cargo build`
}
```

**The burn:** Code inside `#[cfg(test)]` can import test-only dependencies and use `#[test]` attributes. But if you accidentally put non-test code inside a `cfg(test)` block, it vanishes from your binary with no warning.

---

## Summary: Where to Look When Something is "Magic"

| Symptom | Likely convention |
|---------|------------------|
| "Where does this target come from?" | Hardcoded directory: `src/`, `examples/`, `tests/`, `benches/` |
| "Where does this type/method come from?" | Prelude import or trait in scope |
| "Why does this method exist on this type?" | Deref coercion or auto-referencing |
| "Why did adding a semicolon break everything?" | Implicit return |
| "Where is this trait implementation?" | `#[derive(...)]` macro |
| "Why does this code run at compile time?" | `build.rs` |
| "Why is my code missing from the binary?" | `#[cfg(test)]` |
| "Why can't I use this type in a Query?" | Missing `#[derive(Component)]` |
| "What does `DefaultPlugins` include?" | ~20 plugins; check Bevy docs |
| "Why does assignment not move?" | Type derives `Copy` |
