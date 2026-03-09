# Decisions

This file contains **decisions only**. Analysis, rationale, alternatives considered, and implementation details live in the rest of the `docs/` folder. Do not add explanations, justifications, or discussion here — state what was decided and link to the relevant document for context. This keeps the decision register scannable and prevents it from becoming a duplicate of the documentation.

## Decisions

### Product
- The product is **Sean's Arcade**, hosted at **seanshubin.com** (owned, AWS)
- The game client (`arcade`) is downloaded from the website
- The game client is built with **Bevy**, compiled to a single Rust binary per platform, distributed as a direct download from S3
- First application is a **drop-in/drop-out chat room**
- Chat is the starting point because it exercises the full infrastructure without game complexity
- Evolution: Chat → Chat + Pong → Game library → Persistence ([plan](project-overview.md))

### Project Structure
- Three binaries: **`arcade`** (game client, Bevy), **`relay`** (input coordinator, runs on AWS), **`arcade-ops`** (local tooling)
- The relay stays minimal — internet-facing, packet forwarding only, writes state to S3 periodically
- Admin monitoring, management, analytics, and infrastructure control via **`arcade-ops`** — one tool, no separate web dashboard ([decision](architecture-decisions.md#admin-cli-replaces-web-dashboard), [details](architecture/admin-cli.md))
- `arcade-ops` reads state from S3, writes commands to S3, and shells out to AWS/SSH/Terraform for infrastructure operations

### AWS (Global Coordination)
- Global coordination is **minimized** — AWS handles only what individual peers cannot
- AWS serves the **static website** at **arcade.seanshubin.com** (S3 + CloudFront + ACM) ([decision](architecture-decisions.md#static-site-hosting-s3--cloudfront))
- AWS runs an always-on **relay** on a cheap cloud VM (e.g., AWS Lightsail, ~$3.50/month) for NAT traversal
- All clients make **outbound connections** to the AWS relay — no port forwarding, no UPnP, no STUN/TURN
- Relay protocol is **UDP** — plain socket application, no HTTP, no WebSocket ([details](architecture/network-operations.md))
- Relay access is **invite-only** via **shared secret in the Hello handshake** — operator distributes a passphrase out-of-band, relay rejects connections without it ([decision](architecture-decisions.md))
- Hello handshake carries **commit hash, shared secret, identity name, and identity secret** — one message, one round-trip; relay validates secret, validates identity, groups by version ([decision](architecture-decisions.md))
- **Identity registry** on the relay — maps identity names to hashed secrets, persisted to local disk, access control state not game state
- Identity is a **persistent identity name** with an auto-generated identity secret (4 BIP-39 words) — first-claim registration, enforced by the relay; secret changeable via CLI
- **Fixed 30-second retry** for all connectivity failures — version check, relay connection, S3 sync ([decision](architecture-decisions.md))
- Estimated total cost for 0-10 users: **~$5/month** ([cost details](architecture/network-operations.md#cost-estimate-0-10-users))

### Chat (v1)
- Chat messages are **game inputs** (opaque payload) — the relay never distinguishes chat from any other input type
- Messages are plain text with sender name and timestamp
- Chat history is **world state** — persisted, compacted, and restored via the same S3 save infrastructure as game state
- Joining peers receive chat history as part of the S3 save download, same as player positions in a game
- Bounding of chat history (last N messages, session-only, unlimited) is a **game mechanic**, not an infrastructure decision
- Identity is a **persistent identity name** — name chosen on first launch, identity secret auto-generated, first-claim enforced by relay
- Local config (identity name, identity secret, relay secret) stored in **platform app data directory** (`%APPDATA%\seans-arcade\config.toml` on Windows) ([decision](architecture-decisions.md))
- Game entities reference **identity name as persistent owner key** — entity reassociation on reconnect is game-layer logic
- `arcade-ops` identity management: `identity list`, `identity reset <name>`, `identity secret [<new-secret>]`
- **Identity secret rotation** via two-field config — player adds `new_identity_secret` to `config.toml`, client sends both in Hello, relay rotates the stored hash, client removes the field after success; Hello gains an optional `new_secret` field (backward-compatible) ([decision](architecture-decisions.md))
- **New-machine identity recovery** via claimed-name prompt — new machine auto-generates a secret, gets rejected, prompts "enter your identity secret"; player reads the 4 words from `config.toml` on old machine and types them; if old machine is gone, operator runs `identity reset` ([decision](architecture-decisions.md))

### Logging
- **Minimal logging, emergencies only** — no verbose operational logs, just panics and errors
- **`arcade`** logs to a file in `%APPDATA%\seans-arcade\` (next to `config.toml`) — no console window in release builds
- **`relay`** and **`arcade-ops`** log to stderr — console output is accessible directly

### Design
- Prefer **diegetic design** — interactions happen in the game world through the avatar, not in menus or overlays ([philosophy](research/design-philosophy.md))
- **Chat text is readable** — messages use standard legible text rendering
- **Player names use Mathematical Fraktur** — stylized rune-like appearance while remaining recognizable as Latin a-z/A-Z ([rationale](architecture-decisions.md#mathematical-fraktur-for-player-names))

### Arcade Model (v2+)
- The arcade is the **main application** — chat is the always-on social layer, games are sub-applications within it
- Chat is **always visible** to everyone — playing or watching a game does not leave the chat
- Games appear in a **game screen within the chat interface** — chat is the lobby
- Any player can **start a game and invite others**; any player can **spectate** any game
- **Multiple games run simultaneously** with different player subsets
- **Unified world state** — the entire arcade is one simulation with one tick stream; no multiplexed sessions ([rationale](architecture/session-architecture.md))
- **Game isolation (Matryoshka principle)** — each game receives its runtime environment (identity, input, output surfaces, lifecycle) through an explicit contract and never reaches outside it; the game is the invariant, the container is the variable ([decision](architecture-decisions.md))

### Infrastructure Tooling
- Infrastructure managed with **Terraform** — cloud-agnostic HCL, no bootstrap ceremony, state stored locally ([decision](architecture-decisions.md#terraform-for-infrastructure-management))
- CI deploys to S3 via **GitHub Actions OIDC** — no long-lived AWS credentials, role assumption scoped to master branch ([decision](architecture-decisions.md#github-actions-oidc-for-deployment))
- **No Docker or Kubernetes** for static hosting — commodity cloud services (S3, CloudFront) are sufficient ([decision](architecture-decisions.md#no-docker-or-kubernetes-for-static-hosting))
- **Relay deployed via Docker on Lightsail VM** — $5/month, UDP support, CI deploys via SSH ([decision](architecture-decisions.md#relay-deployment-lightsail-vm--docker--ssh))
- **All runtime secrets in GitHub Actions secrets** — relay secret, S3 bucket, AWS credentials; no secrets stored on the VM; VM is fully disposable ([decision](architecture-decisions.md#all-runtime-secrets-stored-in-github-actions-secrets))
- Default relay address is **relay.seanshubin.com:7700** — overridable in `config.toml` for local development

### Admin CLI
- **`arcade-ops` is the single operator interface** — monitoring, management, analytics, and infrastructure control in one tool ([decision](architecture-decisions.md#admin-cli-replaces-web-dashboard), [details](architecture/admin-cli.md))
- **All data flows through S3** — relay writes state every 5-15 seconds, CLI reads; data may be stale but never inconsistent
- Relay health via **heartbeat file** — relay writes `admin/heartbeat.json` with timestamp; CLI shows relay as down if timestamp is >30 seconds old
- Admin commands via **command files** — CLI writes to `admin/commands/`, relay polls and executes; relay is the single owner of mutable state
- One S3 bucket with **key prefixes** (`admin/`), not multiple buckets
- **Infrastructure control** — restart, redeploy, destroy via SSH and Terraform from the same tool
- **Analytics** — message volume, uptime history, version distribution, composite health checks
- **Supersedes the static web dashboard** — same S3 data flow, simpler consumer

### Assets
- **Manifest-based asset download** — S3 hosts `assets-manifest.json` (filename + SHA-256 hash per asset); client compares local vs. remote manifest on startup, downloads only changed/missing assets to the platform data directory ([decision](architecture-decisions.md#asset-distribution-strategy))

### Distribution
- **Windows, macOS, and Linux** — CI builds all three platforms; design is cross-platform from the start
- Single binary per platform, **self-replacing auto-update** — no separate launcher, no installer
- Version source of truth: `https://arcade.seanshubin.com/version` (git commit hash) — shared across all platforms
- The application has a **compiled-in commit hash** (embedded at build time) checked on startup against the remote version
- If versions match → proceed normally
- If versions differ → **auto-update**: download the platform-specific binary, replace self, restart
- If version check fails (no internet) → **offline mode**: launch with current version, show offline indicator, retry periodically until reachable
- The **relay isolates clients by version** — clients with different commit hashes cannot interact
- **Download URL is platform-specific** — platform subdirectories on S3 (e.g., `windows/arcade.exe`, `macos/arcade`, `linux/arcade`); the binary knows its own target at compile time
- **Self-replacement varies by platform** — Windows requires a rename dance (can't delete running exe); macOS/Linux can overwrite directly
- **Builds via GitHub Actions CI** — push to `master` triggers parallel native builds (`build-windows`, `build-macos`, `build-linux`); deploy job uploads binaries to S3 in platform subdirectories, invalidates CloudFront, and deploys relay via SSH
- **All platforms distributed as bare binaries**, not platform-specific bundles (no `.app`, no `.AppImage`)
- Bevy's `enhanced-determinism` feature flag **enabled for all builds** — required for cross-platform lockstep (forces `libm` software math)
- Bevy's `dynamic_linking` feature **disabled for all release builds** (breaks macOS and WASM)
- No differential/patch updates — full binary download every time
- No rollback — publish a new version with a higher number
- No code signing for v1 (required for macOS when that platform is added)
- **Self-installing on first run** (future) — binary copies itself to a standard OS location (`%LOCALAPPDATA%\seans-arcade\` on Windows, `~/Applications/seans-arcade/` on macOS, `~/.local/bin/` on Linux), creates shortcuts, and launches from there; auto-updates happen in that location. Blocked on code signing — unsigned self-installing binaries trigger OS security warnings.
- **OS user separation** for multi-user machines — each OS account gets its own data directory automatically (`%APPDATA%`, `~/Library/Application Support/`, `~/.config/`). No profiles feature needed; the `--data-dir` flag covers testing with multiple identities on one OS account.
- Running clients **continue on their current version** until relaunch — no mid-session updates, no background downloads
- The relay groups clients by commit hash — **multiple versions coexist** independently; each version group operates in isolation
- The relay treats game inputs as **opaque bytes** — only protocol-level changes (message framing, handshake) require relay redeployment; game logic changes are relay-transparent
- ([mechanism details](architecture/distribution.md))

### Networking (Games, v2+)
- **Deterministic lockstep** with relay server ([architecture](architecture/network-architecture.md))
- **Latency hiding**, not rollback ([decision](architecture-decisions.md))
- Relay **drops** slow inputs rather than stalling ([decision](architecture-decisions.md))
- Determinism via **constrained f32 + libm** for transcendentals, enforced by Bevy's `enhanced-determinism` feature flag ([decision](architecture-decisions.md))
- **Non-cryptographic checksums** (xxhash) for drift detection ([decision](architecture-decisions.md))
- **Seeded PRNG** for deterministic randomness — seed distributed as a game input, separate PRNGs for gameplay (synchronized) and cosmetics (local only) ([decision](architecture-decisions.md))
- **Full message logging**, always on ([decision](architecture-decisions.md))
- **Deterministic replay** from input logs, no game state logging ([decision](architecture-decisions.md))
- Persistence via **S3**, not always-on servers ([decision](architecture-decisions.md))
- **Tick-based sync** protocol, no streaming infrastructure ([decision](architecture-decisions.md))
- Player join via **S3 save + input log buffer** ([decision](architecture-decisions.md))
- **Two-tier log model** — hot buffer on relay, cold archive in S3 ([decision](architecture-decisions.md))
- **Log compaction** via periodic world state snapshots, stored separately from the input log with marker events ([decision](architecture-decisions.md))
- **Persistent storage layout** organized by commit hash and session ([details](architecture/network-operations.md))

## Decisions Needed

(none)

## Documentation

| Document                                                            | Contents                                                                         |
| ------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| [project-overview.md](project-overview.md)                          | Entry-point overview, evolution path, document index                             |
| [network-architecture.md](architecture/network-architecture.md)     | Lockstep relay networking model, determinism, latency hiding                     |
| [network-operations.md](architecture/network-operations.md)         | Diagnostics, debugging, deployment, AWS infrastructure, cost estimates           |
| [distribution.md](architecture/distribution.md)                     | Distribution, versioning, CI pipeline, auto-update                               |
| [deployment-pipeline.md](architecture/deployment-pipeline.md)       | CI/CD pipeline: build, deploy to S3, CloudFront invalidation                     |
| [deployment-setup.md](architecture/deployment-setup.md)             | One-time setup: AWS credentials, Terraform, GitHub secrets                       |
| [operations-reference.md](architecture/operations-reference.md)     | Where everything lives, VM commands, debugging checklist                         |
| [admin-cli.md](architecture/admin-cli.md)                           | Admin CLI design: monitoring, management, analytics, infrastructure control     |
| [admin-dashboard.md](architecture/admin-dashboard.md)               | *(Superseded)* — replaced by admin CLI                                          |
| [architecture-decisions.md](architecture-decisions.md)              | Formalized technical decisions with rationale                                    |
| [design-philosophy.md](research/design-philosophy.md)               | Game design principles that motivate technical decisions                         |
| [game-engine-anatomy.md](architecture/game-engine-anatomy.md)       | High-level engine pipeline overview                                              |
| [session-architecture.md](architecture/session-architecture.md)     | Session architecture decision: unified world state, not multiplexed sessions     |
| [bevy-vs-recoil-rts-scale.md](research/bevy-vs-recoil-rts-scale.md) | Bevy ECS vs Recoil engine for large-scale RTS                                    |
| [classic-game-candidates.md](research/classic-game-candidates.md)   | Candidate classic games for the arcade                                           |
| [zelda-case-study.md](research/zelda-case-study.md)                 | Zelda design analysis                                                            |
| [logical-design.md](games/9-keys/logical-design.md)                 | 9 Keys — logical design specification                                            |
| [design-analysis.md](games/9-keys/design-analysis.md)               | 9 Keys — game design reasoning                                                   |
| [maze-generation.md](games/9-keys/maze-generation.md)               | 9 Keys — maze generation theory                                                  |
| [mythology-and-naming.md](games/9-keys/mythology-and-naming.md)     | 9 Keys — Greek mythology behind item names                                       |
| [bevy-modern-api.md](research/bevy-modern-api.md)                   | Bevy modern API reference: current patterns vs deprecated ones                   |
| [randomness-in-bevy.md](research/randomness-in-bevy.md)             | Bevy RNG: PRNG types, algorithm selection, bevy_rand, deterministic seed forking |
