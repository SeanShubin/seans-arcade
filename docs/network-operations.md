# Network Operations: Running, Debugging & Maintaining

Companion to [network-architecture.md](network-architecture.md), which covers architecture and concepts. For the key decisions and their rationale, see [architecture-decisions.md](architecture-decisions.md). This document covers the operational side: how to monitor, debug, deploy, and maintain networked games using the lockstep relay model.

## Network Diagnostics

### Prediction Depth (How Far Ahead)

The gap between the latest unconfirmed input and the latest confirmed tick. "I've sent inputs through tick 505, but only have confirmations through tick 500 — I'm 5 ticks ahead."

This is **healthy and intentional**. It exists because latency hiding requires playing ahead of confirmation. It's roughly RTT divided by tick duration. At 50ms round-trip and 60 ticks/sec, prediction depth is ~3 ticks.

What to watch:
- **Stable value** — normal. Reflects latency to the relay.
- **Growing** — the relay is taking longer to confirm. Either latency is increasing or the relay is bogged down.
- **Very large** — high latency. The latency state is far from authoritative state, so when predictions are wrong the visual correction will be jarring.
- **Fluctuating** — jitter. The experience will feel inconsistent.

### Tick Deficit (How Far Behind)

This is actually two different problems that present the same way:

**Network starvation** — the relay has broadcast confirmed tick 500, but the client has only received through tick 495. Five confirmed packages are in flight or lost. The authoritative state can't advance because the inputs needed to simulate those ticks are missing.

**Processing starvation** — all confirmations have been received, but the CPU can't simulate fast enough to keep up. Ticks are piling up in the inbox.

Both result in the authoritative state falling behind, but the cause and fix are different. Network starvation needs better connection or packet recovery. Processing starvation means the game is too expensive to simulate at the target tick rate.

In normal operation this should be **zero**. Any non-zero value is a problem.

### Other Metrics

**Round-trip time (RTT)** — time from sending an input to receiving it back inside a confirmed package. The most fundamental measurement. Everything else derives from this.

**Jitter** — variance in RTT. Stable 50ms feels fine. Alternating 20ms-80ms-30ms-90ms feels terrible even though the average is similar. Input buffering absorbs jitter, but only up to the buffer depth.

**Input omissions** — how often the relay broadcasts a confirmed package with a player's input missing (because it arrived late). This means the player's character didn't act that tick. Occasional omissions are invisible. Frequent omissions mean buffer depth is too shallow for the jitter.

