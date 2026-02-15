# Bevy Game Development Learning Plan

## Starting Point

- Experienced business application developer (cloud, frontend, backend)
- IntelliJ IDEA Ultimate Edition available
- Rust toolchain installed (rustc 1.88.0, cargo 1.88.0)
- This repository is a sandbox for learning game development with Rust and Bevy
- Learns best from examples and short descriptions, not sequential documentation
- Strict about code quality; maintains custom static analysis tooling and architectural rules

## 1. IDE Setup

RustRover (free, JetBrains) is the recommended IDE since the IntelliJ ecosystem is already familiar. Download the standard Windows (x86-64) installer. The Rust plugin for IntelliJ IDEA Ultimate is another option. VS Code with rust-analyzer is a fallback.

Note: RustRover non-commercial license covers personal learning and hobby projects. If Rust becomes part of professional work or a game is sold, a paid license is required. Check whether existing JetBrains subscription already includes RustRover.

## 2. Learn Rust

### Learning Approach

Skip sequential documentation. Use example-driven resources:

- **Rust by Example**: https://doc.rust-lang.org/rust-by-example/ - short concept, concrete code, try it
- **Bevy official examples**: clone the Bevy repo, run with `cargo run --example <name>`, read and modify
- Look up specific Rust Book chapters only when hitting a concept you need explained (ownership, lifetimes, trait bounds)

### Key Concepts (ordered by importance for Bevy)

1. **Ownership & borrowing** (Rust Book ch 4) - the big paradigm shift from JVM/JS/TS
2. **Enums & pattern matching** (ch 6) - used heavily in game logic
3. **Traits** (ch 10) - Rust's approach to polymorphism, central to Bevy's design
4. **Error handling with Result/Option** (ch 9) - no exceptions in Rust

## 3. First Bevy Steps

### Resources

- Bevy Book: https://bevyengine.org/learn/book/introduction/
- Official examples: clone the Bevy repo and run with `cargo run --example <name>`

### Core Mental Model: ECS (Entity Component System)

- **Entities** = IDs (like a row ID in a database)
- **Components** = data attached to entities (like columns)
- **Systems** = functions that query and transform components (like queries/stored procedures)

ECS is essentially a relational data model with reactive query functions. This maps well to business application thinking.

## 4. Progressive Project Ideas

1. **Bouncing ball** - window, sprite, movement system, collision with walls
2. **Pong clone** - input handling, two entities interacting, score state
3. **Breakout clone** - spawning/despawning entities, levels, UI overlay
4. **Top-down shooter** - camera, sprites, projectile systems, enemy AI
5. **Tile-based RPG** - tilemaps, inventory state, scene transitions

## 5. Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-02-14 | Created sandbox repository | Dedicated space for game dev experimentation |
| 2026-02-14 | Plan: Rust fundamentals before Bevy | Ownership errors in Bevy code are confusing without borrow checker familiarity |
| 2026-02-14 | Example-driven learning over sequential docs | Matches learning style; Rust by Example preferred over Rust Book cover-to-cover |
| 2026-02-14 | Accept ECS as a different paradigm from OOP DI | ECS optimizes for throughput with different composition mechanisms; some architectural rules don't apply directly |
| 2026-02-14 | Plan to port code-structure analysis to Rust | Start with `syn` crate for AST-based dependency extraction |
