# Stress Balls: Performance Case Study

## Setup

`examples/stress_balls.rs` — a configurable bouncing-ball stress test. All controls are powers of 2 to make scaling relationships obvious.

- **Ball count**: 2^n entities, each a 1×1 white-pixel sprite with tint
- **Speed**: 2^n pixels per tick (no delta-time scaling — movement is per-tick)
- **Size**: 2^n pixels
- **FixedUpdate toggle**: switches ball systems between Update (every frame) and FixedUpdate (64 Hz)
- GPU batching: all balls share one texture, so the GPU draws them in a single batch

Test machine: Windows 11, 240 Hz display with vsync.

## Observations

### 1. Vsync stair-stepping

| Balls | FPS (Update) |
|-------|-------------|
| 2^15 (32K) | 240 |
| 2^16 (64K) | ~120 |
| 2^17 (128K) | ~60 |
| 2^18 (256K) | ~30 |

Up to 32K balls, per-frame work fits within the 4.17 ms frame budget (1/240 s). After that, each doubling of balls doubles the work, pushing past the next vsync deadline. The compositor snaps to the next interval — 120, 60, 30 — producing a clean halving pattern rather than a gradual decline.

**Takeaway**: With vsync, performance doesn't degrade smoothly. It stair-steps. A workload that's 1% over budget looks the same as one that's 90% over — both drop to the next step. This makes vsync-on benchmarks misleading unless you watch frame times directly.

### 2. Per-tick movement and the 4x speed ratio

Movement applies `speed` once per tick with no `delta_secs()` scaling. When switching from Update (240 ticks/sec) to FixedUpdate (64 ticks/sec), balls move 240/64 = 3.75x slower.

Since speed is controlled in powers of 2, bumping 2 exponents (4x) is the closest compensation. In practice this looks right — the 4x multiplier slightly overshoots (4.0 vs 3.75), but the difference is imperceptible.

**Takeaway**: Per-tick movement without delta-time is frame-rate-dependent by design in this stress test, but it makes the Update/FixedUpdate ratio directly visible. In a real game, you'd multiply by `delta_secs()` so behavior is tick-rate-independent.

### 3. FixedUpdate shifts the ceiling — then fails differently

With Update at 240 Hz, the per-second work at the performance cliff is:

    32K balls × 240 ticks/sec = 7.9M ball-updates/sec

With FixedUpdate at 64 Hz, a similar budget supports ~4x more balls:

    131K balls × 64 ticks/sec = 8.4M ball-updates/sec

This explains why 2^16 and 2^17 show no degradation under FixedUpdate — the per-second computation is still within budget.

**But the failure mode is different.** With Update, exceeding the frame budget simply drops to the next vsync step. The result is smooth but slower. With FixedUpdate, when a single tick exceeds 1/64 s (15.6 ms), Bevy accumulates time debt. On the next frame it tries to run multiple catch-up ticks, each of which also exceeds the budget. This creates a death spiral: more catch-up ticks → longer frames → more debt. The visible result is choppiness and stuttering rather than a clean FPS reduction.

| Balls | Update behavior | FixedUpdate behavior |
|-------|----------------|---------------------|
| 2^15 | 240 FPS, smooth | 240 FPS, smooth |
| 2^16 | ~120 FPS, smooth | 240 FPS, smooth |
| 2^17 | ~60 FPS, smooth | 240 FPS, smooth |
| 2^18 | ~30 FPS, smooth | choppy / stuttering |

**Takeaway**: FixedUpdate lets you handle more entities before trouble starts, but when it does fail, it fails badly. Update degrades gracefully (vsync stair-steps); FixedUpdate hits a wall (tick death spiral). For a real game, FixedUpdate is still the right choice for physics — but you need to budget your fixed-tick work carefully and consider Bevy's max-ticks-per-frame cap to prevent the spiral.

## Summary

| Concept | What the stress test demonstrates |
|---------|----------------------------------|
| Vsync stair-stepping | FPS halves in discrete steps, not gradually |
| Per-tick vs per-second | No delta-time → visible 4x ratio between 240 Hz and 64 Hz |
| FixedUpdate tradeoff | ~4x entity headroom, but catastrophic failure instead of graceful degradation |
| GPU batching | Not the bottleneck here — shared texture means CPU-side systems are the limit |
