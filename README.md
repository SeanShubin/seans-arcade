# Sean's Arcade

This file contains **decisions only**. Analysis, rationale, alternatives considered, and implementation details live in the `docs/` folder. Do not add explanations, justifications, or discussion here — state what was decided and link to the relevant document for context. This keeps the README scannable and prevents it from becoming a duplicate of the documentation.

## Decisions

### Product
- The product is **Sean's Arcade**, hosted at **seanshubin.com** (owned, AWS)
- The application is downloaded from the website
- The application is built with **Bevy**, compiled to a single Rust binary, distributed as a direct download from S3
- First application is a **drop-in/drop-out chat room**
- Chat is the starting point because it exercises the full infrastructure without game complexity
- Evolution: Chat → Chat + Pong → Game library → Persistence ([plan](docs/project-overview.md))

### AWS (Global Coordination)
- Global coordination is **minimized** — AWS handles only what individual peers cannot
- AWS serves the **static website** (S3, optionally CloudFront)
- AWS runs an always-on **relay** on a cheap cloud VM (e.g., AWS Lightsail, ~$3.50/month) for NAT traversal
- All clients make **outbound connections** to the AWS relay — no port forwarding, no UPnP, no STUN/TURN
- Relay protocol is **UDP** — plain socket application, no HTTP, no WebSocket ([details](docs/network-operations.md))
- Relay access is **invite-only** via **shared secret in the Hello handshake** — operator distributes a passphrase out-of-band, relay rejects connections without it ([decision](docs/architecture-decisions.md))
- Hello handshake carries **commit hash, shared secret, and display name** — one message, one round-trip; relay validates secret, groups by version, tracks identity ([decision](docs/architecture-decisions.md))
- Identity is a **self-chosen display name** — no cryptographic identity, no uniqueness enforcement
- **Fixed 30-second retry** for all connectivity failures — version check, relay connection, S3 sync ([decision](docs/architecture-decisions.md))
- Estimated total cost for 0-10 users: **~$5/month** ([cost details](docs/network-operations.md#cost-estimate-0-10-users))

### Chat (v1)
- Chat messages are **game inputs** (opaque payload) — the relay never distinguishes chat from any other input type
- Messages are plain text with sender name and timestamp
- Chat history is **world state** — persisted, compacted, and restored via the same S3 save infrastructure as game state
- Joining peers receive chat history as part of the S3 save download, same as player positions in a game
- Bounding of chat history (last N messages, session-only, unlimited) is a **game mechanic**, not an infrastructure decision
- Identity is a self-chosen display name, stored locally, no uniqueness enforcement
- Local config (display name, relay secret) stored in **platform app data directory** (`%APPDATA%\seans-arcade\config.toml` on Windows) ([decision](docs/architecture-decisions.md))

### Arcade Model (v2+)
- The arcade is the **main application** — chat is the always-on social layer, games are sub-applications within it
- Chat is **always visible** to everyone — playing or watching a game does not leave the chat
- Games appear in a **game screen within the chat interface** — chat is the lobby
- Any player can **start a game and invite others**; any player can **spectate** any game
- **Multiple games run simultaneously** with different player subsets
- Each game is its own **lockstep session** with its own tick stream and player group
- The relay **multiplexes** one connection per client across chat and concurrent game sessions

### Distribution
- **Windows-only** for v1 — add platforms when needed, but design is cross-platform from the start
- Single binary per platform, **self-replacing auto-update** — no separate launcher, no installer
- Version source of truth: `https://seanshubin.com/version` (git commit hash) — shared across all platforms
- The application has a **compiled-in commit hash** (embedded at build time) checked on startup against the remote version
- If versions match → proceed normally
- If versions differ → **auto-update**: download the platform-specific binary, replace self, restart
- If version check fails (no internet) → **offline mode**: launch with current version, show offline indicator, retry periodically until reachable
- The **relay isolates clients by version** — clients with different commit hashes cannot interact
- **Download URL is platform-specific** — the binary knows its own target at compile time (e.g., `seans-arcade-windows.exe`, `seans-arcade-macos`, `seans-arcade-linux`)
- **Self-replacement varies by platform** — Windows requires a rename dance (can't delete running exe); macOS/Linux can overwrite directly
- **Builds via GitHub Actions CI** — push to `main` triggers parallel native builds (Windows, macOS universal, Linux); deploy job uploads all binaries to S3, then updates the version file as the atomic "go" signal
- **All platforms distributed as bare binaries**, not platform-specific bundles (no `.app`, no `.AppImage`)
- Bevy's `enhanced-determinism` feature flag **enabled for all builds** — required for cross-platform lockstep (forces `libm` software math)
- Bevy's `dynamic_linking` feature **disabled for all release builds** (breaks macOS and WASM)
- No differential/patch updates — full binary download every time
- No rollback — publish a new version with a higher number
- No code signing for v1 (required for macOS when that platform is added)
- Running clients **continue on their current version** until relaunch — no mid-session updates, no background downloads
- The relay groups clients by commit hash — **multiple versions coexist** independently; each version group operates in isolation
- The relay treats game inputs as **opaque bytes** — only protocol-level changes (message framing, handshake) require relay redeployment; game logic changes are relay-transparent
- ([mechanism details](docs/distribution.md))

### Networking (Games, v2+)
- **Deterministic lockstep** with relay server ([architecture](docs/network-architecture.md))
- **Latency hiding**, not rollback ([decision](docs/architecture-decisions.md))
- Relay **drops** slow inputs rather than stalling ([decision](docs/architecture-decisions.md))
- Determinism via **constrained f32 + libm** for transcendentals, enforced by Bevy's `enhanced-determinism` feature flag ([decision](docs/architecture-decisions.md))
- **Non-cryptographic checksums** (xxhash) for drift detection ([decision](docs/architecture-decisions.md))
- **Full message logging**, always on ([decision](docs/architecture-decisions.md))
- **Deterministic replay** from input logs, no game state logging ([decision](docs/architecture-decisions.md))
- Persistence via **S3**, not always-on servers ([decision](docs/architecture-decisions.md))
- **Tick-based sync** protocol, no streaming infrastructure ([decision](docs/architecture-decisions.md))
- Player join via **S3 save + input log buffer** ([decision](docs/architecture-decisions.md))
- **Two-tier log model** — hot buffer on relay, cold archive in S3 ([decision](docs/architecture-decisions.md))
- **Log compaction** via periodic world state snapshots, stored separately from the input log with marker events ([decision](docs/architecture-decisions.md))
- **Persistent storage layout** organized by commit hash and session ([details](docs/network-operations.md))

## Decisions Needed

(none)

## Documentation

| Document | Contents |
|----------|----------|
| [project-overview.md](docs/project-overview.md) | Entry-point overview, evolution path, document index |
| [network-architecture.md](docs/network-architecture.md) | Lockstep relay networking model, determinism, latency hiding |
| [network-operations.md](docs/network-operations.md) | Diagnostics, debugging, deployment, AWS infrastructure, cost estimates |
| [distribution.md](docs/distribution.md) | Distribution, versioning, CI pipeline, auto-update |
| [architecture-decisions.md](docs/architecture-decisions.md) | Formalized technical decisions with rationale |
| [design-philosophy.md](docs/design-philosophy.md) | Game design principles that motivate technical decisions |
| [game-engine-anatomy.md](docs/game-engine-anatomy.md) | High-level engine pipeline overview |
