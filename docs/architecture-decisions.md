# Architecture Decisions

Formalized technical decisions with rationale. Each decision is stated declaratively. Detailed explanations live in the referenced documents. Game design principles that motivate these decisions live in [design-philosophy.md](research/design-philosophy.md).

This document records decisions that have been made. It is not a wishlist or a plan. If a decision is here, it's the current direction.

---

## Project Structure

### Three binaries: game client, relay server, operator CLI

**Decision:** The project produces three separate Rust binaries: `arcade` (the Bevy game client players run), `relay` (the lightweight input coordinator on AWS), and `arcade-cli` (an operator tool with subcommands for deployment, monitoring, and debugging: `deploy`, `status`, `logs`, `desync-check`, `save push`, `save pull`).

**Over:** A single binary with mode flags (conflates player-facing and operator concerns, ships admin tooling to every player), many small scripts (scattered config, duplicated credential handling), or baking admin features into the relay (exposes admin surface on an internet-facing server).

**Rationale:** The game client and relay have different deployment targets (player machines vs. AWS VM) and different security postures (local vs. internet-facing). The operator CLI consolidates all admin tasks because they share configuration (AWS credentials, relay address, S3 bucket) and are all developer-facing. Keeping admin out of the relay means the relay binary stays minimal with no admin attack surface. Three binaries is the minimum — fewer conflates concerns, more fragments shared config.

---

## Networking Model

### Deterministic lockstep with relay server

**Decision:** All clients run the full simulation independently from shared inputs. A lightweight relay server orders and broadcasts inputs but performs no simulation and holds no game state.

**Over:** Server-authoritative (server simulates, clients render) and peer-to-peer rollback (clients simulate with rollback on misprediction).