**Packet loss** — confirmed packages from the relay that never arrive. Distinct from input omissions (which are a player's inputs not reaching the relay in time). Lost confirmed packages cause tick deficit on the receiving client.

**Checksum agreement** — are periodic checksums matching across all clients? This isn't a latency metric — it's a correctness metric. A mismatch means a determinism bug, not a network problem.

### How Metrics Relate

```
RTT determines → prediction depth (how far ahead you play)
Jitter determines → required input buffer depth
Input buffer too shallow + jitter → input omissions (actions get dropped)
Packet loss → tick deficit (client falls behind)
CPU too slow → tick deficit (client falls behind)
Tick deficit → catch-up needed (fast-forward multiple ticks)
Prediction depth too large → visual corrections feel jarring
```

### Summary

| Metric | Normal value | Concerning when | Indicates |
|--------|-------------|----------------|-----------|
| Prediction depth | 2-5 ticks | Growing or fluctuating | Latency increasing or jitter |
| Tick deficit | 0 | Any non-zero | Network or CPU starvation |
| RTT | Stable, any value | Spiking or trending up | Connection quality |
| Jitter | Low variance | High variance | Unreliable connection |
| Input omissions | 0 | Frequent | Buffer depth too shallow |
| Packet loss | 0 | Frequent | Network degradation |
| Checksum match | Always | Ever mismatched | Determinism bug |

## Message Logging and Deterministic Replay

Checksums detect drift. Message logging and deterministic replay diagnose it. In the lockstep relay model, logging every message sent and received is not just feasible — it's practically free, and it gives you debugging tools that server-authoritative architectures can't match.

### Logging Principles

Three principles govern what goes into the log. For the formalized decisions and rationale, see [architecture-decisions.md](architecture-decisions.md) — Event Log Principles.

**The canonical log is sufficient to deterministically recreate all game state.** Given the log and the correct simulation code, every game state at every tick can be recreated with zero ambiguity. The log is self-describing — it contains enough metadata to determine what wire protocol specification each event conforms to, without requiring external knowledge of what version was running at that time.

**The log stores the minimum information needed for zero ambiguity.** Information that is constant across a connection (such as the spec hash) is logged once at the connection event, not repeated on every input from that connection. Log state changes, not state itself. If something can be derived from other logged events, it is not duplicated.

**Indexes are not redundant information.** A derived index that enables a capability the canonical log cannot provide on its own (such as random access without sequential scan) is maintained alongside the log. Derivability does not imply redundancy when the derived structure serves a purpose the source cannot perform.

### Why Full Logging Is Cheap

In lockstep relay, the **only** things on the wire are player inputs, confirmed input packages, and periodic checksums. There is no entity state replication, no position updates, no snapshot traffic.

| Message | Size | Frequency |
|---------|------|-----------|
| Player input | ~20 bytes (tick + slot + input data) | 60/sec per player |
| Confirmed input package | ~20 bytes × player count | 60/sec from relay |
| Checksum | 8 bytes | Every 30-60 ticks |

For 4 players at 60 ticks/second:
- Each client sends ~1.2 KB/sec and receives ~4.8 KB/sec
- The relay handles ~19 KB/sec total throughput
- Logging everything with timestamps and metadata: ~100 KB/sec on the relay

A one-hour session produces ~360 MB of uncompressed logs on the relay. Structured input data compresses extremely well (10-20×), so a full session is ~20-35 MB per hour. For comparison, a server-authoritative game replicating entity state for thousands of entities would be orders of magnitude more traffic and completely impractical to log fully.

### What to Log

On every message, both sides:

| Field | Purpose |
|-------|---------|
| Wall-clock timestamp | Latency measurement, jitter analysis |
| Tick number | Correlate across clients |
| Player slot | Who sent it |
| Direction (send/recv) | Trace message flow |
| Message type | Input, confirmed package, checksum |
| Payload | The actual input bytes or checksum value |

On connection events (Hello/disconnect), additionally:
- Spec hash — the hash of the connecting client's event type specifications, identifying the wire protocol format for all subsequent messages from this connection (see [architecture-decisions.md](architecture-decisions.md) — Specification hash for wire protocol compatibility)

On the relay, additionally:
- Time between receiving a player's input and broadcasting the confirmed package (relay processing latency)
- Which players' inputs were present vs omitted for each tick (detects who's lagging)

### Deterministic Replay

Because the simulation is deterministic, the complete input log **is** a complete recording of the game. No game state needs to be logged. Feed the same inputs into the simulation and it reproduces the exact same session, tick by tick.

This gives you:
- **Replay any session** from just the input log
- **Step through tick by tick** to find where behavior diverged from expectations
- **Reproduce bugs deterministically** — no "it only happens sometimes" if you have the inputs
- **Compare two clients' views** by replaying the same input log on two instances and checking where state diverges (desync debugging)

This is exactly how Factorio debugs desyncs — replay the input log, checksum every tick, find the first tick where states diverge, examine what's different.

### Debugging Scenarios

**"The game felt laggy"** — check the logs for round-trip times per tick. Were inputs arriving late? Was input buffering insufficient?

**"Players saw different things"** — replay both clients' input logs independently, checksum every tick, find the first divergence. Diff the game state at that tick to identify which component diverged.

**"One player kept stuttering"** — check the relay logs for that player's input arrival times. Were they jittering? Were inputs being omitted due to late arrival?

**"Something weird happened on tick 4000"** — replay the input log up to tick 3999, then step through tick 4000 examining state changes.

**"Is this a determinism bug or a network bug?"** — replay the same input log on two separate instances offline (no network). If checksums diverge, it's a determinism bug. If they match, the original divergence was caused by a network issue (dropped or reordered packet, relay bug).

### Version Index

The canonical log records the spec hash on connection events only — not on every input message (see Logging Principles above). To determine the wire protocol specification for any event in the log, a sequential reader tracks a `slot → spec hash` mapping as it processes connection events. Each input inherits the spec hash of its slot's most recent connection.

