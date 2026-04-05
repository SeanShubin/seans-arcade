# Robot Character Design

Procedural robot characters as the entity system for the open world game. All characters are mechanical — no humans, no organic creatures.

## Why Robots

**Procedural generation strengths:**
- Modular parts snap together without needing to look "natural"
- Mechanical joints have simple, predictable animation (rotate, extend, spin)
- Parametric variety is trivial: change limb count, body shape, wheel size = different robot
- Damage visualization is straightforward: missing parts, bent limbs, sparks, exposed internals
- No uncanny valley — slightly wrong proportions read as "different model" not "creepy"

**Design language strengths:**
- Players intuit capabilities from shape (BattleBots precedent)
- Wide base = stable. Big arm = strong but slow. Treads = rough terrain. Glowing = energy.
- Mechanical design communicates function visually without explanation

## Design Principle: Physical Intuition Over Realism

Reality is the baseline. Players understand gravity, inertia, leverage, fragility. Science fiction concepts are welcome (hover propulsion, energy weapons, force fields) but they must not violate the player's physical intuition.

**Examples of acceptable violations:**
- A robot hovers (no real tech does this silently) — but it still looks light, moves smoothly, and gets pushed by explosions
- A beam weapon has no projectile — but it has recoil, a visible origin, and a clear firing direction
- A shield absorbs damage — but it flickers under load, has a visible boundary, and eventually breaks

**Examples of unacceptable violations:**
- A tiny robot carries something visually massive with no consequence
- A heavy robot accelerates instantly
- Damage has no visible effect
- A weapon fires from somewhere other than where it appears to be

The test: can a player who has never read a tooltip look at a robot and correctly guess what it does and how it behaves?

## Body Plan as Data

A robot is a tree of parts. Each part has a type, attachment point, and animation parameters.

```
Chassis (shape, size, mass)
  ├── Locomotion[] (type, position, size)
  │     wheels, treads, legs, hover pads
  ├── Limb[] (joint_type, segments, end_effector)
  │     arms, cranes, tentacles, pincers
  ├── Turret[] (rotation_speed, mount_type)
  │     weapons, sensors, utility
  └── Accessory[] (type, position)
        antenna, lights, armor plates, exhaust
```

### Part Types

**Chassis shapes:** box (tank), cylinder (barrel bot), wedge (flipper), disc (spinner), hexagonal (insectoid)

**Locomotion:**
- Wheels — fast on flat, poor on rough, simple animation (spin)
- Treads — slower, handles any terrain, animation = rolling belt texture
- Legs (2/4/6) — versatile, complex animation, can step over obstacles
- Hover — fast, ignores terrain, vulnerable to knockback, visual = glow/particles underneath

**Limbs:**
- Rigid arm (1-3 segments, rotary joints) — punching, grabbing, holding weapons
- Piston arm (extends/retracts) — hammering, flipping
- Tentacle (many segments, cable-like) — flexible reach, wrapping

**End effectors:**
- Claw/pincer — grab, crush
- Blade/saw — melee damage, spin animation
- Drill — directional melee, spin animation
- Hand/gripper — hold objects, operate switches
- Weapon mount — attach ranged weapon

**Weapons (turret-mounted or limb-mounted):**
- Projectile (bullets, missiles) — visible travel, arc for heavy projectiles
- Beam (laser, plasma) — instant hit, visible line, power drain
- Melee (hammer, spinner, flipper) — close range, big visual impact

## Animation Approach

All animation is parametric — functions of time and state, no keyframes.

**Locomotion animation:**
- Wheels: `rotation += speed * dt`
- Treads: UV offset on tread texture
- Legs: inverse kinematics to ground contact points, step cycle from movement speed
- Hover: `y_offset = base + sin(time * bob_freq) * bob_amplitude`

**Limb animation:**
- Joint angles interpolated with easing functions
- State machine: idle → reach → grab → retract
- Each state is a target pose, animation is `lerp(current, target, eased_t)`

**Damage animation:**
- Part detachment: part becomes a physics object, falls away
- Sparks: particle emitter at damage point
- Limping: locomotion animation parameters degrade (shorter step, slower speed, wobble)

## Variety Generation

A robot's identity is its part list + parameters. Two robots with different part configurations look and behave differently without any art.

**Variety knobs:**
- Chassis shape and size (5 shapes × continuous size = infinite)
- Locomotion type and count (4 types × 1-6 count)
- Limb count and configuration (0-4 limbs × segment count × end effector)
- Color/material (procedural — base color + accent + wear/rust level)
- Asymmetry (left/right can differ — looks battle-scarred or specialized)

**Faction/class through silhouette:**
- Scout: small chassis, wheels/hover, no arms, sensor turret
- Brawler: large chassis, treads, heavy arms, melee weapons
- Sniper: medium chassis, legs, one long-barrel turret
- Support: medium chassis, multiple utility arms, no weapons

The player learns to read robot class from shape at a distance, before details are visible.

See also: [procedural-animation.md](procedural-animation.md) for the animation technique foundation.
