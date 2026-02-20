# Architecture Decisions

Formalized technical decisions with rationale. Each decision is stated declaratively. Detailed explanations live in the referenced documents. Game design principles that motivate these decisions live in [design-philosophy.md](design-philosophy.md).

This document records decisions that have been made. It is not a wishlist or a plan. If a decision is here, it's the current direction.

---

## Networking Model

### Deterministic lockstep with relay server

**Decision:** All clients run the full simulation independently from shared inputs. A lightweight relay server orders and broadcasts inputs but performs no simulation and holds no game state.

**Over:** Server-authoritative (server simulates, clients render) and peer-to-peer rollback (clients simulate with rollback on misprediction).

**Rationale:** Trust between players (design philosophy #11) means no resources should be spent on server-side validation. Lockstep minimizes bandwidth (only inputs travel, not entity state) and eliminates the need for a powerful central server. The relay is stateless, cheap, and trivially replaceable.

**See:** [network-architecture.md](network-architecture.md) — How Lockstep Relay Works

### Latency hiding, not rollback

**Decision:** Each client maintains two separate states — authoritative (confirmed inputs only) and latency (authoritative + unconfirmed local inputs rebuilt each tick). The authoritative state is never wrong and never rewound.

**Over:** Rollback, where the authoritative state advances with predicted inputs and must rewind/re-simulate when predictions are wrong.

**Rationale:** The authoritative state is never temporarily wrong, so there's no correction cost and no visual "snap-back." The latency state is disposable — rebuilt fresh every tick from the authoritative state. Simpler to reason about: one state is always correct, the other is always a guess.

**See:** [network-architecture.md](network-architecture.md) — Latency Hiding: Prediction Without Rollback

### Relay drops slow inputs rather than stalling

**Decision:** If a player's input hasn't arrived by the time the relay needs to broadcast the confirmed package for a tick, the relay omits that player's input and broadcasts without it. The slow player's character doesn't act that tick.

**Over:** Pure lockstep, where the simulation stalls until all inputs arrive.

**Rationale:** Pure lockstep lets one slow connection freeze every player. Dropping and continuing keeps the game running for everyone. The slow player catches up by processing multiple ticks per frame when their connection recovers. This is the change that took Factorio from ~24 players to 400+.

**See:** [network-architecture.md](network-architecture.md) — Stalls vs. Dropping

### Commit hash as the single code identifier

**Decision:** The git commit hash is the single identifier for "this exact code." It is embedded at build time (build script reads `git rev-parse HEAD`), included in the Hello handshake, and logged on connection events. It replaces both the manually-bumped application version integer and any derived code/spec hashes.

**Over:** Manually-bumped version integer (`VERSION: u32 = 7`) — requires remembering to bump, and forgetting silently allows incompatible clients. Spec hash derived from event type definitions — only detects wire format changes, not gameplay logic changes (e.g., changing paddle speed doesn't change the spec hash, but produces different simulation results from the same inputs). Code hash derived from source files — the commit hash already identifies the exact source, making a separate source hash redundant.

**Rationale:** In deterministic lockstep, any code change — wire format, gameplay logic, constants, physics — produces different simulation results from the same inputs. The commit hash captures all of these because it identifies the exact source code. It changes automatically on every commit without anyone remembering to bump anything. For replay, the commit hash tells you exactly which code to check out and build. For session compatibility, same commit hash = same code = same simulation. The CI pipeline writes `$GITHUB_SHA` to the S3 version file, eliminating the manual version bump step entirely.

**See:** [distribution.md](distribution.md) — Version Check, [network-operations.md](network-operations.md) — Version Index

### Version-aware connection routing during grace period

**Decision:** During the grace period when the relay accepts connections from clients updating to a new version, the relay does not mix clients with different commit hashes into the same input broadcast. A client connecting with a new commit hash either waits in a lobby or joins a separate input pool. The relay uses the commit hash from the Hello handshake to enforce this — one comparison at connection time, zero per-message cost.

**Over:** No separation (let mixed-version sessions drain naturally) and per-message version tagging (version byte on every message envelope).

**Rationale:** The relay treats payloads as opaque bytes. Without version-aware routing, a fast-updating client gets merged into the same input pool as remaining clients running old code. Even if the wire format hasn't changed, different code produces different simulation results from the same inputs — checksums detect the divergence but can't recover from a code mismatch. Separating by commit hash at connection time prevents the problem at its source.

**See:** [distribution.md](distribution.md) — Live Update Orchestration

---

## Determinism

### Constrained f32 with software transcendentals

**Decision:** Use standard f32 for basic math (+, -, *, /, sqrt — already IEEE 754 deterministic). Use the `libm` crate for transcendental functions (sin, cos, etc.). Compile with `-C llvm-args=-fp-contract=off` to prevent FMA fusion. Use deterministic collections (BTreeMap, sorted iteration). Disable Bevy's parallel system execution for gameplay systems.

**Over:** Fixed-point math (awkward API, no Bevy integration), WASM as determinism layer (performance overhead), or accepting non-determinism.

**Rationale:** Patches the specific known problem spots (transcendentals, FMA, collection ordering, system parallelism) with minimal friction. Most code stays normal. Combined with checksums as a safety net.

**See:** [network-architecture.md](network-architecture.md) — Making P2P Determinism Practical

### Non-cryptographic checksums for drift detection

**Decision:** Periodic state checksums using xxhash (non-cryptographic, 64-bit). Exchanged every 30-60 ticks. Treat every desync as a determinism bug to investigate and fix.

**Over:** Cryptographic hashes (SHA-256) or no checksums.

**Rationale:** Players are trusted (design philosophy #11), so collision resistance against intentional forgery is unnecessary. Non-cryptographic hashes are orders of magnitude faster. 64-bit xxhash has a 1-in-2^64 probability of accidental false-match — effectively impossible. Checksums cost 8 bytes per exchange per peer.

**See:** [network-architecture.md](network-architecture.md) — State Checksums: Detecting Drift

---

## Debugging and Observability

### Full message logging, always on

**Decision:** Log every network message sent and received — player inputs, confirmed packages, checksums — with wall-clock timestamps, tick numbers, player slots, and payloads. On both clients and relay.

**Over:** Selective logging or no logging.

**Rationale:** In lockstep relay, total network traffic is ~19 KB/sec for 4 players. Logging everything with metadata is ~100 KB/sec — negligible. A one-hour session compresses to ~20-35 MB. This is practically free and enables deterministic replay, latency analysis, and desync debugging. Server-authoritative architectures can't do this because their traffic volume is orders of magnitude higher.

**See:** [network-operations.md](network-operations.md) — Message Logging and Deterministic Replay

### Deterministic replay from input logs

**Decision:** The complete input log is a complete game recording. Any session can be replayed by feeding the logged inputs into the simulation. No game state logging is needed.

**Over:** State-based replay (recording entity positions each frame) or no replay capability.

**Rationale:** Determinism guarantees that identical inputs produce identical state. The input log is tiny (KB/sec). State recording would be orders of magnitude larger and still less useful for debugging. Replay enables tick-by-tick inspection, desync diagnosis (replay on two instances and compare checksums), and distinguishing determinism bugs from network bugs.

**See:** [network-operations.md](network-operations.md) — Deterministic Replay

---

## Persistence and Infrastructure

### Cloud storage for persistent state, not always-on servers

**Decision:** Save files stored in cheap object storage (S3, R2, B2). No always-on server. The relay runs only during active sessions. Between sessions, nothing is running and the only cost is storage (pennies/month).

**Over:** Always-on dedicated servers that hold world state.

**Rationale:** Traditional server-authoritative architectures conflate simulation compute, authoritative state, and persistent storage into one always-on server. In lockstep relay, every client holds the full state and the relay is stateless. The only between-session need is storage, which costs essentially nothing. Paying for 24/7 compute to store a few MB is wasteful.

**See:** [network-operations.md](network-operations.md) — Persistent State Between Sessions

### Tick-based sync protocol

**Decision:** Any client can sync the canonical save to S3 at any time. Check the stored tick number (HEAD request), upload only if ahead (PUT with tick in metadata), skip if behind or equal. Automatic periodic sync during play and final sync on session end.

**Over:** Designated-uploader protocols, lock-based sync, or streaming infrastructure.

**Rationale:** Determinism means two clients at the same tick have identical state. There is no merge, no diff, no conflict. Two clients racing to upload the same tick upload identical bytes. A lagging client checks, sees it's behind, and skips. The protocol is three steps: HEAD, compare, conditional PUT. Only authoritative state (confirmed ticks) is synced — never the latency state.

**See:** [network-operations.md](network-operations.md) — Sync Protocol

### S3 save + input log for player join

**Decision:** Players joining mid-session download the latest save from S3 and catch up via buffered input history. No existing client is involved in the join process.

**Over:** Existing client snapshots and sends state directly to the joining player.

**Rationale:** Zero impact on existing players — they don't even notice someone joined. S3 download is fast and reliable. Multiple players can join simultaneously. The catch-up window is bounded by the sync interval (at most a few thousand ticks of fast-forward simulation). Three separately-motivated features converge: S3 sync (persistence), message logging (debugging), and relay input buffer (jitter absorption) — all contribute to making join seamless.

**See:** [network-operations.md](network-operations.md) — Player Join Mid-Game

### No streaming infrastructure

**Decision:** Direct S3 API calls (PUT/GET/HEAD) for all persistence needs. No message brokers (Kafka, Kinesis), no event streams, no pub/sub systems.

**Over:** Kafka → S3 pipeline or similar streaming architectures.

**Rationale:** The relay handles ~19 KB/sec of input data for 4 players. This is far too little traffic to justify infrastructure designed for millions of messages per second. Kafka itself requires always-on servers, reintroducing the cost the architecture is designed to avoid. A periodic S3 PUT every few minutes is the entire persistence solution.

**See:** [network-operations.md](network-operations.md) — Mid-Session Durability

---

## Event Log Principles

### The canonical log is sufficient to deterministically recreate all game state

**Decision:** The canonical event log is the sole source of truth for what happened in a session. Given the log and the correct version of the simulation code, every game state at every tick can be recreated with zero ambiguity. The log contains every input, in order, with enough metadata to identify which code produced each event — the commit hash, logged on connection events, tells you exactly which code to check out and build for replay.

**Over:** Logs that require external context (out-of-band knowledge of what version was running, manual annotations) or logs that record derived state (entity positions, scores) instead of inputs.

**Rationale:** Deterministic simulation means inputs + code are the complete description of a session. Logging inputs is sufficient; logging derived state is redundant. But inputs alone are not sufficient without knowing which code to replay them with — the same inputs produce different results on different versions. The commit hash on connection events makes the log self-describing: check out that commit, build, feed in the inputs, get the exact simulation.

**See:** [network-operations.md](network-operations.md) — Message Logging and Deterministic Replay

### The log stores the minimum information needed for zero ambiguity

**Decision:** Information that can be derived from other logged events is not duplicated on every log entry. State that is constant across a connection (such as the commit hash) is logged once at the connection event, not repeated on every input from that connection.

**Over:** Per-event annotation (stamping every log entry with the commit hash or other connection-level metadata).

**Rationale:** The commit hash is identical for every message from a given connection — it cannot change without disconnecting and reconnecting, which produces a new connection event. Repeating it on every log entry adds bytes that carry zero additional information. The connection event already establishes the mapping from slot to commit hash; every subsequent input from that slot inherits it until the next connection event. Minimum information means: log state changes, not state itself.

**See:** [network-operations.md](network-operations.md) — Version Index

### Indexes are not redundant information

**Decision:** Derived index structures that enable capabilities the canonical log cannot provide on its own are maintained alongside the log. The version index — a small structure mapping (log position, slot) to commit hash at version-change points — is maintained because it enables random access to version context without sequential scan of the canonical log.

**Over:** Treating all derived structures as redundant and requiring sequential log scan for every query.

**Rationale:** An index serves a specific purpose that cannot be achieved otherwise. The canonical log is sequential — to determine the commit hash for an event at log position N, you must scan backwards to find the most recent connection event for that slot. The version index eliminates this scan with a direct lookup. The index is derived from the canonical log (it can be rebuilt by replaying connection events) and adds no new information, but it provides a capability — random access — that the sequential log cannot. Derivability does not imply redundancy when the derived structure enables a function the source cannot perform.