For random access — looking up the spec for an event at an arbitrary log position without reading from the beginning — the relay maintains a **version index** alongside the canonical log. The version index records one entry per version-change point per slot:

```
log_position 0:     slot 1 = 0xABCD1234
log_position 0:     slot 2 = 0xABCD1234
log_position 4200:  slot 1 = 0xEF567890
log_position 4205:  slot 2 = 0xEF567890
```

To find the spec hash for any event: look up the slot, find the most recent index entry at or before that log position. The index is a handful of entries per session — typically one per connection event, a few hundred bytes total.

The version index is **derived from** the canonical log. It can be deleted and rebuilt by replaying connection events. It adds no new information — it provides random access to information that already exists in the canonical log but would otherwise require sequential scan to reach.

## Connection Disruptions

### Player Leave: Trivial

A player disconnects. The relay stops expecting inputs from that slot. Other clients stop receiving inputs for that player. The simulation continues — the departed player's entities either get despawned or become inert (game design decision). Nothing to sync, nothing to recover.

### Relay Restart

The relay runs on AWS (Lightsail) and is managed by the platform's process manager. If it crashes, it restarts automatically. No player takes over as the relay — the relay is always on AWS.

The relay is stateless — no game state is lost when it restarts. Every client already has the full authoritative simulation. The only thing lost is the relay's in-memory connection state (connected clients, input buffer, recent input history).

**What happens:**
1. Relay process crashes
2. Lightsail restarts it
3. Clients detect the disconnection (connection timeout)
4. Clients reconnect to the relay
5. Clients report their last confirmed tick number
6. Relay takes the **minimum** confirmed tick and everyone resumes from there

**The tick gap problem:** When the relay dies, some inputs may have been in-flight — sent to the old relay process but never broadcast. Different clients may have received confirmed inputs through different ticks (client A got through tick 500, client B only through 498).

**Resolution:** The relay takes the minimum and everyone resumes from there. Clients who were a tick or two ahead discard those extra ticks. This is safe because the authoritative state only advances with confirmed inputs — rewinding to the minimum means replaying at most a few ticks. Unconfirmed inputs in each client's latency state get replayed through the relay naturally.

**Input history recovery:** The relay's input history buffer (used for player joins) is lost on restart. The relay rebuilds it from the point of restart. Any player joining immediately after a relay restart would need to download the S3 save and catch up from the restart tick, which is the normal join flow anyway.

### Player Join Mid-Game

