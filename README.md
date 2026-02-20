# Sean's Arcade

This file contains **decisions only**. Analysis, rationale, alternatives considered, and implementation details live in the `docs/` folder. Do not add explanations, justifications, or discussion here — state what was decided and link to the relevant document for context. This keeps the README scannable and prevents it from becoming a duplicate of the documentation.

## Decisions

### Product
- The product is **Sean's Arcade**, hosted at **seanshubin.com** (owned, AWS)
- Anyone can download the application from the website — no accounts, no login, no signup
- The application is built with **Bevy**, compiled to a single Rust binary, distributed as a direct download from S3
- First application is a **drop-in/drop-out chat room**
- Chat is the starting point because it exercises the full infrastructure without game complexity
- Evolution: Chat → Chat + Pong → Game library → Persistence ([plan](docs/project-overview.md))

### Host Model
- The first user to launch the application becomes the **host**
- Every subsequent user connects as a peer
- Peers are ordered by join time — this is the **succession order**
- When the host leaves, the next peer in the succession list becomes the new host
- The succession list is shared with all peers so everyone agrees on the order

### AWS (Global Coordination)
- Global coordination is **minimized** — AWS handles only what individual peers cannot
- AWS serves the **static website** (S3, optionally CloudFront)
- AWS runs a **presence registry** — answers "who is the current host?"
- AWS runs an always-on **relay** on a cheap cloud VM (e.g., AWS Lightsail, ~$3.50/month) for NAT traversal
- All peers (including the host) make **outbound connections** to the AWS relay — no port forwarding, no UPnP, no STUN/TURN
- The host role is **logical** (succession, message ordering) even though packets flow through the AWS relay
- Relay protocol is **UDP** — plain socket application, no HTTP, no WebSocket ([details](docs/network-operations.md))
- Presence registry uses **S3 polling** (HEAD/PUT/GET) — no dedicated API, no Lambda ([details](docs/architecture-decisions.md))
- Relay access is **invite-only** — only people the operator is in contact with can obtain access; mechanism TBD
- Identity is a **self-chosen display name** — no cryptographic identity, no uniqueness enforcement
- Estimated total cost for 0-10 users: **~$5/month** ([cost details](docs/network-operations.md#cost-estimate-0-10-users))

### Chat (v1)
- Messages are plain text with sender name and timestamp
- No server-side chat history — joining peers see messages from the moment they connect forward
- Each client keeps its own local scrollback
- Identity is a self-chosen display name, stored locally, no uniqueness enforcement

### Distribution
- **Windows-only** for v1 — add platforms when needed, but design is cross-platform from the start
- Single binary per platform, **self-replacing auto-update** — no separate launcher, no installer
- Version source of truth: `https://seanshubin.com/version` (git commit hash) — shared across all platforms
- The application has a **compiled-in commit hash** (embedded at build time) checked on startup against the remote version
- If versions match → proceed normally
- If versions differ → **auto-update**: download the platform-specific binary, replace self, restart
- If version check fails (no internet) → **offline mode**: launch with current version, show offline indicator, retry periodically until reachable
- The **relay enforces version as a backstop** — rejects clients with mismatched versions on connect
- **Download URL is platform-specific** — the binary knows its own target at compile time (e.g., `seans-arcade-windows.exe`, `seans-arcade-macos`, `seans-arcade-linux`)
- **Self-replacement varies by platform** — Windows requires a rename dance (can't delete running exe); macOS/Linux can overwrite directly
- **Builds via GitHub Actions CI** — push to `main` triggers parallel native builds (Windows, macOS universal, Linux); deploy job uploads all binaries to S3, then updates the version file as the atomic "go" signal
- **All platforms distributed as bare binaries**, not platform-specific bundles (no `.app`, no `.AppImage`)
- Bevy's `enhanced-determinism` feature flag **enabled for all builds** — required for cross-platform lockstep (forces `libm` software math)
- Bevy's `dynamic_linking` feature **disabled for all release builds** (breaks macOS and WASM)
- No differential/patch updates — full binary download every time
- No rollback — publish a new version with a higher number
- No code signing for v1 (required for macOS when that platform is added)
- **Live update for running clients** — relay polls the version file (~30s), notifies all connected clients when a new version appears
- Clients download the new binary **in the background** while the game continues — downtime is sub-second (restart only)
- **Grace period** (~5 minutes) after a new version — relay accepts both old and new version connections, then hard-rejects the old version
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

## Decisions Needed

- **Version check retry interval** — fixed interval, exponential backoff, or something else? What starting duration?
- **Chat history** — should later versions offer opt-in history stored in S3

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
