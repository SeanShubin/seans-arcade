# Sean's Arcade — Project Overview

A website at **seanshubin.com** where anyone can download the Sean's Arcade application. The application connects to a lightweight relay server on AWS that coordinates all communication. Clients never talk to each other directly — everyone makes outbound connections to the relay. Global coordination is minimal: AWS hosts the relay, a version file, and binary downloads.

## Starting Point: Chat

The first application is a drop-in/drop-out chat room. No accounts, no login, no signup. Download, launch, talk.

Chat is the right starting point because it exercises the full infrastructure — relay connection, peer discovery, message forwarding — without the complexity of deterministic simulation, tick synchronization, or latency hiding. Once chat works end-to-end, games are an incremental addition on top of the same foundation.

### Message Flow (Chat)

```
User types message
    → Client sends to relay (AWS)
    → Relay broadcasts to all connected clients
    → Each client displays the message
```

Messages are plain text with a sender name and timestamp. No history is stored on the relay (stateless). Chat history is world state — joining peers receive it as part of the S3 save download, the same way they'd receive player positions in a game.

## What the Application Looks Like (v1 — Chat)

A simple window with:
- A text area showing chat messages (sender name + message)
- An input field at the bottom for typing
- A status bar showing: connection state, your name, number of connected peers, whether you're the host
- A name picker on first launch (stored locally for next time)

The entire application is built with Bevy, including v1 chat. Bevy's ECS and rendering pipeline are used from the start, so there's no framework migration when games are added later.

## Evolution Path

| Phase | What's added | What it exercises |
|-------|-------------|-------------------|
| **v1: Chat** | Drop-in text chat | Relay connection, peer discovery, message forwarding, AWS coordination |
| **v2: Chat + Pong** | Side-by-side chat and game lobby. Two peers can launch a pong match from within the app. | Deterministic lockstep, latency hiding, game state alongside chat |
| **v3: Game library** | Multiple game options in the lobby. Chat persists across games. | Game-agnostic relay, modular game loading |
| **v4: Persistence** | Game state saved to S3 between sessions | Cloud storage, tick-based sync protocol |

Each phase builds on the previous infrastructure. The chat channel persists through all phases — it becomes the social backbone of the arcade.

## Open Questions

See [Decisions Needed](../README.md#decisions-needed) in the README (canonical list of pending decisions).

## Document Index

### Architecture & Infrastructure
- [architecture-decisions.md](architecture-decisions.md) — Formalized technical decisions with rationale
- [network-architecture.md](network-architecture.md) — Lockstep relay networking model: architecture and concepts
- [network-operations.md](network-operations.md) — Running, debugging, and maintaining networked games; AWS infrastructure and cost estimates
- [distribution.md](distribution.md) — Distribution, versioning, CI pipeline, and auto-update
- [game-engine-anatomy.md](game-engine-anatomy.md) — How a game engine is structured

### Game Design
- [design-philosophy.md](design-philosophy.md) — Core game design principles
- [avatar-control-and-game-feel.md](avatar-control-and-game-feel.md) — Making controls feel responsive
- [pain-before-relief.md](pain-before-relief.md) — Pacing solutions after the player feels the problem
- [discovered-contract.md](discovered-contract.md) — Rules as discovery content
- [emergent-gameplay-and-progression.md](emergent-gameplay-and-progression.md) — Multiple progression axes and emergent interactions
- [meaningful-choice-analysis.md](meaningful-choice-analysis.md) — What makes choices feel meaningful (BG3 vs Cyberpunk)
- [progression-and-difficulty-design.md](progression-and-difficulty-design.md) — Progression gating and player-directed difficulty
- [procedural-vs-authored-design.md](procedural-vs-authored-design.md) — Tradeoffs between procedural and hand-crafted content
- [reference-games.md](reference-games.md) — Games studied for design lessons
- [classic-game-candidates.md](classic-game-candidates.md) — Candidate classic games for the arcade
- [zelda-case-study.md](zelda-case-study.md) — Zelda design analysis
- [design-topics-to-explore.md](design-topics-to-explore.md) — Design topics for future research

### Development & Tooling
- [project-setup.md](project-setup.md) — Project setup and configuration
- [learning-plan.md](learning-plan.md) — Learning roadmap
- [pong-step-by-step.md](pong-step-by-step.md) — Step-by-step pong implementation
- [architectural-rules-in-ecs.md](architectural-rules-in-ecs.md) — Architectural rules for ECS codebases
- [static-analysis-for-rust.md](static-analysis-for-rust.md) — Static analysis tooling for Rust
- [rust-implicit-conventions.md](rust-implicit-conventions.md) — Rust implicit conventions
- [keeping-dependencies-updated.md](keeping-dependencies-updated.md) — Dependency management
- [gilrs-dual-gamepad-bug.md](gilrs-dual-gamepad-bug.md) — Gilrs dual gamepad bug investigation
- [non-programming-skills.md](non-programming-skills.md) — Non-programming skills for game development
