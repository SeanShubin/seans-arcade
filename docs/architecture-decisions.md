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

### Specification hash for wire protocol compatibility

**Decision:** Wire protocol compatibility is determined by a hash of the event type specifications (their structure, field types, and ordering), not by the application version integer. The spec hash is a compiled-in constant, computed from the event type definitions. The Hello handshake includes the spec hash. The application version (`VERSION: u32`) and the wire protocol spec hash are independent — a gameplay-only change does not change the spec hash, and a wire format change is detected automatically without anyone remembering to bump a number.

**Over:** Using the application version integer for wire compatibility (conflates application changes with protocol changes — a gameplay hotfix forces unnecessary incompatibility, and a forgotten version bump silently allows incompatible clients).

**Rationale:** Separation of concerns. The application version tracks releases. The spec hash tracks what the bytes on the wire mean. Two builds with different gameplay but identical event schemas produce the same spec hash and are wire-compatible. A build that adds a field to an input event produces a different spec hash automatically. This eliminates both false incompatibility (unnecessary rejection) and false compatibility (silent corruption).

**See:** [seans-arcade-plan.md](seans-arcade-plan.md) — Version Check, [network-operations.md](network-operations.md) — Version Index

### Global spec hash, not per-game-mode

**Decision:** The application has a single global spec hash derived from a root input type (e.g., `GameInput`) that encompasses all game modes. If any input type in the tree changes — chat, pong, or any future game — the spec hash changes.

**Over:** Per-game-mode spec hashes (each game mode tracks its own wire compatibility independently).

**Rationale:** Simplicity. One hash, one compatibility check, one value in the Hello handshake. The relay doesn't need to know which game mode a session is running — it just compares spec hashes. Per-game-mode hashing would require the relay to understand game modes, track which hash applies to which session, and add routing complexity for a problem that doesn't exist at this scale. All clients run the same binary with the same input type tree. If any part of that tree changes, the binary is different and compatibility should be rechecked.

### Derive macro for spec hash computation

**Decision:** The spec hash is computed at compile time by a proc macro (`#[derive(SpecHash)]`) that walks the type's AST. A `SpecHash` trait provides a single associated const:

```rust
trait SpecHash {
    const HASH: u64;
}
```

The derive macro on a type produces a const hash from the type's structure — variant names, field names, field types, and ordering. For types that contain other types (e.g., `GameInput` containing `ChatInput`), the generated impl references the child type's `SpecHash::HASH` const, so the compiler resolves the full type tree at compile time. Primitive types (`String`, `u8`, `Vec<T>`, etc.) have blanket impls provided by the `spec_hash` crate. The application's global spec hash is `<GameInput as SpecHash>::HASH`.

**Over:** Schema-first (maintaining a separate schema file like protobuf and generating Rust types from it) and explicit const (manually maintaining a string representation of the type structure).

**Rationale:** The derive macro operates directly on the Rust type definitions — the types *are* the specification, not a separate schema that must be kept in sync. The hash changes automatically when the type changes, without anyone remembering to update anything. Schema-first adds a maintenance burden (two representations of the same types) and a build step. Explicit const is simpler but relies on a human to update a string when the type changes — the same class of error the spec hash is designed to eliminate. The proc macro inspects the AST deterministically, so the same type definition always produces the same hash on every compilation and platform.

**Crate structure:** Rust requires proc macros in a separate crate. The `spec_hash` crate provides the trait, the derive macro, and blanket impls for primitive types.

**See:** [seans-arcade-plan.md](seans-arcade-plan.md) — Version Check

### Version-aware connection routing during grace period

**Decision:** During the grace period when the relay accepts both version N and N+1 connections, the relay does not mix them into the same input broadcast. A v(N+1) client connecting during the grace period either waits in a lobby or joins a separate v(N+1) input pool. The relay uses the spec hash from the Hello handshake to enforce this — one comparison at connection time, zero per-message cost.

**Over:** No separation (let mixed-version sessions drain naturally) and per-message version tagging (version byte on every message envelope).

**Rationale:** The relay treats payloads as opaque bytes. Once inputs are in the history buffer, there is no way to distinguish one format from another. Without version-aware routing, a fast-updating client that reconnects with a new spec hash gets merged into the same input pool as remaining clients with the old spec hash. The confirmed input packages then contain a mix of incompatible payload formats that no client can correctly interpret. Checksums detect the divergence but the recovery path (state snapshot from host) cannot fix a code mismatch — the problem is that clients are running different binaries, not that state drifted. Separating by spec hash at connection time prevents the problem at its source.

**See:** [seans-arcade-plan.md](seans-arcade-plan.md) — Live Update Orchestration

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

**Decision:** The canonical event log is the sole source of truth for what happened in a session. Given the log and the correct version of the simulation code, every game state at every tick can be recreated with zero ambiguity. The log contains every input, in order, with enough metadata to fully interpret each event — including which wire protocol specification each event conforms to.

**Over:** Logs that require external context (out-of-band knowledge of what version was running, manual annotations) or logs that record derived state (entity positions, scores) instead of inputs.

**Rationale:** Deterministic simulation means inputs are the complete description of a session. Logging inputs is sufficient; logging derived state is redundant. But inputs alone are not sufficient if you cannot tell what format they're in — a bag of opaque bytes with no specification context is uninterpretable. The log must be self-describing: a reader with access to the event type specifications (keyed by spec hash) can interpret any event in the log without external knowledge of what version was running at that time.

**See:** [network-operations.md](network-operations.md) — Message Logging and Deterministic Replay

### The log stores the minimum information needed for zero ambiguity

**Decision:** Information that can be derived from other logged events is not duplicated on every log entry. State that is constant across a connection (such as the spec hash) is logged once at the connection event, not repeated on every input from that connection.

**Over:** Per-event annotation (stamping every log entry with the spec hash or other connection-level metadata).

**Rationale:** The spec hash is identical for every message from a given connection — it cannot change without disconnecting and reconnecting, which produces a new connection event. Repeating it on every log entry adds bytes that carry zero additional information. The connection event already establishes the mapping from slot to spec hash; every subsequent input from that slot inherits it until the next connection event. Minimum information means: log state changes, not state itself.

**See:** [network-operations.md](network-operations.md) — Version Index

### Indexes are not redundant information

**Decision:** Derived index structures that enable capabilities the canonical log cannot provide on its own are maintained alongside the log. The version index — a small structure mapping (log position, slot) to spec hash at version-change points — is maintained because it enables random access to spec hash context without sequential scan of the canonical log.

**Over:** Treating all derived structures as redundant and requiring sequential log scan for every query.

**Rationale:** An index serves a specific purpose that cannot be achieved otherwise. The canonical log is sequential — to determine the spec hash for an event at log position N, you must scan backwards to find the most recent connection event for that slot. The version index eliminates this scan with a direct lookup. The index is derived from the canonical log (it can be rebuilt by replaying connection events) and adds no new information, but it provides a capability — random access — that the sequential log cannot. Derivability does not imply redundancy when the derived structure enables a function the source cannot perform.