**Rationale:** Trust between players (design philosophy #11) means no resources should be spent on server-side validation. Lockstep minimizes bandwidth (only inputs travel, not entity state) and eliminates the need for a powerful central server. The relay is stateless, cheap, and trivially replaceable.

**See:** [network-architecture.md](architecture/network-architecture.md) — How Lockstep Relay Works

### Latency hiding, not rollback

**Decision:** Each client maintains two separate states — authoritative (confirmed inputs only) and latency (authoritative + unconfirmed local inputs rebuilt each tick). The authoritative state is never wrong and never rewound.

**Over:** Rollback, where the authoritative state advances with predicted inputs and must rewind/re-simulate when predictions are wrong.

**Rationale:** The authoritative state is never temporarily wrong, so there's no correction cost and no visual "snap-back." The latency state is disposable — rebuilt fresh every tick from the authoritative state. Simpler to reason about: one state is always correct, the other is always a guess.

**See:** [network-architecture.md](architecture/network-architecture.md) — Latency Hiding: Prediction Without Rollback

### Relay drops slow inputs rather than stalling

**Decision:** If a player's input hasn't arrived by the time the relay needs to broadcast the confirmed package for a tick, the relay omits that player's input and broadcasts without it. The slow player's character doesn't act that tick.

**Over:** Pure lockstep, where the simulation stalls until all inputs arrive.

**Rationale:** Pure lockstep lets one slow connection freeze every player. Dropping and continuing keeps the game running for everyone. The slow player catches up by processing multiple ticks per frame when their connection recovers. This is the change that took Factorio from ~24 players to 400+.

**See:** [network-architecture.md](architecture/network-architecture.md) — Stalls vs. Dropping

### Commit hash as the single code identifier

**Decision:** The git commit hash is the single identifier for "this exact code." It is embedded at build time (build script reads `git rev-parse HEAD`), included in the Hello handshake, and logged on connection events. It replaces both the manually-bumped application version integer and any derived code/spec hashes.

**Over:** Manually-bumped version integer (`VERSION: u32 = 7`) — requires remembering to bump, and forgetting silently allows incompatible clients. Spec hash derived from event type definitions — only detects wire format changes, not gameplay logic changes (e.g., changing paddle speed doesn't change the spec hash, but produces different simulation results from the same inputs). Code hash derived from source files — the commit hash already identifies the exact source, making a separate source hash redundant.

**Rationale:** In deterministic lockstep, any code change — wire format, gameplay logic, constants, physics — produces different simulation results from the same inputs. The commit hash captures all of these because it identifies the exact source code. It changes automatically on every commit without anyone remembering to bump anything. For replay, the commit hash tells you exactly which code to check out and build. For session compatibility, same commit hash = same code = same simulation. The CI pipeline writes `$GITHUB_SHA` to the S3 version file, eliminating the manual version bump step entirely.

**See:** [distribution.md](architecture/distribution.md) — Version Check, [network-operations.md](architecture/network-operations.md) — Version Index

### Version isolation (no mid-session updates)

**Decision:** Running clients continue on their current version until relaunch. The relay groups clients by commit hash — different versions coexist independently with no interaction. No mid-session update notification, no background download, no grace period.

**Over:** Live update orchestration (relay polls version file, notifies clients, background download, grace period with version-aware routing).

**Rationale:** For a small invite-only group, coordinating a relaunch is trivial. The live update machinery adds substantial complexity for minimal benefit. Version isolation is simpler to implement and reason about, and naturally handles any number of concurrent versions.

**See:** [distribution.md](architecture/distribution.md) — Version Isolation

### Chat messages as game inputs

**Decision:** Chat is an Input with a chat-typed payload. The relay never distinguishes chat from game inputs. Chat gets tick ordering, logging, replay, and persistence from the existing architecture with zero relay changes.

**Over:** Separate `ClientMessage::Chat` variant — leaks game concepts into the relay protocol, requires relay changes for each new message type.

**Rationale:** The relay treats all inputs as opaque bytes. A chat message is just a payload variant that the client interprets. Chat automatically inherits tick ordering, full message logging, deterministic replay, and S3 persistence without any relay modifications. Adding new message types is a client-only change.

---

## Determinism

### Constrained f32 with software transcendentals

**Decision:** Use standard f32 for basic math (+, -, *, /, sqrt — already IEEE 754 deterministic). Use the `libm` crate for transcendental functions (sin, cos, etc.). Compile with `-C llvm-args=-fp-contract=off` to prevent FMA fusion. Use deterministic collections (BTreeMap, sorted iteration). Disable Bevy's parallel system execution for gameplay systems.

**Over:** Fixed-point math (awkward API, no Bevy integration), WASM as determinism layer (performance overhead), or accepting non-determinism.

**Rationale:** Patches the specific known problem spots (transcendentals, FMA, collection ordering, system parallelism) with minimal friction. Most code stays normal. Combined with checksums as a safety net.

**See:** [network-architecture.md](architecture/network-architecture.md) — Making P2P Determinism Practical

### Non-cryptographic checksums for drift detection

**Decision:** Periodic state checksums using xxhash (non-cryptographic, 64-bit). Exchanged every 30-60 ticks. Treat every desync as a determinism bug to investigate and fix.

**Over:** Cryptographic hashes (SHA-256) or no checksums.

**Rationale:** Players are trusted (design philosophy #11), so collision resistance against intentional forgery is unnecessary. Non-cryptographic hashes are orders of magnitude faster. 64-bit xxhash has a 1-in-2^64 probability of accidental false-match — effectively impossible. Checksums cost 8 bytes per exchange per peer.

**See:** [network-architecture.md](architecture/network-architecture.md) — State Checksums: Detecting Drift

---

## Debugging and Observability

### Full message logging, always on

**Decision:** Log every network message sent and received — player inputs, confirmed packages, checksums — with wall-clock timestamps, tick numbers, player slots, and payloads. On both clients and relay.

**Over:** Selective logging or no logging.

**Rationale:** In lockstep relay, total network traffic is ~19 KB/sec for 4 players. Logging everything with metadata is ~100 KB/sec — negligible. A one-hour session compresses to ~20-35 MB. This is practically free and enables deterministic replay, latency analysis, and desync debugging. Server-authoritative architectures can't do this because their traffic volume is orders of magnitude higher.

**See:** [network-operations.md](architecture/network-operations.md) — Message Logging and Deterministic Replay

### Deterministic replay from input logs

**Decision:** The complete input log is a complete game recording. Any session can be replayed by feeding the logged inputs into the simulation. No game state logging is needed.

**Over:** State-based replay (recording entity positions each frame) or no replay capability.

**Rationale:** Determinism guarantees that identical inputs produce identical state. The input log is tiny (KB/sec). State recording would be orders of magnitude larger and still less useful for debugging. Replay enables tick-by-tick inspection, desync diagnosis (replay on two instances and compare checksums), and distinguishing determinism bugs from network bugs.

**See:** [network-operations.md](architecture/network-operations.md) — Deterministic Replay

---

## Log Compaction

### Two-tier log model

**Decision:** The input log is split into two tiers. The hot tier lives in relay memory — input entries from the latest snapshot tick to the current tick. The cold tier is archived to S3 — the full session history from tick 0. Compaction advances the boundary by writing a world state snapshot to S3 and flushing older entries from the hot buffer to cold storage.

**Over:** Single-tier log where everything lives in relay memory or everything streams directly to S3 with no operational split.

**Rationale:** The relay needs fast access to recent inputs for player join catch-up and jitter absorption, but doesn't need the full session history in memory. Separating hot from cold keeps relay memory bounded while preserving the complete log for replay and debugging. The hot buffer size is determined by the compaction interval — at 2–5 minutes and 60 ticks/sec, this is 7,200–18,000 entries, well within memory.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

### Snapshots stored separately from the input log

**Decision:** World state snapshots are stored as separate S3 objects, not embedded in the input log. The log contains a snapshot marker event at each compaction tick, recording the tick number and S3 key of the corresponding snapshot.

**Over:** Embedding serialized world state inline in the input log.

**Rationale:** Input entries are small and uniform (~20 bytes each). World state snapshots can be KB to MB. Embedding snapshots inline would bloat the log and complicate sequential reads. Separate storage lets each artifact be read independently — operations (join, crash recovery) read only the snapshot; replay reads only the log. The marker event keeps the log self-describing without carrying the snapshot payload.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

### Compaction triggers

**Decision:** Compaction is triggered by four conditions: periodic interval during play (every 2–5 minutes), session end, player join when the hot log has grown large since the last compaction, and a safety-valve size threshold on the hot buffer.

**Over:** A single trigger (e.g., only periodic or only on session end).

**Rationale:** Multiple triggers ensure bounded hot buffer size under all conditions. Periodic compaction handles the steady state. Session-end compaction captures the final state. Join-triggered compaction reduces catch-up time for arriving players. The size threshold guards against edge cases where periodic timers drift or fail. At this data volume, the cost of extra S3 PUTs is negligible.

**See:** [network-operations.md](architecture/network-operations.md) — Log Compaction

---

## Persistence and Infrastructure

### Cloud storage for persistent state, not always-on servers

**Decision:** Save files stored in cheap object storage (S3, R2, B2). No always-on server. The relay runs only during active sessions. Between sessions, nothing is running and the only cost is storage (pennies/month).

**Over:** Always-on dedicated servers that hold world state.

**Rationale:** Traditional server-authoritative architectures conflate simulation compute, authoritative state, and persistent storage into one always-on server. In lockstep relay, every client holds the full state and the relay is stateless. The only between-session need is storage, which costs essentially nothing. Paying for 24/7 compute to store a few MB is wasteful.

**See:** [network-operations.md](architecture/network-operations.md) — Persistent State Between Sessions

### Tick-based sync protocol

**Decision:** Any client can sync the canonical save to S3 at any time. Check the stored tick number (HEAD request), upload only if ahead (PUT with tick in metadata), skip if behind or equal. Automatic periodic sync during play and final sync on session end.

**Over:** Designated-uploader protocols, lock-based sync, or streaming infrastructure.

**Rationale:** Determinism means two clients at the same tick have identical state. There is no merge, no diff, no conflict. Two clients racing to upload the same tick upload identical bytes. A lagging client checks, sees it's behind, and skips. The protocol is three steps: HEAD, compare, conditional PUT. Only authoritative state (confirmed ticks) is synced — never the latency state.

**See:** [network-operations.md](architecture/network-operations.md) — Sync Protocol

### S3 save + input log for player join

**Decision:** Players joining mid-session download the latest save from S3 and catch up via buffered input history. No existing client is involved in the join process.

**Over:** Existing client snapshots and sends state directly to the joining player.

**Rationale:** Zero impact on existing players — they don't even notice someone joined. S3 download is fast and reliable. Multiple players can join simultaneously. The catch-up window is bounded by the sync interval (at most a few thousand ticks of fast-forward simulation). Three separately-motivated features converge: S3 sync (persistence), message logging (debugging), and relay input buffer (jitter absorption) — all contribute to making join seamless.

**See:** [network-operations.md](architecture/network-operations.md) — Player Join Mid-Game

### No streaming infrastructure

**Decision:** Direct S3 API calls (PUT/GET/HEAD) for all persistence needs. No message brokers (Kafka, Kinesis), no event streams, no pub/sub systems.

**Over:** Kafka → S3 pipeline or similar streaming architectures.

**Rationale:** The relay handles ~19 KB/sec of input data for 4 players. This is far too little traffic to justify infrastructure designed for millions of messages per second. Kafka itself requires always-on servers, reintroducing the cost the architecture is designed to avoid. A periodic S3 PUT every few minutes is the entire persistence solution.

**See:** [network-operations.md](architecture/network-operations.md) — Mid-Session Durability

---

## Access Control

### Shared secret in the Hello handshake

**Decision:** The relay is protected by a shared passphrase. The operator sets the secret on the relay (environment variable or config file) and distributes it to invited players through an out-of-band channel (text message, Discord, phone call). The client includes the secret in the Hello handshake alongside the commit hash. The relay silently drops connections with an incorrect or missing secret — no error response, no acknowledgment. The secret is sent in plaintext over UDP.

**Over:** Per-user tokens with a revocation list (more infrastructure for a problem that doesn't exist at 0-10 trusted users), client certificates or key pairs (overengineered, contradicts "no cryptographic identity"), IP allowlists (fragile — residential IPs change, people play from different networks), and encrypted handshake (DTLS — all session traffic is already plaintext UDP; encrypting only the handshake solves nothing).

**Rationale:** The threat model is random strangers on the internet discovering the relay's IP and connecting — not malicious insiders or targeted surveillance. A shared secret stops port scanners, bots, and accidental connections. The group is small (0-10) and trusted, so the social cost of sharing one passphrase is near zero. Rotation is trivial — the operator changes it on the relay and tells everyone the new one. The secret is not stored in S3 or any cloud service; it exists only on the relay and on each client's local disk.

**See:** [network-operations.md](architecture/network-operations.md) — Relay Access Control

### Hello handshake carries commit hash, secret, and display name

**Decision:** All three fields in a single Hello message. The relay validates the secret first (silently drops if wrong), groups by commit hash (version isolation), and tracks the display name. One round-trip, no multi-step handshake.

**Over:** Multi-step handshake where secret validation, version check, and identity registration happen in separate exchanges.

**Rationale:** A single message keeps the protocol simple and minimizes round-trips. The relay already needs all three pieces of information before it can assign a player slot. Bundling them means a connecting client is either fully accepted or silently rejected in one exchange.

---

## Connectivity

### Fixed 30-second retry interval

**Decision:** When any network operation fails (version check, relay connection, S3 sync), the client retries on a fixed 30-second interval until it succeeds. No exponential backoff, no jitter, no adaptive timing.

**Over:** Exponential backoff (standard for distributed systems with shared servers) and adaptive intervals.

**Rationale:** Exponential backoff exists to protect shared servers from retry storms when many clients fail simultaneously. None of those conditions apply here: there is one client retrying, the targets are S3 (effectively infinite capacity) or a relay serving 0-10 users, and there is no thundering herd. A fixed interval gives the user a predictable countdown, and 30 seconds means connectivity is detected within half a minute of restoration. The cost of each retry is one small HTTP GET or UDP packet — negligible.

---

## Local Storage

### Local config in platform app data directory

**Decision:** Player display name and relay secret persist between launches in a TOML file at the platform-conventional location (`%APPDATA%\seans-arcade\config.toml` on Windows). Not alongside the binary, not in the registry.

**Over:** Config file next to the binary (binary self-replaces on update, complicating the update dance or risking config loss) and Windows registry (not portable, harder to inspect, platform-specific API).

**Rationale:** Platform app data directories survive application updates, are user-discoverable, and work the same conceptual way across platforms. TOML is human-readable and trivially editable. The file contains no secrets that need encryption — the relay secret is a shared passphrase, not a credential.

---

## Event Log Principles

### The canonical log is sufficient to deterministically recreate all game state

**Decision:** The canonical event log is the sole source of truth for what happened in a session. Given the log and the correct version of the simulation code, every game state at every tick can be recreated with zero ambiguity. The log contains every input, in order, with enough metadata to identify which code produced each event — the commit hash, logged on connection events, tells you exactly which code to check out and build for replay.

**Over:** Logs that require external context (out-of-band knowledge of what version was running, manual annotations) or logs that record derived state (entity positions, scores) instead of inputs.

**Rationale:** Deterministic simulation means inputs + code are the complete description of a session. Logging inputs is sufficient; logging derived state is redundant. But inputs alone are not sufficient without knowing which code to replay them with — the same inputs produce different results on different versions. The commit hash on connection events makes the log self-describing: check out that commit, build, feed in the inputs, get the exact simulation.

**See:** [network-operations.md](architecture/network-operations.md) — Message Logging and Deterministic Replay

### The log stores the minimum information needed for zero ambiguity

**Decision:** Information that can be derived from other logged events is not duplicated on every log entry. State that is constant across a connection (such as the commit hash) is logged once at the connection event, not repeated on every input from that connection.

**Over:** Per-event annotation (stamping every log entry with the commit hash or other connection-level metadata).

**Rationale:** The commit hash is identical for every message from a given connection — it cannot change without disconnecting and reconnecting, which produces a new connection event. Repeating it on every log entry adds bytes that carry zero additional information. With version isolation, logs are split by version, so every entry in a given log shares the same commit hash — recorded once per log. Minimum information means: log state changes, not state itself.

**See:** [network-operations.md](architecture/network-operations.md) — Version Index