A new player needs the full authoritative simulation state to start simulating. With periodic S3 sync in place (see [Persistent State Between Sessions](#persistent-state-between-sessions)), the joining player downloads from S3 rather than requesting state from an existing client. This makes joining **zero-impact on existing players**.

**Steps:**
1. New player downloads the latest save from S3 (at tick N, which may be a few minutes old)
2. New player connects to the relay
3. Relay provides confirmed input packages from tick N+1 through the current tick
4. New player loads the S3 state and fast-forwards through the buffered ticks (simulate without rendering)
5. New player is in sync and continues normally

No existing client does anything. They don't even notice someone joined until inputs from a new player slot start appearing in confirmed packages.

**Catch-up cost is small.** If S3 syncs every 2 minutes at 60 ticks/sec, that's at most ~7,200 ticks of catch-up. Simulating 7,200 ticks without rendering takes a fraction of a second. The input data for those ticks is ~576 KB for 4 players.

**Multiple players can join simultaneously** with no additional load — they're all downloading from S3 independently.

The relay needs to retain recent input history back to the latest S3 save tick, which it already does if message logging is enabled (see [Message Logging and Deterministic Replay](#message-logging-and-deterministic-replay)). The data is small enough to buffer in memory regardless.

**Fallback (without S3 sync):** An existing client snapshots their authoritative state and sends it directly to the joining player through the relay. This works but puts load on the sending client and bottlenecks on their upload bandwidth. The S3 approach is strictly better when periodic sync is in place.

Three previously separate features converge to make this work:

| Feature | Original motivation | Also enables |
|---------|-------------------|--------------|
| S3 periodic sync | Persistent state between sessions | Base state for joining players |
| Message logging | Debugging and replay | Catch-up input history for joining players |
| Relay input buffer | Absorbing jitter during play | Providing recent ticks to joining players |

### Difficulty Summary

| Scenario | Difficulty | Why |
|----------|-----------|-----|
| Player leave | Trivial | Stop expecting their inputs, continue simulation |
| Relay restart | Simple | Relay is stateless; Lightsail restarts it, clients reconnect, agree on last confirmed tick |
| Player join mid-game (with S3) | Easy | Download from S3, catch up from input log |
| Player join mid-game (without S3) | Moderate | Existing client must snapshot and send state through relay |

The stateless relay and full-simulation-on-every-client design means all three scenarios are simpler than in server-authoritative architectures, where the server holds all game state and losing it means losing the game.

## Connecting Over the Internet

With the relay lockstep model, NAT (routers blocking incoming connections) is a non-issue. Both clients make **outbound** connections to the relay server, and NAT routers allow outbound connections freely. The problem NAT creates — unsolicited incoming packets getting dropped — never applies because nobody connects directly to another player.

**Setup:**
1. Deploy a small Rust binary on a cheap cloud VM (~$4-6/month) with a public IP
2. The relay is a plain UDP socket application — no web server, no HTTP, no browser, no database. Just receives and forwards packets.
3. Players launch the game client, type in the relay's IP address (or domain name if you register one), and play. All coordination is invisible to the player.
4. Relay assigns player slots, merges inputs per tick, and forwards to all clients. No game logic.

For pong, total network traffic is ~540 bytes/second. The relay uses a negligible fraction of even the cheapest VM.

**Hosting options:** Any cheap VM works. AWS Lightsail (~$3.50/month), DigitalOcean/Vultr (~$4-6/month), or Fly.io (free tier may suffice). Lambda/serverless does NOT work — the relay needs a persistent UDP socket, not per-request invocations.

**Packaging:** A single compiled Rust binary with no dependencies. Can deploy as a bare binary or in a Docker container. Container adds auto-restart (`--restart=always`) and repeatable deployment for minimal extra effort. Required if using Fly.io. Either approach works on a plain VM.

## Persistent State Between Sessions

### The Problem with Always-On Servers

Most multiplayer games require renting a server that runs 24/7. The server holds the world state, runs the simulation, and must be available whenever someone might want to play. You pay for compute around the clock, even though the server sits idle most of the time. For a small group of friends who play a few hours a week, you're paying for 168 hours of compute to use maybe 10.

The fundamental issue: traditional server-authoritative architectures **conflate three concerns**:

| Concern | What it actually needs | Traditional server | Lockstep relay |
|---------|----------------------|-------------------|----------------|
| Simulation compute | CPU during play | Server runs 24/7 | Clients compute; relay is near-free |
| Authoritative state | One source of truth during play | Server holds it | Every client holds it |
| Persistent storage | Save file between sessions | Server's disk (must stay on) | Any client's disk or cloud storage |

Always-on hosting pays for **compute** when what you actually need is **storage**. Storage costs essentially nothing.

### Why Lockstep Relay Doesn't Have This Problem

In the lockstep relay model, every client runs the full simulation and holds the complete authoritative state at all times. The relay is stateless — it never has game state to persist. When a session ends, every client that was playing holds a complete, identical copy of the world.

This is the Grounded model — one player hosts, the world lives on their machine, and when nobody's playing, nothing is running. No server costs. The world resumes when someone launches it.

### Where the Save File Lives

Three options, from simplest to most flexible:

**Option A: Host carries the save (Grounded model)**
- One player is the save owner — the file lives on their machine
- They start a session by launching the relay and loading the save
- Other players join, receive state transfer, play
- When the session ends, the host saves locally
- Any client could also save locally as a backup (they have identical state)
- **Cost: $0**
- **Limitation:** the save owner must be online to start a session

**Option B: Cloud storage (S3 bucket)**
- Save files stored in cheap object storage (AWS S3, Cloudflare R2, Backblaze B2)
- When anyone wants to play, they download the latest save, start a relay, and host
- When the session ends, the save uploads back to storage
- Any player can start a session, not just the original host
- **Cost:** pennies/month for storage (a save file is KB to low MB), zero compute when idle

**Option C: Every client keeps a copy + cloud sync**
- Every client that was in the session already has the complete authoritative state (natural consequence of lockstep)
- Between sessions, each player has a valid save file locally
- Cloud storage is a sync point, not a requirement — upload after sessions so that a player who wasn't in the last session can grab the latest
- **Cost:** pennies/month if you add cloud sync, $0 if you don't

### Sync Protocol

Since the simulation is deterministic, two clients at the same tick have **identical state** by definition. There is no merge, no diff, no conflict resolution. Any client can sync the canonical save to cloud storage at any time:

1. HEAD request to S3 — read the tick number from object metadata (cheap, fast)
2. If your authoritative tick > stored tick, upload your state (PUT with your tick in metadata)
3. If your authoritative tick <= stored tick, do nothing — you're behind or equal

Two clients racing to upload the same tick? Doesn't matter which wins — they're uploading identical bytes. A lagging client checks, sees it's behind, skips. A client that played further uploads and advances the canonical state.

**Important:** only sync the **authoritative** state (confirmed ticks only), not the latency state. Unconfirmed predicted inputs must not be baked into the canonical save.

This can happen automatically — periodic sync during play (every few minutes), and a final sync when a session ends. No manual save management needed.

### Mid-Session Durability

If every client crashes simultaneously mid-session, any unsaved progress since the last sync is lost. For most games this is an acceptable risk (auto-save every few minutes, lose at most a few minutes of play). If stronger durability is needed:

- Any client can auto-save locally at regular intervals (they already have full state)
- The relay can request periodic state snapshots to upload to S3
- On crash recovery, download the last save from S3 and resume from that tick

No streaming infrastructure (Kafka, Kinesis, etc.) is needed. A periodic S3 PUT every few minutes is sufficient. The relay handles ~19 KB/sec of input data for 4 players — this is far too little traffic to justify a message broker designed for millions of messages per second.

### Cost Comparison

| Model | Monthly cost (idle) | Monthly cost (active) | Who can start a session? |
|-------|--------------------|-----------------------|--------------------------|
| Traditional server-authoritative | $4-50/month (always on) | Same | Anyone (server always running) |
| Host carries save (Option A) | $0 | $0 | Save owner only |
| Cloud storage (Option B) | ~$0.01 (S3 storage) | ~$0.01 + relay compute | Anyone |
| Client copies + cloud sync (Option C) | ~$0.01 (S3 storage) | ~$0.01 + relay compute | Anyone who has a copy |

The relay compute cost during active play is the same as in the "Connecting Over the Internet" section — a cheap Lightsail VM at $3.50/month. The relay runs only during active sessions; between sessions, the only cost is S3 storage.

## Project Structure for Networked Examples

Standalone examples (single-file, run with `cargo run --example`) stay in `examples/`. Networked examples that need separate binaries (relay server + game client) live as workspace crates under `crates/`.

```
bevy-prototyping/
├── Cargo.toml              # root [package] + [workspace]
├── src/lib.rs              # shared utilities
├── examples/               # standalone examples (cargo run --example pong)
│   ├── pong.rs
│   ├── neon_pong.rs
│   └── ...
├── crates/                 # multi-crate workspace members
│   ├── spec_hash/          # SpecHash trait + derive macro + primitive impls
│   │   ├── Cargo.toml      # proc-macro = true
│   │   └── src/lib.rs
│   ├── relay/              # relay server (game-agnostic input coordinator)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs     # binary entry point
│   │       └── lib.rs      # shared protocol types (messages, tick types)
│   └── net_pong/           # networked pong client
│       ├── Cargo.toml
│       └── src/main.rs
```

| Command | What it runs |
|---------|-------------|
| `cargo run --example pong` | Standalone pong (unchanged) |
| `cargo run -p relay` | Relay server |
| `cargo run -p net_pong` | Networked pong client |
| `cargo clippy --workspace -- -D warnings` | Lint everything |

### Game-Agnostic Relay

The relay server knows nothing about pong, health bars, or any game concept. It coordinates **opaque input bytes** organized by tick and player slot. Any game that implements the same message protocol can reuse the same relay binary. The game-specific logic lives entirely in the client crate (e.g., `net_pong`).

The relay's `lib.rs` exports the shared protocol types (message enums, tick ID, player slot) so both the relay and client crates can depend on them without duplicating definitions.
