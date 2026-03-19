# Procedural Animation Learning Plan

Top-down 2D character animation using vector graphics (no sprites). The character is assembled from geometric primitives and animated through code — positions, rotations, and scales computed at runtime from math and state.

## Goals

1. **No art assets** — the character is code, not images
2. **Resolution independence** — vector graphics scale to any display
3. **Interactive** — weapons and projectiles respond to gameplay, not canned sequences
4. **Learnable** — each concept builds on the previous one with a running example

## Core Techniques

### Drawing

| Technique | What it does | When you need it |
|---|---|---|
| Gizmos (lines, circles, rects) | Immediate-mode vector drawing | Example 1 onward |
| Shape composition | Assembling a character from child entities with local offsets | Example 1 |
| Z-ordering / draw order | Controlling which parts render in front | Example 2 |

### Animation

| Technique | What it does | When you need it |
|---|---|---|
| Sine-wave oscillation | Repeating smooth motion (walk bob, arm sway) | Example 3 |
| Lerp / easing | Smooth transitions between values with controllable feel | Example 4 |
| Arc interpolation | Rotating a part around a pivot over time (sword swing) | Example 4 |
| State machine | Switching between idle/walk/attack/item states | Example 3 |

### Gameplay

| Technique | What it does | When you need it |
|---|---|---|
| Entity spawning | Creating projectile entities at runtime | Example 5 |
| Velocity + lifetime | Moving projectiles and despawning them | Example 5 |
| Return curves | Boomerang-style projectiles that curve back to the player | Example 5 |
| Squash & stretch | Brief scale changes for impact feel | Example 6 |
| Hit flash | Momentary color change on damage | Example 6 |

### Key Math

| Function | Purpose |
|---|---|
| `atan2(dy, dx)` | Angle from one point to another |
| `sin(t) / cos(t)` | Oscillation, converting angle to direction |
| `lerp(a, b, t)` | Linear interpolation between two values |
| `smoothstep(t)` | S-curve interpolation (3t² - 2t³) |
| `vec2.length() / normalize()` | Distance and direction |

## Example Progression

Each example is a standalone file in `examples/` that builds on concepts from the previous one.

### Example 1: Shape Composition (`procedural_hero_shapes.rs`)

Assemble a static top-down character from geometric primitives using Gizmos.

**Concepts:** Gizmos API, shape primitives (rect, circle, triangle), color, positioning parts relative to a character origin.

**Result:** A static hero-like figure facing down — head, body, legs, arms, shield outline. No movement, no input.

### Example 2: Facing Directions (`procedural_hero_facing.rs`)

Add arrow-key input to change the character's facing direction. Each direction rearranges which body parts are visible and their relative positions (top-down perspective means facing up hides the face, facing left/right mirrors the layout).

**Concepts:** Input reading, 4-direction state, conditional part arrangement, z-ordering.

**Result:** Press arrow keys to face 4 directions. Character parts rearrange to show the correct top-down view.

### Example 3: Walk Cycle (`procedural_hero_walk.rs`)

Add movement and a procedural walk animation. Legs oscillate with `sin(time)`, body bobs slightly, arms sway opposite to legs.

**Concepts:** Sine-wave animation, time-based motion, state machine (idle vs walking), movement speed.

**Result:** Arrow keys move the character. Walking triggers leg/arm oscillation and body bob. Stopping returns to idle pose.

### Example 4: Sword Attack (`procedural_hero_sword.rs`)

Add a sword swing on a button press. The sword rotates in an arc around a pivot point near the character's hand. A hitbox follows the sword tip during the swing.

**Concepts:** Arc interpolation, easing functions (ease-out for snappy swing), attack state with cooldown, hitbox generation from geometry.

**Result:** Press a button to swing the sword in a ~120-degree arc. The attack has a windup, swing, and recovery phase. A debug circle shows the hitbox following the sword tip.

### Example 5: Projectiles (`procedural_hero_projectiles.rs`)

Add projectile weapons: boomerang, arrow, and wand beam. Each has distinct geometry and flight behavior.

**Concepts:** Entity spawning/despawning, velocity components, lifetime timers, boomerang return curve (lerp toward player after max distance), straight-line projectiles.

**Result:** Different buttons fire different projectiles. Boomerang flies out and curves back. Arrow flies straight until max range. Wand beam is a small fast projectile.

### Example 6: Polish (`procedural_hero_juice.rs`)

Add visual feedback: squash/stretch on actions, hit flash on simulated damage, screen shake.

**Concepts:** Transient scale modification, color override with timer, camera offset oscillation.

**Result:** Sword swing briefly stretches the arm. Taking simulated damage (press a key) flashes the character white. Landing from a walk step squashes slightly. A strong hit triggers screen shake.

## Reference

- `docs/research/character-rendering.md` — comparison of rendering approaches (this is Approach F)
- Coding Train (YouTube) — conceptual walkthroughs of IK, steering, oscillation in p5.js
- "Nature of Code" by Daniel Shiffman — forces, oscillation, particle systems
