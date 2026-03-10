# Architecture Decisions

Formalized technical decisions with rationale. Each decision is stated declaratively. Detailed explanations live in the referenced documents. Game design principles that motivate these decisions live in [design-philosophy.md](research/design-philosophy.md).

This document records decisions that have been made. It is not a wishlist or a plan. If a decision is here, it's the current direction.

---

## Project Structure

### Crate structure

**Decision:** The project is a monorepo with the following crate layers:

| Crate | Kind | Depends on | Purpose |
| --- | --- | --- | --- |
| **`game-interface`** | lib | nothing | The trait that every game implements. Defines the contract: state type, input type, transition function. No Bevy dependency, no networking, no hosting knowledge. |
| **`pong`** (one per game) | lib | `game-interface` | Implements the trait. Pure transition function. Cannot tell whether it is running standalone or inside the arcade. |
| **`standalone`** | lib | `game-interface` | Generic host harness for running any game outside the arcade. Provides window setup, local input capture, simulation loop, rendering, replay recording/playback — all generic over the `game-interface` trait. |
| **`pong-standalone`** (one per game) | bin | `standalone`, `pong` | Entry point that wires a game to the standalone harness. Minimal code — typically one line: `standalone::run::<PongGame>()`. |
| **`protocol`** | lib | nothing | Wire format between client and relay. Serialization, admin types, S3 persistence types. |
| **`arcade`** | bin | `game-interface`, `protocol`, all game crates | The Bevy game client. Hosts all games, provides the navigable Arcade space, chat, networking. Acts as the production container for game plugins. |
| **`relay`** | bin | `protocol` | The lightweight input coordinator on AWS. Routes inputs by simulation context ID. No game state, no game logic. |
| **`arcade-ops`** | bin | `protocol` | Operator CLI for monitoring, management, analytics, and infrastructure control. |

**Key properties:**
- Game crates depend only on `game-interface` — never on `arcade`, `standalone`, `protocol`, or each other.
- `game-interface` has no dependencies on any other project crate.
- The `standalone` crate and the `arcade` crate are both hosts that satisfy the same `game-interface` contract. Games cannot distinguish between them.
- Each game has two crates: the game logic (lib) and its standalone entry point (bin). The standalone crates contain almost no code — just the dependency wiring.

**Alternatives rejected:** A single binary with mode flags (conflates player-facing and operator concerns, ships admin tooling to every player), many small scripts (scattered config, duplicated credential handling), or baking admin features into the relay (exposes admin surface on an internet-facing server). Separate repositories per game (unnecessary once AST hashing provides per-game transition function identity without needing per-repo commit hashes).

**Rationale:** The game client and relay have different deployment targets (player machines vs. AWS VM) and different security postures (local vs. internet-facing). The operator CLI consolidates all admin tasks because they share configuration (AWS credentials, relay address, S3 bucket) and are all developer-facing. Keeping admin out of the relay means the relay binary stays minimal with no admin attack surface. The two-crate-per-game pattern (lib + standalone bin) follows from the Matryoshka principle — the game is the invariant, the host is the variable. The standalone harness crate eliminates duplication across standalone entry points.

---

## Infrastructure

### Static site hosting: S3 + CloudFront

**Decision:** Downloadable binaries are served from `arcade.seanshubin.com` via S3 (storage) + CloudFront (CDN, HTTPS) + ACM (SSL certificate) + Route53 (DNS). The S3 bucket is private — only CloudFront can read from it via Origin Access Control.

**Alternatives rejected:** Self-hosted Nginx on EC2 (unnecessary operational burden for static files), GitHub Releases (ties distribution to GitHub, not a custom domain), bare S3 with public access (no HTTPS on custom domain, no CDN caching).

**Rationale:** Three `.exe` files and an `index.html` is a static hosting problem. S3+CloudFront is the commodity solution — zero maintenance, automatic scaling, pennies/month. CloudFront provides HTTPS and edge caching. The setup is trivially replaceable: every cloud has equivalent services (Azure Blob + CDN, GCP Cloud Storage + CDN). No application code depends on AWS APIs.

### Terraform for infrastructure management

**Decision:** AWS infrastructure is defined in Terraform (HCL) in the `infra/` directory. State is stored locally.

**Alternatives rejected:** AWS CloudFormation (verbose YAML, AWS-only), AWS CDK (requires Node.js bootstrap step, AWS-only), Pulumi (less mature ecosystem), manual console setup (not reproducible, not version-controlled).

**Rationale:** Terraform is cloud-agnostic — the same HCL knowledge and workflow applies to any cloud provider. If the project moves off AWS, the infrastructure definition is rewritten (~60 lines) but the tooling and mental model transfer completely. No bootstrap ceremony is required (unlike CDK). State starts as a local file, which is appropriate for a single-developer project. The configuration is version-controlled alongside the application code. CloudFormation was rejected primarily for verbosity and AWS lock-in; CDK was rejected for requiring a Node.js toolchain and `cdk bootstrap` in the AWS account.

### GitHub Actions OIDC for deployment

**Decision:** GitHub Actions authenticates to AWS via OpenID Connect (OIDC) federation — no long-lived AWS access keys. An IAM role (`arcade-github-deploy`) trusts the GitHub OIDC provider, scoped to the `master` branch of this specific repository. The role has minimal permissions: S3 put/delete/list on the site bucket, and CloudFront cache invalidation on the site distribution.

**Alternatives rejected:** IAM user with access key stored as GitHub secret (long-lived credential that must be rotated, broader blast radius if leaked), manual deployment (not automated, error-prone).

