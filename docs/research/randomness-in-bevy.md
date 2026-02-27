# Randomness in Bevy

## Types of Randomness

There are three categories of random number generation, and they differ in how hard they are to predict — not in whether they produce "true" randomness.

**Pseudo-Random (PRNG).** A deterministic algorithm seeded by an initial value. Fast, reproducible given the same seed, and perfectly adequate for games. If you know the seed and algorithm, you can predict every output. Examples: WyRand, PCG, Xoshiro.

**Cryptographically Secure (CSPRNG).** A deterministic expansion of a small truly-random seed. The OS collects entropy from hardware sources (keystroke timing, disk seeks, interrupt timing), feeds it into a strong algorithm (ChaCha20, AES-CTR), and stretches it into a long output stream. Security comes from the math of the expansion — breaking it would require solving problems believed to be computationally infeasible. No adversary with bounded computational resources can distinguish the output from true randomness. Examples: OsRng, ThreadRng.

**True Random.** Physical phenomena: radioactive decay, thermal noise, photon behavior. Fundamentally unpredictable per quantum mechanics. Hardware RNGs like Intel's RDRAND harvest this. Overkill for nearly everything — CSPRNGs provide equivalent practical guarantees.

CSPRNGs don't need true randomness to be secure. They need three properties: unpredictability (can't guess the next bit from prior outputs), backtracking resistance (compromised state doesn't reveal past outputs), and state compromise recovery (re-secures itself with fresh entropy after a leak). These are computational guarantees, not information-theoretic ones.

## Bevy's RNG Landscape

Bevy has no built-in RNG. The community standard is **`bevy_rand`**, a third-party plugin by Bluefinger that wraps the `rand` ecosystem into ECS-friendly components with reflection, serialization, and deterministic seed forking.

### Algorithm Selection

| Algorithm                    | Speed    | When to Use                                                                                    |
| ---------------------------- | -------- | ---------------------------------------------------------------------------------------------- |
| **WyRand**                   | Fastest  | General game logic — spawning, AI, damage rolls, proc-gen. Right choice for almost everything. |
| **ChaCha8Rng**               | Fast     | Competitive multiplayer where players might try to predict RNG.                                |
| **ChaCha20Rng**              | Moderate | Maximum PRNG unpredictability. Still not cryptographic.                                        |
| **OsRng** (raw `rand` crate) | Slowest  | Actual cryptographic needs — tokens, auth, keys. Use directly, not through `bevy_rand`.        |

### Key Types

**`GlobalRng`** — a marker component on a singleton entity. The root source of randomness for the app. Accessed via `Single<&mut WyRand, With<GlobalRng>>`.

**`ForkableSeed`** — deterministically derives child RNG seeds from a parent. Critical for replays, netcode, and parallel systems. The parent seed produces the same child seeds every time, so forking is reproducible.

**`Entropy`** — per-entity RNG components forked from the global source. Enable parallel system execution because each entity has its own independent RNG state.

**`EntropyPlugin<T>`** — plugin registration. You choose your algorithm at plugin setup time.

### The Parallelism Tradeoff

Any system that mutably accesses `GlobalRng` forces serial execution — Bevy's scheduler can't run two systems that both want `&mut WyRand` on the same entity. The recommended pattern:

1. Fork seeds from `GlobalRng` at setup time into per-entity components.
2. Use per-entity RNGs during gameplay.
3. Determinism is preserved because the fork order is fixed at setup, and each entity's RNG evolves independently.

This is the same principle behind deterministic lockstep networking: if every entity has its own RNG seeded deterministically from a shared root, two clients with the same initial seed will produce identical simulations regardless of system execution order.

### Setup

```toml
# Cargo.toml
rand_core = "0.9"
bevy_rand = { version = "0.14", features = ["wyrand"] }
```

```rust
use bevy::prelude::*;
use bevy_prng::WyRand;
use bevy_rand::prelude::EntropyPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EntropyPlugin::<WyRand>::default())
        .run();
}
```

### Usage

**Global RNG access (simple, serial):**
```rust
fn roll_damage(mut rng: Single<&mut WyRand, With<GlobalRng>>) {
    let damage = rng.next_u32() % 20 + 1;
}
```

**Forking per-entity RNG (parallel, deterministic):**
```rust
fn setup_enemies(mut commands: Commands, mut global: Single<&mut WyRand, With<GlobalRng>>) {
    for _ in 0..10 {
        commands.spawn((Enemy, global.fork_seed()));
    }
}
```

**Shape sampling with `bevy_math`:**
```rust
fn random_spawn_point(mut rng: Single<&mut WyRand, With<GlobalRng>>) {
    let circle = Circle::new(500.0);
    let point = circle.sample_interior(rng.as_mut());
}
```

### Version Compatibility

| Bevy | bevy_rand   |
| ---- | ----------- |
| 0.18 | 0.13 – 0.14 |
| 0.17 | 0.12        |
| 0.16 | 0.10 – 0.11 |
| 0.15 | 0.8 – 0.9   |
