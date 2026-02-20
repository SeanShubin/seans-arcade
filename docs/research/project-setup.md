# Project Setup

## Create the project

```bash
cargo new bevy-prototyping --lib
cd bevy-prototyping
cargo add bevy
mkdir examples
```

This generates `Cargo.toml`, `src/lib.rs`, and a git repo. `cargo add` fetches the latest stable Bevy version and adds it to `Cargo.toml`.

## Add Bevy fast-compile profile

Add this to `Cargo.toml` to speed up dev builds. Bevy has many dependencies - without this, incremental compiles are noticeably slower:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
```

## Create an experiment

Add a file to `examples/` with a `fn main()`:

```rust
// examples/my_experiment.rs
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}
```

Pure Rust experiments don't need Bevy imports:

```rust
// examples/my_experiment.rs
fn main() {
    println!("hello");
}
```

## Run an experiment

```bash
cargo run --example my_experiment
```

## Shared code

If you find yourself repeating helpers across experiments, put them in `src/lib.rs` and import them:

```rust
use bevy_prototyping::my_helper;
```
