# Case Study: Implementing Original Legend of Zelda in Bevy

The original Zelda (1986) is deceptively simple visually but has real architectural depth. This document breaks down how it maps to Bevy's ECS.

## Core Systems

### 1. Tile-based World

The overworld is a grid of screens, each screen is a grid of tiles. No scrolling within a screen - the camera snaps to the next screen when Link walks off the edge.

- A `Tilemap` resource or entity holding tile data (walkable, wall, water, etc.)
- A `CurrentScreen` resource tracking which screen the player is on
- A screen transition system that detects when the player crosses an edge, swaps the tilemap, and repositions the player on the opposite side

### 2. Player

Link has 4-directional movement (half-tile aligned, not strictly tile-to-tile). Components: `Player` marker, `Position`, `Direction`, `Health` (hearts), `Inventory`. Systems for input handling, movement with collision against the tilemap, and attack (sword swing spawns a hitbox entity for a few frames).

### 3. Enemies

Each enemy type is a simple state machine:

- **Octoroks**: walk in a direction, pause, shoot a projectile
- **Moblins**: walk toward player, shoot arrows
- **Keese** (bats): fly in random diagonal patterns
- **Darknuts**: walk in patterns, only vulnerable from certain directions

Each enemy has an `EnemyBehavior` enum component. One system per behavior pattern, or one system that matches on the enum variant.

### 4. Combat

Sword swings and projectiles are entities with short lifetimes. A collision system checks overlap between hitbox entities and damageable entities. Damage events flow to health, health depletion flows to death/despawn.

Event architecture applies directly: combat sends `DamageEvent`, health plugin reads it, audio plugin reads it for the hit sound, visual plugin reads it for the flash effect.

### 5. Items and Inventory

Items on the ground are entities with `Item` components. Picking them up removes the entity and modifies the player's `Inventory` resource. Some items gate progression (raft, ladder, specific weapons). The inventory is a resource because there's only one player.

### 6. Dungeons

Structurally identical to the overworld (screens of tiles) but with locked doors (key component check), pushable blocks (tile state mutation), and boss rooms (spawn boss entity on room entry). A `WorldType` state enum switches between overworld and dungeon.

### 7. HUD

Reads `Health`, `Inventory`, and `Rupees` resources/components and renders hearts, item slots, and counters. Pure read-only display systems. Updates every frame.

## Build Order

Build vertically. Each step is a playable game. Never build infrastructure for a future payoff.

1. **Link in an empty room** - window, sprite, 4-directional movement with keyboard input. Game loop, rendering, and input.
2. **Add walls** - tilemap with collision. Link can't walk through walls. World geometry.
3. **Screen transitions** - multiple rooms, walk off the edge to move between them. A world.
4. **One enemy type** - Octorok that walks and shoots. Entities with AI and projectiles.
5. **Combat** - sword hitbox, damage, enemy death. A game.
6. **Items** - heart drops, rupees, one special item. Progression.
7. **HUD** - health display, item display. UI.
8. **Dungeons** - locked doors, keys, boss. Structure.

## Plugin Structure

```
src/
├── player/        # input, movement, sword attack
├── world/         # tilemap, screen transitions, collision
├── combat/        # hitboxes, damage events, health
├── enemies/       # AI behaviors, spawning per room
├── items/         # pickups, inventory
├── hud/           # health hearts, item display, rupees
└── audio/         # sound effects reacting to game events
```

Each plugin owns its domain. Cross-domain communication happens through events: combat sends `DamageEvent`, player sends `ItemPickedUp`, world sends `ScreenTransition`.

## Why It's Harder Than Pong

The jump isn't in any single system - it's in the number of systems that interact. Pong has ~5 systems with 2 entity types. Zelda has 30+ systems with a dozen entity types and cross-cutting concerns (combat affects enemies, player, projectiles, and items). Plugin boundaries and events prevent this from becoming a tangled mess.

## Realistic Assessment

A complete Zelda clone is weeks of work, not hours. But a "Link walks between rooms and fights Octoroks" prototype is achievable after Pong and Breakout.

Progression: Pong -> Breakout -> top-down Zelda-like prototype -> flesh it out.