**Rationale:** OIDC federation is the AWS-recommended approach for GitHub Actions. No credentials are stored — GitHub mints a short-lived token per workflow run, AWS validates it against the OIDC provider. The IAM role is scoped to exactly the permissions needed (one S3 bucket, one CloudFront distribution) and can only be assumed from the master branch of this repo. If the repository is compromised, the attacker can only upload files to the download site — they cannot access other AWS resources.

### No Docker or Kubernetes for static hosting

**Decision:** The download site uses S3+CloudFront directly. No containers, no container orchestration.

**Alternatives rejected:** Docker + Nginx (container running a web server to serve static files), Kubernetes/EKS (container orchestration for multiple services).

**Rationale:** The download site serves three static files. Docker adds a container image to build, push, and run — for something S3 does natively. Kubernetes adds a ~$75/month control plane cost, steep learning curve, and operational complexity designed for dozens of microservices with auto-scaling needs. Neither provides any benefit over commodity static hosting at this scale. Docker may become relevant if the relay server is deployed to a cloud VM (packaging the relay binary with its runtime), but that is a separate decision for a separate concern.

### Relay deployment: Lightsail VM + Docker + SSH

**Decision:** The relay server runs as a Docker container on a Lightsail nano instance ($5/month, dual-stack). CI builds a Linux relay binary, packages it in a Docker image, pushes to ECR, and triggers deployment via SSH. DNS points `relay.seanshubin.com` to the VM's static IP.

**Alternatives rejected:**
- Lightsail Container Service ($7/month, fully managed) — does not support UDP, only HTTP/HTTPS
- ECS Fargate ($9-13/month, fully managed, supports UDP) — 3x the cost for a service that uses negligible resources
- Lightsail VM + systemd without Docker (cheapest, $5/month) — hand-rolled update mechanism; if the polling script breaks, requires SSH to fix
- Switching relay protocol to TCP to unlock container services — TCP's automatic retransmission contradicts the "drop slow inputs" design decision for future lockstep games
- AWS Systems Manager (SSM) — Lightsail instances are not EC2 instances and do not register with SSM; SSM commands cannot target them

**Rationale:** The relay is a UDP server. Managed container services (Lightsail Container, App Runner, Lambda) don't support UDP. Among the options that do, the Lightsail VM is the cheapest. Docker provides reproducible deployments — the same image runs everywhere. SSH key is stored as a GitHub secret (`RELAY_SSH_KEY`); CI writes it to a temporary file, runs the deploy command, and deletes it. The one-time VM setup (Docker, deploy script) is handled by Terraform user_data, so `terraform apply` creates a ready-to-deploy VM.

### All runtime secrets stored in GitHub Actions secrets

