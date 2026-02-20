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

## Arcade Model

Think of a physical arcade — everyone can hear each other, and players wander around to play or watch games at will. Chat is the shared space (the lobby), and games are the machines players walk up to. Starting or joining a game doesn't take you away from the chat; it opens a game screen within it. Multiple games run simultaneously with different player groups, and anyone can spectate any game in progress. Each game is its own lockstep session with its own tick stream, while the relay multiplexes a single connection per client across chat and all concurrent game sessions. See the [decision register](decisions.md#arcade-model-v2) for the full decision list.

## What the Application Looks Like (v1 — Chat)

A simple window with:
- A text area showing chat messages (sender name + message)
- An input field at the bottom for typing
- A status bar showing: connection state, your name, number of connected peers
- A name picker on first launch (stored locally for next time)

The entire application is built with Bevy, including v1 chat. Bevy's ECS and rendering pipeline are used from the start, so there's no framework migration when games are added later.

## Evolution Path

| Phase | What's added | What it exercises |
|-------|-------------|-------------------|
| **v1: Chat** | Drop-in text chat | Relay connection, peer discovery, message forwarding, AWS coordination |
| **v2: Chat + Pong** | Pong playable within the chat interface. Two peers can start a match while others watch or chat. | Game sub-session within the arcade, spectating, deterministic lockstep, latency hiding |
| **v3: Game library** | Multiple game types. Multiple simultaneous games with different player groups. | Relay multiplexing, concurrent sessions, modular game loading |
| **v4: Persistence** | Game state saved to S3 between sessions | Cloud storage, tick-based sync protocol |

Each phase builds on the previous infrastructure. Chat is the always-on social layer — it doesn't persist "across" games so much as games exist within it. The chat is the arcade; games are what you do there.

## Open Questions

See [Decisions Needed](decisions.md#decisions-needed) in the decision register (canonical list of pending decisions).

## Document Index

### Decisions
- [decisions.md](decisions.md) — Decision register (terse bullets, links to rationale)
- [architecture-decisions.md](architecture-decisions.md) — Formalized technical decisions with rationale

### Architecture
- [network-architecture.md](architecture/network-architecture.md) — Lockstep relay networking model: architecture and concepts
- [network-operations.md](architecture/network-operations.md) — Running, debugging, and maintaining networked games; AWS infrastructure and cost estimates
- [distribution.md](architecture/distribution.md) — Distribution, versioning, CI pipeline, and auto-update
- [game-engine-anatomy.md](architecture/game-engine-anatomy.md) — How a game engine is structured

### Research — Game Design
- [design-philosophy.md](research/design-philosophy.md) — Core game design principles
- [avatar-control-and-game-feel.md](research/avatar-control-and-game-feel.md) — Making controls feel responsive
- [pain-before-relief.md](research/pain-before-relief.md) — Pacing solutions after the player feels the problem
- [discovered-contract.md](research/discovered-contract.md) — Rules as discovery content
- [emergent-gameplay-and-progression.md](research/emergent-gameplay-and-progression.md) — Multiple progression axes and emergent interactions
- [meaningful-choice-analysis.md](research/meaningful-choice-analysis.md) — What makes choices feel meaningful (BG3 vs Cyberpunk)
- [progression-and-difficulty-design.md](research/progression-and-difficulty-design.md) — Progression gating and player-directed difficulty
- [procedural-vs-authored-design.md](research/procedural-vs-authored-design.md) — Tradeoffs between procedural and hand-crafted content
- [reference-games.md](research/reference-games.md) — Games studied for design lessons
- [classic-game-candidates.md](research/classic-game-candidates.md) — Candidate classic games for the arcade
- [zelda-case-study.md](research/zelda-case-study.md) — Zelda design analysis
- [design-topics-to-explore.md](research/design-topics-to-explore.md) — Design topics for future research

### Research — Technical & Learning
- [architectural-rules-in-ecs.md](research/architectural-rules-in-ecs.md) — Architectural rules for ECS codebases
- [static-analysis-for-rust.md](research/static-analysis-for-rust.md) — Static analysis tooling for Rust
- [rust-implicit-conventions.md](research/rust-implicit-conventions.md) — Rust implicit conventions
- [keeping-dependencies-updated.md](research/keeping-dependencies-updated.md) — Dependency management
- [gilrs-dual-gamepad-bug.md](research/gilrs-dual-gamepad-bug.md) — Gilrs dual gamepad bug investigation
- [non-programming-skills.md](research/non-programming-skills.md) — Non-programming skills for game development
- [learning-plan.md](research/learning-plan.md) — Learning roadmap
- [project-setup.md](research/project-setup.md) — Project setup and configuration
- [pong-step-by-step.md](research/pong-step-by-step.md) — Step-by-step pong implementation