**Decision:** All secrets needed by the relay at runtime — `RELAY_SECRET`, `S3_BUCKET`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` — are stored as GitHub Actions repository secrets. CI passes them to the Docker container as environment variables during deployment. No secrets are stored as files on the VM.

**Alternatives rejected:**
- Secrets as files on the VM, set once via SSH — lost when the VM is destroyed or recreated; requires manual re-setup; no audit trail; easy to forget
- AWS Secrets Manager — $0.40/secret/month; unnecessary cost for this scale
- AWS SSM Parameter Store — free, but adds AWS API dependency at relay startup; Lightsail instances have limited IAM integration

**Rationale:** GitHub Actions secrets are free, encrypted at rest, masked in logs, and survive VM destruction. The VM becomes fully disposable — `terraform destroy` + `terraform apply` + a CI push produces a working relay with no manual steps beyond the initial Terraform and GitHub secret setup. This follows the same pattern already used for `RELAY_SSH_KEY`, `AWS_DEPLOY_ROLE_ARN`, and `CLOUDFRONT_DISTRIBUTION_ID`.

### Platform neutrality through commodity services

**Decision:** All AWS services used are commodity services with direct equivalents on every major cloud. No proprietary APIs are called from application code. The only AWS-specific artifacts are Terraform definitions and the CI deploy step.

**Alternatives rejected:** Using AWS-specific services in application code (DynamoDB, SQS, Lambda), building an abstraction layer over cloud APIs "just in case."

**Rationale:** Platform neutrality is achieved by choosing services that are commodity (S3 = blob storage, CloudFront = CDN, Route53 = DNS, ACM = SSL certs) rather than by abstracting over them. Every cloud offers these. The application binaries have zero cloud dependencies — they are Rust executables that communicate via UDP. Switching clouds means rewriting ~60 lines of Terraform and updating the CI deploy command — an afternoon of work, not an architecture change. An abstraction layer would add ongoing maintenance cost for a migration that may never happen.

---

## Admin CLI (replaces web dashboard)

### Admin CLI replaces web dashboard

**Decision:** `arcade-ops` is the single operator interface for monitoring, management, analytics, and infrastructure control. It reads S3 state files, writes S3 command files, and shells out to SSH/Terraform for infrastructure operations. There is no separate web dashboard. The S3 data flow is unchanged from the earlier dashboard design — only the consumer changed from a browser to a CLI.

**Previous decision (superseded):** A static web dashboard served from S3. Rejected because it required building, hosting, and securing a web frontend with browser-based auth, while the operator already has AWS credentials and SSH keys on their machine.

**Alternatives rejected:**
- HTTP/WebSocket endpoint on the relay — mixes admin concerns into the relay, adds HTTP to a UDP-only process.
- Separate admin API service — another binary to build, deploy, and maintain for minimal benefit.
- Static web dashboard — additional artifact to build and deploy; requires browser auth (signed cookies or Lambda@Edge); operator already has CLI credentials.

**Rationale:** The relay stays simple — write-only to S3, no admin endpoints, no HTTP. The CLI reuses existing operator credentials (AWS for S3, SSH key for Lightsail, Terraform state for infra). No web frontend to build, host, or secure. Near-real-time (5-15 second staleness) is sufficient for operator monitoring. The command-file pattern for write operations keeps the relay as the single owner of mutable state.

**See:** [admin-cli.md](architecture/admin-cli.md)

---

## Installation & User Separation

### Self-installing on first run

**Decision:** On first run, the binary copies itself to a standard OS location and launches from there. Future auto-updates happen in that installed location. Not yet implemented — blocked on code signing.

**Locations:**

| Platform | Install location | Data directory |
|----------|-----------------|----------------|
| Windows | `%LOCALAPPDATA%\seans-arcade\arcade.exe` | `%APPDATA%\seans-arcade\` |
| macOS | `~/Applications/seans-arcade/arcade` | `~/Library/Application Support/seans-arcade/` |
| Linux | `~/.local/bin/seans-arcade/arcade` | `~/.config/seans-arcade/` |

**Alternatives rejected:** Suggested location (user docs say "put it here" — no enforcement, users ignore it), platform-specific installer (MSI, .app bundle, .deb — heavy tooling for minimal benefit at this scale).

**Rationale:** Users download a bare binary and run it. Without self-install, it lives wherever they saved it (Desktop, Downloads, random folder), which makes auto-update fragile and the experience messy. Self-installing gives a predictable location without requiring a separate installer. Blocked on code signing because an unsigned binary that copies itself and creates shortcuts triggers aggressive OS security warnings (Windows SmartScreen, macOS Gatekeeper).

### OS user separation (no profiles)

**Decision:** Each OS user account gets its own data directory automatically via platform conventions (`%APPDATA%`, `~/Library/Application Support/`, `~/.config/`). No in-app profiles feature. The `--data-dir` flag covers the testing case of running multiple identities on one OS account.

**Alternatives rejected:** Named profiles (`--profile alice` mapping to a subdirectory), profile picker UI on launch.

**Rationale:** Different OS users already get separate data directories with zero code. An in-app profiles feature adds UI complexity for a scenario that barely exists — multiple real humans sharing one OS account and both playing the same niche game. The `--data-dir` flag already handles the developer testing case (batch files with separate data dirs). Building profiles solves a problem that doesn't exist yet.

---

## Assets

### Asset distribution strategy

**Decision:** Assets are downloaded on first launch via a manifest-based approach. S3 hosts `assets-manifest.json` listing each asset's relative path and SHA-256 hash. On startup, the client compares the remote manifest to the local manifest (stored in the platform data directory, next to `config.toml`). Only changed or missing assets are downloaded. Assets are stored in the data directory alongside the manifest. This keeps the binary small and avoids re-downloading unchanged assets. CI generates the manifest from tracked assets (`assets/fonts/`, `assets/local/`) and uploads both assets and manifest to S3.

**Options considered:**

| Strategy | Binary size | Distribution | Build iteration | Modding | When to use |
|----------|------------|-------------|-----------------|---------|-------------|
| **Embedded in binary** | Grows with every asset | Single file, nothing to copy | Recompile for any asset change | Impossible | Few small assets |
| **Assets on disk** | Binary stays small | Need installer, zip, or copy step | Fast — just replace the file | Users can swap files | Many large assets, rapid art iteration |
| **Asset download on first launch** (current) | Binary stays small | Single file + first-run download | Fast — update server assets | Possible via override directory | Moderate asset volume, clean distribution |
| **Installer/zip bundle** | N/A (packaged together) | Platform-specific packaging | Fast — just replace the file | Users can swap files | Commercial distribution, platform stores |

**Rationale:** The manifest approach keeps the binary small, eliminates the "where are my assets?" problem by storing them in the platform data directory, and only downloads what changed. Asset updates don't require a binary update — just change the asset on S3 and the manifest hash triggers a re-download on next launch.

---

## Networking Model

### Deterministic lockstep with relay server

**Decision:** All clients run the full simulation independently from shared inputs. A lightweight relay server orders and broadcasts inputs but performs no simulation and holds no game state.

**Alternatives rejected:** Server-authoritative (server simulates, clients render) and peer-to-peer rollback (clients simulate with rollback on misprediction).

**Rationale:** Trust between players (design philosophy #12) means no resources should be spent on server-side validation. Lockstep minimizes bandwidth (only inputs travel, not entity state) and eliminates the need for a powerful central server. The relay is stateless, cheap, and trivially replaceable.

**See:** [network-architecture.md](architecture/network-architecture.md) — How Lockstep Relay Works

### Latency hiding, not rollback

**Decision:** Each client maintains two separate states — authoritative (confirmed inputs only) and latency (authoritative + unconfirmed local inputs rebuilt each tick). The authoritative state is never wrong and never rewound.

**Alternatives rejected:** Rollback, where the authoritative state advances with predicted inputs and must rewind/re-simulate when predictions are wrong.

**Rationale:** The authoritative state is never temporarily wrong, so there's no correction cost and no visual "snap-back." The latency state is disposable — rebuilt fresh every tick from the authoritative state. Simpler to reason about: one state is always correct, the other is always a guess.

**See:** [network-architecture.md](architecture/network-architecture.md) — Latency Hiding: Prediction Without Rollback

### Relay drops slow inputs rather than stalling

**Decision:** If a player's input hasn't arrived by the time the relay needs to broadcast the confirmed package for a tick, the relay omits that player's input and broadcasts without it. The slow player's character doesn't act that tick.

**Alternatives rejected:** Pure lockstep, where the simulation stalls until all inputs arrive.

**Rationale:** Pure lockstep lets one slow connection freeze every player. Dropping and continuing keeps the game running for everyone. The slow player catches up by processing multiple ticks per frame when their connection recovers. This is the change that took Factorio from ~24 players to 400+.

**See:** [network-architecture.md](architecture/network-architecture.md) — Stalls vs. Dropping

### Commit hash as the build identifier

**Decision:** The git commit hash identifies the overall build. It is embedded at build time (build script reads `git rev-parse HEAD`), included in the Hello handshake, and logged on connection events. It replaces the manually-bumped application version integer.

The commit hash identifies the build, not individual transition functions. For per-game transition function identity, see [Transition function identity via AST hash](#transition-function-identity-via-ast-hash). The two work together: the commit hash tells you which code to check out and build; the AST hash tells you whether a specific game's logic actually changed.

**Alternatives rejected:** Manually-bumped version integer (`VERSION: u32 = 7`) — requires remembering to bump, and forgetting silently allows incompatible clients. Using the commit hash as the transition function identity — too coarse, since unrelated changes (Chat, Arcade, docs) change the hash even when a game's logic is unchanged, invalidating replays unnecessarily.

**Rationale:** The commit hash changes automatically on every commit without anyone remembering to bump anything. It tells you exactly which code to check out and build. The CI pipeline writes `$GITHUB_SHA` to the S3 version file, eliminating the manual version bump step entirely. Per-game compatibility is handled by AST hashes, not the commit hash.

**See:** [distribution.md](architecture/distribution.md) — Version Check, [network-operations.md](architecture/network-operations.md) — Version Index

### Version isolation (no mid-session updates)

**Decision:** Running clients continue on their current version until relaunch. The relay groups simulation context inputs by commit hash — different versions coexist independently. Chat is not version-isolated; chat messages are broadcast to all connected clients regardless of version. No mid-session update notification, no background download, no grace period.

**Alternatives rejected:** Live update orchestration (relay polls version file, notifies clients, background download, grace period with version-aware routing). Full version isolation including chat (unnecessarily partitions conversation across versions).

**Rationale:** For a small invite-only group, coordinating a relaunch is trivial. The live update machinery adds substantial complexity for minimal benefit. Simulation context inputs require version isolation because deterministic lockstep demands identical code. Chat does not — messages are independent events with no causal state dependency, so they can cross version boundaries safely.

**See:** [distribution.md](architecture/distribution.md) — Version Isolation, [network-architecture.md](architecture/network-architecture.md) — Simulation Context: When Lockstep Applies

### Context-based input routing

**Decision:** Each `Input` message carries a `context` tag that determines broadcast scope. The relay uses the context to route: inputs with context `"chat"` are broadcast to all connected clients regardless of version. All other context values are treated as simulation context inputs and broadcast only to clients with the same commit hash.

**Previous decision (superseded):** Chat was an opaque Input payload indistinguishable from game inputs. The relay broadcast all inputs to same-version clients only. This unnecessarily partitioned chat across versions.

**Alternatives rejected:** Separate `ClientMessage::Chat` variant — leaks application semantics into the relay protocol. Having the relay deserialize payloads to detect chat — violates the opaque payload principle.

**Rationale:** The relay remains payload-opaque. It reads the context tag (a string) to determine routing scope, but never interprets the payload. The client's focus system determines which context tag is attached to each input. Chat history persists across version changes because it was never tied to a simulation context.

---

## Game Isolation (Matryoshka Principle)

### Each game is a self-contained unit that cannot tell if it's running standalone or embedded

**Decision:** Every game in the arcade is a self-contained unit that receives its runtime environment through an explicit interface and never reaches outside it. The game is the invariant; the container is the variable. A game cannot detect or depend on whether it's running standalone, embedded in the chat lobby, or nested inside another game.

The boundary is defined by a **runtime contract** between the game and its container:

The container provides:
- **Identity** — who the players are (identity names, player slots)
- **Input** — abstract game actions, not raw devices (the game never touches keyboards or gamepads directly)
- **Output surfaces** — render target, audio channel, output event sink
- **Lifecycle signals** — start, pause, teardown

The game provides:
- **Systems** — its simulation logic
- **Entity hierarchy** — its world state, rooted under a single entity
- **Output events** — typed events emitted to the container (game over, score, etc.)

The game declares what it needs; the container satisfies those declarations. The game never reaches outward.

**Alternatives rejected:** Games that import lobby systems, games that call into their container, games that directly communicate with sibling games, or games that assume a specific hosting context.

**Rationale:** The constraint forces real abstraction boundaries. If a game can run standalone, it has no hidden dependencies on the lobby — no ambient state, no implicit services, no backdoor coupling. This enables standalone testing (run any game without lobby infrastructure), faster development iteration (build and test Pong without booting chat), and trivially correct spectating and replay (a game is just an input stream and an entity hierarchy).

The litmus test: the chat lobby is itself a game, and the architecture should permit running it as a game inside another chat lobby. This recursive property doesn't need to ship in production, but if the architecture can't support it, the boundaries are leaky.

The runtime contract is the same regardless of what the container is — the chat lobby (production), a standalone test harness (development), another lobby (Matryoshka), or a hypothetical web port, mobile shell, or replay viewer. Porting to a different environment means implementing the runtime contract in that environment, not modifying the game. This separation between "the game" and "setting up the game" is already natural; the Matryoshka constraint makes it load-bearing.

Cross-game interaction flows through the container, never directly between games. A game emits output events (game over, score); the container decides what to do with them (post to chat, update a scoreboard, offer a rematch). The game has no concept of chat, scoreboards, or rematches.

---

## Determinism

### Constrained f32 with software transcendentals

**Decision:** Use standard f32 for basic math (+, -, *, /, sqrt — already IEEE 754 deterministic). Use the `libm` crate for transcendental functions (sin, cos, etc.). Compile with `-C llvm-args=-fp-contract=off` to prevent FMA fusion. Use deterministic collections (BTreeMap, sorted iteration). Disable Bevy's parallel system execution for gameplay systems.

**Alternatives rejected:** Fixed-point math (awkward API, no Bevy integration), WASM as determinism layer (performance overhead), or accepting non-determinism.

**Rationale:** Patches the specific known problem spots (transcendentals, FMA, collection ordering, system parallelism) with minimal friction. Most code stays normal. Combined with checksums as a safety net.

**See:** [network-architecture.md](architecture/network-architecture.md) — Making P2P Determinism Practical

### Non-cryptographic checksums for drift detection

**Decision:** Periodic state checksums using xxhash (non-cryptographic, 64-bit). Exchanged every 30-60 ticks. Treat every desync as a determinism bug to investigate and fix.

**Alternatives rejected:** Cryptographic hashes (SHA-256) or no checksums.

**Rationale:** Players are trusted (design philosophy #12), so collision resistance against intentional forgery is unnecessary. Non-cryptographic hashes are orders of magnitude faster. 64-bit xxhash has a 1-in-2^64 probability of accidental false-match — effectively impossible. Checksums cost 8 bytes per exchange per peer.

**See:** [network-architecture.md](architecture/network-architecture.md) — State Checksums: Detecting Drift

### Seeded PRNG for deterministic randomness

**Decision:** Randomly generated content (level generation, enemy behavior, loot drops) uses a deterministic pseudorandom number generator seeded from a value distributed as a game input. Use a PRNG with a spec-guaranteed output sequence (e.g., `ChaCha8Rng` from `rand_chacha`). Separate PRNGs for gameplay (must be synchronized) and cosmetics (particles, screen shake — local only, not synchronized). The seed is an opaque input like any other — the relay doesn't know it's a seed.

**Alternatives rejected:** `thread_rng()` or `StdRng` for gameplay randomness (`StdRng`'s algorithm is not guaranteed stable across Rust versions), a single shared PRNG for both gameplay and cosmetics (cosmetic draw count divergence causes gameplay desync), or avoiding randomness entirely.

**Rationale:** Deterministic lockstep requires identical simulation across all clients. A seeded PRNG produces the same sequence on every client given the same seed and the same draw order. Distributing the seed as a game input means it travels through the existing tick-ordered input stream — no new infrastructure. Separating gameplay and cosmetic PRNGs ensures that visual-only randomness (which may vary with frame rate or rendering differences) cannot perturb the gameplay sequence. The existing checksum infrastructure catches any divergence if PRNG draw order accidentally differs between clients.

### Transition function identity via AST hash

**Decision:** Each game's transition function is identified by a hash of its Rust AST (Abstract Syntax Tree). CI parses each game crate's source, hashes the AST, and writes the hash as a build artifact alongside the binary. Inputs are tagged with the AST hash of the transition function that interprets them, not the commit hash of the overall build.

The AST hash is per-game, not per-build. A commit that changes Chat code does not change the Pong AST hash, so Pong replays remain valid. A commit that changes Pong logic produces a new Pong AST hash, correctly invalidating old Pong replays without affecting other games.

Games do not know their own AST hash. The hash is metadata *about* the game, not embedded in the binary. It is computed by CI and stored as a build artifact. The host (arcade or standalone harness) reads the hash file at startup and uses it to tag inputs and replay logs. The game crate has no dependency on the hashing mechanism.

**Alternatives rejected:**

| Approach | What gets hashed | Why rejected |
| --- | --- | --- |
| **Commit hash** | Entire repository | Too coarse — unrelated changes invalidate replays. Already used for overall build identity, but not suitable for per-game transition function identity. |
| **Manual version bump** | Developer's memory | Error-prone — forgetting to bump silently allows incompatible clients. |
| **Source hash** | Source files | False positives from comment and formatting changes that don't affect behavior. |
| **MIR hash** | Mid-level Intermediate Representation | Fewer false positives than AST (ignores refactors that don't change behavior), but MIR extraction (`-Z unpretty=mir`) requires nightly Rust. Depending on an unstable toolchain introduces unpredictable breakage — each nightly snapshot has no stability guarantees, no changelog, and no way to assess what problems a particular build may have before committing to it. |
| **LLVM IR hash** | LLVM intermediate representation | More sensitive to compiler internals than AST without being more useful. Same nightly-dependency problem as MIR. |
| **Object code hash** | Compiled `.o` files | Too many false positives — optimization level, debug flags, and link order change output without changing behavior. |
| **Separate repositories per game** | Per-repo commit hash | Solves the granularity problem but unnecessary once AST hashing provides per-game identity within a monorepo. Adds operational overhead of multi-repo management. |

**Key tradeoff — AST vs MIR:** AST has more false positives than MIR — it changes on refactors (variable rename, function extraction, parameter reorder) that don't change behavior. MIR ignores these because it represents post-desugaring computation. However, MIR extraction requires nightly Rust. The false positives from AST are harmless (a replay hash invalidation just means re-recording), while the instability of nightly Rust is an open-ended risk with unpredictable consequences. A refactor that triggers a false positive is something the developer did and understands; a nightly build that breaks in a new way is not.

**Rationale:** The AST ignores comments, formatting, and whitespace — the most common sources of meaningless churn. It works on stable Rust with no unstable compiler flags. The crate boundary (one crate per game) provides a natural scope for AST extraction. The false positive rate is higher than MIR (refactors trigger hash changes) but the existing runtime checksum infrastructure catches anything the AST hash misses, and the cost of a false positive is low — just a replay invalidation, not a correctness problem.

**See:** [network-architecture.md](architecture/network-architecture.md) — Simulation Context: When Lockstep Applies

---

## Debugging and Observability

### Full message logging, always on

**Decision:** Log every network message sent and received — player inputs, confirmed packages, checksums — with wall-clock timestamps, tick numbers, player slots, and payloads. On both clients and relay.

**Alternatives rejected:** Selective logging or no logging.

**Rationale:** In lockstep relay, total network traffic is ~19 KB/sec for 4 players. Logging everything with metadata is ~100 KB/sec — negligible. A one-hour session compresses to ~20-35 MB. This is practically free and enables deterministic replay, latency analysis, and desync debugging. Server-authoritative architectures can't do this because their traffic volume is orders of magnitude higher.

**See:** [network-operations.md](architecture/network-operations.md) — Message Logging and Deterministic Replay

### Deterministic replay from input logs

**Decision:** The complete input log is a complete game recording. Any session can be replayed by feeding the logged inputs into the simulation. No game state logging is needed.

**Alternatives rejected:** State-based replay (recording entity positions each frame) or no replay capability.

**Rationale:** Determinism guarantees that identical inputs produce identical state. The input log is tiny (KB/sec). State recording would be orders of magnitude larger and still less useful for debugging. Replay enables tick-by-tick inspection, desync diagnosis (replay on two instances and compare checksums), and distinguishing determinism bugs from network bugs.

**See:** [network-operations.md](architecture/network-operations.md) — Deterministic Replay

---

## Log Compaction

### Two-tier log model

**Decision:** The input log is split into two tiers. The hot tier lives in relay memory — input entries from the latest snapshot tick to the current tick. The cold tier is archived to S3 — the full session history from tick 0. Compaction advances the boundary by writing a world state snapshot to S3 and flushing older entries from the hot buffer to cold storage.

**Alternatives rejected:** Single-tier log where everything lives in relay memory or everything streams directly to S3 with no operational split.

**Rationale:** The relay needs fast access to recent inputs for player join catch-up and jitter absorption, but doesn't need the full session history in memory. Separating hot from cold keeps relay memory bounded while preserving the complete log for replay and debugging. The hot buffer size is determined by the compaction interval — at 2–5 minutes and 60 ticks/sec, this is 7,200–18,000 entries, well within memory.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

### Snapshots stored separately from the input log

**Decision:** World state snapshots are stored as separate S3 objects, not embedded in the input log. The log contains a snapshot marker event at each compaction tick, recording the tick number and S3 key of the corresponding snapshot.

**Alternatives rejected:** Embedding serialized world state inline in the input log.

**Rationale:** Input entries are small and uniform (~20 bytes each). World state snapshots can be KB to MB. Embedding snapshots inline would bloat the log and complicate sequential reads. Separate storage lets each artifact be read independently — operations (join, crash recovery) read only the snapshot; replay reads only the log. The marker event keeps the log self-describing without carrying the snapshot payload.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

### Compaction triggers

**Decision:** Compaction is triggered by four conditions: periodic interval during play (every 2–5 minutes), session end, player join when the hot log has grown large since the last compaction, and a safety-valve size threshold on the hot buffer.

**Alternatives rejected:** A single trigger (e.g., only periodic or only on session end).

**Rationale:** Multiple triggers ensure bounded hot buffer size under all conditions. Periodic compaction handles the steady state. Session-end compaction captures the final state. Join-triggered compaction reduces catch-up time for arriving players. The size threshold guards against edge cases where periodic timers drift or fail. At this data volume, the cost of extra S3 PUTs is negligible.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

---

## Persistence and Infrastructure

### Cloud storage for persistent state, not always-on servers

**Decision:** Save files stored in cheap object storage (S3, R2, B2). No always-on server. The relay runs only during active sessions. Between sessions, nothing is running and the only cost is storage (pennies/month).

**Alternatives rejected:** Always-on dedicated servers that hold world state.

**Rationale:** Traditional server-authoritative architectures conflate simulation compute, authoritative state, and persistent storage into one always-on server. In lockstep relay, every client holds the full state and the relay is stateless. The only between-session need is storage, which costs essentially nothing. Paying for 24/7 compute to store a few MB is wasteful.

**See:** [network-operations.md](architecture/network-operations.md) — Persistent State Between Sessions

### Tick-based sync protocol

**Decision:** Any client can sync the canonical save to S3 at any time. Check the stored tick number (HEAD request), upload only if ahead (PUT with tick in metadata), skip if behind or equal. Automatic periodic sync during play and final sync on session end.

**Alternatives rejected:** Designated-uploader protocols, lock-based sync, or streaming infrastructure.

**Rationale:** Determinism means two clients at the same tick have identical state. There is no merge, no diff, no conflict. Two clients racing to upload the same tick upload identical bytes. A lagging client checks, sees it's behind, and skips. The protocol is three steps: HEAD, compare, conditional PUT. Only authoritative state (confirmed ticks) is synced — never the latency state.

**See:** [network-operations.md](architecture/network-operations.md) — Sync Protocol

### S3 save + input log for player join

**Decision:** Players joining mid-session download the latest save from S3 and catch up via buffered input history. No existing client is involved in the join process.

**Alternatives rejected:** Existing client snapshots and sends state directly to the joining player.

**Rationale:** Zero impact on existing players — they don't even notice someone joined. S3 download is fast and reliable. Multiple players can join simultaneously. The catch-up window is bounded by the sync interval (at most a few thousand ticks of fast-forward simulation). Three separately-motivated features converge: S3 sync (persistence), message logging (debugging), and relay input buffer (jitter absorption) — all contribute to making join seamless.

**See:** [network-operations.md](architecture/network-operations.md) — Player Join Mid-Game

### No streaming infrastructure

**Decision:** Direct S3 API calls (PUT/GET/HEAD) for all persistence needs. No message brokers (Kafka, Kinesis), no event streams, no pub/sub systems.

**Alternatives rejected:** Kafka → S3 pipeline or similar streaming architectures.

**Rationale:** The relay handles ~19 KB/sec of input data for 4 players. This is far too little traffic to justify infrastructure designed for millions of messages per second. Kafka itself requires always-on servers, reintroducing the cost the architecture is designed to avoid. A periodic S3 PUT every few minutes is the entire persistence solution.

**See:** [network-operations.md](architecture/network-operations.md) — Mid-Session Durability

---

## Access Control

### Shared secret in the Hello handshake

**Decision:** The relay is protected by a shared passphrase. The operator sets the secret on the relay (environment variable or config file) and distributes it to invited players through an out-of-band channel (text message, Discord, phone call). The client includes the secret in the Hello handshake alongside the commit hash. The relay silently drops connections with an incorrect or missing secret — no error response, no acknowledgment. The secret is sent in plaintext over UDP.

**Alternatives rejected:** Per-user tokens with a revocation list (more infrastructure for a problem that doesn't exist at 0-10 trusted users), client certificates or key pairs (overengineered — requires PKI infrastructure for 0-10 trusted users), IP allowlists (fragile — residential IPs change, people play from different networks), and encrypted handshake (DTLS — all session traffic is already plaintext UDP; encrypting only the handshake solves nothing).

**Rationale:** The threat model is random strangers on the internet discovering the relay's IP and connecting — not malicious insiders or targeted surveillance. A shared secret stops port scanners, bots, and accidental connections. The group is small (0-10) and trusted, so the social cost of sharing one passphrase is near zero. Rotation is trivial — the operator changes it on the relay and tells everyone the new one. The secret is not stored in S3 or any cloud service; it exists only on the relay and on each client's local disk.

**See:** [network-operations.md](architecture/network-operations.md) — Relay Access Control

### Player identity: self-service first-claim with auto-generated identity secret

**Decision:** Each player has a persistent identity name (up to 20 characters), used everywhere as their sole identifier. On first launch the client prompts for a name only — the client auto-generates 4 random BIP-39 words as the identity secret, stores both in config.toml, and the player never sees the secret during onboarding. The relay stores identity_name → hash(identity_secret) in a local registry file. On subsequent connections: unclaimed name → register and accept; correct secret → accept; wrong secret → relay responds with a "name claimed" rejection (the client has already passed shared secret validation, so it's trusted). The client then prompts the player to enter their identity secret or pick a different name. The player can view or change their secret later via `arcade-ops identity secret [<new-secret>]` (any passphrase, not restricted to BIP-39 or 4 words).

**Alternatives rejected:** Account registration via web portal (contradicts download-launch-play for 0-10 people), operator-managed identity list (bottleneck on operator for every player), cryptographic key pairs (key loss = permanent identity loss with no recourse), prompting for an identity secret on first launch (the player has no context for why they need one, will type something throwaway and forget it).

**Rationale:** Extends the existing access control pattern — shared secret stops strangers, identity secret stops impersonation. Shared secret failure is silent drop (outsiders learn nothing); identity failure after passing shared secret is a rejection response (the client is already trusted and needs to know the reason to prompt the player). The registry is access control state, not game state. At 0-10 people, registry loss on VM replacement means friends re-claim in seconds.

### Hello handshake carries commit hash, shared secret, identity name, and identity secret

**Decision:** All four fields in a single Hello message. The relay validates the shared secret first (silently drops if wrong — rejects strangers), then validates identity (unclaimed name → register; correct secret → accept; wrong secret → responds with a "name claimed" rejection), then groups by commit hash (version isolation). The handshake may involve a retry for the claimed-name case: client sends Hello → gets "name claimed" → prompts user for their identity secret → sends Hello again with the correct secret.

**Alternatives rejected:** Multi-step handshake where secret validation, version check, and identity registration happen in separate exchanges.

**Rationale:** A single message keeps the protocol simple and minimizes round-trips. The relay already needs all four pieces of information before it can assign a player slot. Shared secret failure is silent drop — outsiders learn nothing about whether the relay exists or what protocol is in use. Identity failure after passing shared secret is a rejection response — the client has already proven it's invited, so there's no security reason to withhold the rejection reason, and the client needs to know the reason to prompt the player appropriately. The claimed-name retry adds at most one additional round-trip in the second-machine case.

### Identity secret rotation via two-field config

**Decision:** A player changes their identity secret by adding a `new_identity_secret` field to `config.toml` alongside the existing `identity_secret`. On next launch, the client sends both values in the Hello message — the current secret for authentication, the new secret for rotation. The relay validates the current secret, replaces the stored hash with `hash(new_identity_secret)`, and accepts the connection. The client then rewrites `config.toml` with only the new secret, removing the `new_identity_secret` field. The Hello message gains an optional `new_secret` field for this purpose — if absent, the existing Hello flow is unchanged (backward-compatible).

**Alternatives rejected:** Dedicated CLI command requiring the user to run a separate tool (extra step for a one-field edit), web-based secret change (no web UI exists), or relay admin endpoint for rotation (exposes admin surface on the internet-facing server).

**Rationale:** The user already has `config.toml` open as their single point of configuration. Adding one line to the file they already edit is the lowest-friction rotation flow possible. The two-field approach is atomic from the user's perspective — edit, launch, done — and safe: if the old secret is wrong, nothing changes and the `new_identity_secret` field stays for the user to diagnose. The protocol extension is backward-compatible: the optional `new_secret` field is ignored by relays that don't support it, and its absence preserves the existing flow exactly. Edge cases are benign — setting the new secret identical to the old one is a harmless no-op (relay re-hashes the same value, client cleans up the field).

### New-machine identity recovery via claimed-name prompt

**Decision:** When a player connects from a new machine, the client auto-generates a new secret (as with any first launch), sends Hello, and receives a "name claimed" rejection because the auto-generated secret doesn't match the relay's stored hash. The client then prompts: "This name is already taken. Enter your identity secret to prove it's yours." The player reads their `identity_secret` from `config.toml` on their old machine and types it on the new machine. The client sends Hello again with the entered secret. On success, the new machine's `config.toml` is written with the correct name and secret. If the old machine is inaccessible, the operator runs `arcade-ops identity reset <name>` to clear the relay's stored hash, and the player re-claims the name on next connect via first-claim registration.

**Alternatives rejected:** Automatic multi-device sync (requires sync infrastructure that doesn't exist for 0-10 users), export/import key file (extra steps for a 4-word string the user can just read and type), or email/SMS recovery (requires account infrastructure that doesn't exist).

**Rationale:** The secret is already stored in plaintext in `config.toml` — the user just reads 4 words and types them. No recovery tool, no export step, no QR code. The claimed-name prompt is the same one that fires for any wrong-secret Hello, so no new UI is needed. The operator reset fallback (`arcade-ops identity reset <name>`) handles the lost-machine case without building recovery infrastructure; at 0-10 friends this is a quick ask, not a support ticket. The flow reuses the existing Hello retry mechanism — the client already handles "name claimed" rejections and re-prompts.

---

## Connectivity

### Fixed 30-second retry interval

**Decision:** When any network operation fails (version check, relay connection, S3 sync), the client retries on a fixed 30-second interval until it succeeds. No exponential backoff, no jitter, no adaptive timing.

**Alternatives rejected:** Exponential backoff (standard for distributed systems with shared servers) and adaptive intervals.

**Rationale:** Exponential backoff exists to protect shared servers from retry storms when many clients fail simultaneously. None of those conditions apply here: there is one client retrying, the targets are S3 (effectively infinite capacity) or a relay serving 0-10 users, and there is no thundering herd. A fixed interval gives the user a predictable countdown, and 30 seconds means connectivity is detected within half a minute of restoration. The cost of each retry is one small HTTP GET or UDP packet — negligible.

---

## Local Storage

### Local config in platform app data directory

**Decision:** Player identity name, identity secret, and relay secret persist between launches in a TOML file at the platform-conventional location (`%APPDATA%\seans-arcade\config.toml` on Windows). Not alongside the binary, not in the registry.

**Alternatives rejected:** Config file next to the binary (binary self-replaces on update, complicating the update dance or risking config loss) and Windows registry (not portable, harder to inspect, platform-specific API).

**Rationale:** Platform app data directories survive application updates, are user-discoverable, and work the same conceptual way across platforms. TOML is human-readable and trivially editable. The file contains no secrets that need encryption — the relay secret is a shared passphrase, not a credential, and the identity secret has the same security posture — auto-generated on first launch (4 BIP-39 words), stored locally, changeable via `arcade-ops identity secret <new-secret>` (any passphrase, not restricted to BIP-39).

---

## Event Log Principles

### The canonical log is sufficient to deterministically recreate all game state

**Decision:** The canonical event log is the sole source of truth for what happened in a session. Given the log and the correct version of the simulation code, every game state at every tick can be recreated with zero ambiguity. The log contains every input, in order, with enough metadata to identify which code produced each event — the commit hash, logged on connection events, tells you exactly which code to check out and build for replay.

**Alternatives rejected:** Logs that require external context (out-of-band knowledge of what version was running, manual annotations) or logs that record derived state (entity positions, scores) instead of inputs.

**Rationale:** Deterministic simulation means inputs + code are the complete description of a session. Logging inputs is sufficient; logging derived state is redundant. But inputs alone are not sufficient without knowing which code to replay them with — the same inputs produce different results on different versions. The commit hash on connection events makes the log self-describing: check out that commit, build, feed in the inputs, get the exact simulation.

**See:** [network-operations.md](architecture/network-operations.md) — Message Logging and Deterministic Replay

### The log stores the minimum information needed for zero ambiguity

**Decision:** Information that can be derived from other logged events is not duplicated on every log entry. State that is constant across a connection (such as the commit hash) is logged once at the connection event, not repeated on every input from that connection.

**Alternatives rejected:** Per-event annotation (stamping every log entry with the commit hash or other connection-level metadata).

**Rationale:** The commit hash is identical for every message from a given connection — it cannot change without disconnecting and reconnecting, which produces a new connection event. Repeating it on every log entry adds bytes that carry zero additional information. With version isolation, logs are split by version, so every entry in a given log shares the same commit hash — recorded once per log. Minimum information means: log state changes, not state itself.

**See:** [network-operations.md](architecture/network-operations.md) — Version Index

---

## Design

### Mathematical Fraktur for player names

**Decision:** Chat message text uses standard readable rendering. Player names are displayed in Mathematical Fraktur, mapping a-z to 𝔞–𝔷 (U+1D51E–U+1D537) and A-Z to 𝔄–ℨ (U+1D504–U+1D51B). Names are restricted to a-z/A-Z characters. Note: uppercase Fraktur has five codepoints (C, H, I, R, Z) that live in the Letterlike Symbols block rather than the Mathematical Alphanumeric Symbols block, so uppercase requires a lookup table rather than a simple offset.

**Alternatives rejected:** Elder Futhark runes (real historical runes but unreadable — players can't learn to recognize names), custom sprite sheet (maximum creative control but requires art assets and custom rendering), plain Latin text (functional but no visual identity).

**Rationale:** Mathematical Fraktur is a Unicode range that maps 1:1 to Latin a-z, so names are stylized but still recognizable with brief exposure. It requires no custom art — it's just a character offset. Chat readability is preserved because only the name is stylized, not the message body. The rune-like aesthetic gives the arcade visual character without sacrificing usability.
