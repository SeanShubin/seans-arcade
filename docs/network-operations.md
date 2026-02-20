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

**The canonical log is sufficient to deterministically recreate all game state.** Given the log and the correct simulation code, every game state at every tick can be recreated with zero ambiguity. The log is self-describing — connection events record the git commit hash, which identifies exactly which code to check out and build for replay.

**The log stores the minimum information needed for zero ambiguity.** Information that is constant across a connection (such as the commit hash) is logged once at the connection event, not repeated on every input from that connection. Log state changes, not state itself. If something can be derived from other logged events, it is not duplicated.

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
- Commit hash — the git commit hash of the connecting client's build, identifying the exact code version for all subsequent messages from this connection (see [architecture-decisions.md](architecture-decisions.md) — Commit hash as the single code identifier)

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

With version isolation, logs are split by version — every entry in a given log shares the same commit hash. The commit hash is recorded once per log. A separate version index tracking version-change points within a log is no longer needed.

To replay a log, check out the commit corresponding to its recorded hash and build from that source. The log plus the commit hash fully identifies the code and inputs needed for deterministic replay.

## Connection Disruptions

### Player Leave: Trivial

A player disconnects. The relay stops expecting inputs from that slot. Other clients stop receiving inputs for that player. The simulation continues — the departed player's entities either get despawned or become inert (game design decision). Nothing to sync, nothing to recover.

### Relay Restart

There are two distinct scenarios that cause clients to lose their relay connection:

- **Process crash** — the relay process dies unexpectedly. The hosting platform restarts it automatically. Clients reconnect and resume. Covered in detail below.
- **Protocol change** — CI deploys a new relay binary with an updated wire protocol. The relay restart disconnects all clients. Clients auto-update on next launch via the startup flow. See [distribution.md](distribution.md) — Version Isolation.

**Process crash recovery:**

The relay runs on AWS. If it crashes, the hosting platform restarts it. No player takes over as the relay — the relay is infrastructure, not a player role.

The relay is stateless — no game state is lost when it restarts. Every client already has the full authoritative simulation. The only thing lost is the relay's in-memory connection state (connected clients, input buffer, recent input history).

**What happens:**
1. Relay process crashes
2. Hosting platform restarts it (or it is restarted manually)
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
| Relay restart | Simple | Relay is stateless; hosting platform restarts it, clients reconnect, agree on last confirmed tick |
| Player join mid-game (with S3) | Easy | Download from S3, catch up from input log |
| Player join mid-game (without S3) | Moderate | Existing client must snapshot and send state through relay |

The stateless relay and full-simulation-on-every-client design means all three scenarios are simpler than in server-authoritative architectures, where the server holds all game state and losing it means losing the game.

## AWS Infrastructure

### What Runs on AWS

The goal is to get clients talking to each other. Clients never communicate directly — all traffic flows through a relay. The infrastructure below supports this.

| Service | Purpose | Implementation |
|---------|---------|----------------|
| **Static website** | Serves seanshubin.com — download links, info page | S3 + CloudFront (or just S3 static hosting) |
| **Relay server** | Forwards inputs between clients, assigns player slots, manages connections | Single Rust binary on a cheap cloud VM (e.g., AWS Lightsail at ~$3.50/month) |
| **Version file** | Source of truth for current application version | Single file in S3 containing the commit hash |
| **Binary downloads** | Platform-specific executables for auto-update | S3 |
| **Save files** | Persistent game state between sessions | S3 |

### What Runs on the User's Machine

The downloaded application is a **client only**. It never accepts incoming connections and never acts as a relay. Every client makes an outbound connection to the AWS relay.

| Responsibility | Details |
|---------------|---------|
| **Full game simulation** | Every client runs the complete deterministic simulation independently |
| **Local input capture** | Captures player input and sends it to the relay |
| **Rendering** | Displays the latency state (authoritative + unconfirmed local inputs) |
| **Auto-update** | Checks version, downloads new binary when needed |

### How Clients Connect

All communication flows through the AWS relay. Clients never talk to each other directly. NAT traversal is a non-issue because every client makes only outbound connections.

```
Client A ──outbound──→ AWS Relay ←──outbound── Client B
Client C ──outbound──→ AWS Relay ←──outbound── Client D
```

When a user launches the application:
1. Version check against `seanshubin.com/version` (auto-update if stale)
2. Connect to the relay on AWS
3. Hello handshake — send commit hash, shared secret, and display name
4. Relay validates the secret — match continues, mismatch silently drops the connection
5. Relay assigns a player slot
6. Begin sending/receiving inputs

### Relay Access Control

The relay is protected by a shared passphrase. For the decision and rationale, see [architecture-decisions.md](architecture-decisions.md) — Shared secret in the Hello handshake.

**Where the secret lives:**

| Location | Form | Details |
|----------|------|---------|
| Relay | Environment variable or config file | The accepted secret. Not in source control. |
| Client (local disk) | Stored after first entry | Client prompts on first launch, remembers for subsequent launches. |
| S3 | Nowhere | The secret is never in cloud storage. |

**Relay behavior on invalid secret:** No response. The relay silently drops the packet. From the outside, the relay looks like a closed port — no information is leaked about whether the relay exists, whether the secret was wrong, or what protocol is in use.

**Client behavior on no response:** The client treats it the same as a connection timeout. The UI shows a connection failure, not a distinct "wrong secret" error — this avoids leaking information if someone is probing.

**Secret rotation:** The operator changes the environment variable or config file on the relay and restarts (or the relay hot-reloads, if supported). The operator tells all players the new secret through the same out-of-band channel. At 0-10 people this takes seconds. Connected clients are unaffected until they reconnect — the secret is only checked during the Hello handshake.

**Plaintext transmission:** The secret is sent in plaintext over UDP. This is consistent with the rest of the protocol — all session traffic (inputs, confirmed packages, checksums) is also plaintext UDP. The secret's purpose is to stop unsolicited connections from strangers, not to resist an attacker who can read packets in transit. See the note on who can observe UDP traffic below.

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

## Cost Estimate (0-10 Users)

Assumes 0-10 users online randomly throughout the month, averaging ~2 hours/day of activity, not all at once.

### Fixed Costs (Always Running)

| Service | What | Monthly Cost |
|---------|------|-------------|
| Route 53 hosted zone | DNS for seanshubin.com | $0.50 |
| S3 storage | Website + binaries (~100MB) | $0.003 |
| Domain renewal | seanshubin.com (already owned) | ~$1 amortized ($10-12/year) |

### Relay

The relay needs to be reachable by all clients. An always-on VM is the simplest way to achieve this, but not the only way — on-demand startup, serverless WebSocket, or other approaches could also work. The requirement is that clients can connect to the relay, not that the relay is always running.

**Option A: Cheap cloud VM (e.g., AWS Lightsail — recommended for v1)**

| Service | What | Monthly Cost |
|---------|------|-------------|
| Cloud VM | 512MB instance running the relay | ~$3.50 |
| Data transfer | Chat messages, 0-10 users | $0.00 (included) |
| S3 requests | Save file sync | $0.00 (pennies) |

**Total: ~$5/month.** The machine sits idle most of the time, but at ~$3.50/month the simplicity of "always ready, no cold starts" is worth more than the savings from on-demand startup.

**Option B: Serverless relay (API Gateway WebSocket + Lambda)**

| Service | What | Monthly Cost |
|---------|------|-------------|
| API Gateway WebSocket | Connection relay | $0.04 |
| Lambda | Message routing logic | $0.00 (free tier) |
| S3 requests | Save file sync | $0.00 |

~600 hours total connection time/month (36,000 connection-minutes), ~50,000 messages/month including broadcasts. **Total: ~$1.50/month.** Cheaper but more complex to implement — WebSocket API Gateway + Lambda is a different programming model than a plain TCP/UDP relay.

### What's Effectively Free at This Scale

| Thing | Why |
|-------|-----|
| S3 data transfer | Under 100GB/month free tier |
| CloudFront | 1TB/month free tier (if added) |
| Binary downloads | 10 users x 50MB = 500MB — free |
| SSL certificate | Free via ACM |
| Save file sync | A few hundred GET/PUT requests — fractions of a cent |

### Recommendation

At 0-10 users the difference between the cheapest and most expensive option is negligible. A cheap cloud VM at ~$3.50/month buys simplicity: deploy one Rust binary and forget about it. Optimize to serverless later only if you want to, not because you need to.

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

This is the Grounded model — the world lives on a player's machine, and when nobody's playing, nothing is running. No server costs. The world resumes when someone launches it.

### Where the Save File Lives

Three options, from simplest to most flexible:

**Option A: Local save (Grounded model)**
- One player is the save owner — the file lives on their machine
- They start a session by launching the relay and loading the save
- Other players join, receive state transfer, play
- When the session ends, any client can save locally (they all have identical state)
- **Cost: $0**
- **Limitation:** the save owner must be online to start a session

**Option B: Cloud storage (S3 bucket)**
- Save files stored in cheap object storage (AWS S3, Cloudflare R2, Backblaze B2)
- When anyone wants to play, they download the latest save and connect to the relay
- When the session ends, the save uploads back to storage
- Any player can start a session
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

### Log Compaction

The input log grows continuously during a session. Compaction materializes the log into a world state snapshot, advances the hot/cold boundary, and keeps the relay's memory bounded. For the decisions and rationale, see [architecture-decisions.md](architecture-decisions.md) — Log Compaction.

**Two tiers:**

| Tier | Location | Contents | Purpose |
|------|----------|----------|---------|
| **Hot** | Relay memory | Input entries from the latest snapshot tick to the current tick | Player join catch-up, jitter absorption |
| **Cold** | S3 | Full input history from tick 0 through the latest compaction point | Session replay, desync debugging |

**Three artifacts per session:**

| Artifact | S3 key pattern | Contents | Written |
|----------|---------------|----------|---------|
| **Save** (snapshot) | `sessions/{commit}/{session-id}/save` | Serialized authoritative world state. Tick number in S3 object metadata. | Overwritten at each compaction — only the latest matters for operations. |
| **Log** (cold tier) | `sessions/{commit}/{session-id}/log` | All input entries, checksums, and snapshot markers from session start. | Grows at each compaction — appended through session end. |
| **Snapshot marker** | (inside the log) | `{type: "snapshot", tick: T, s3_key: "..."}` — correlates a log position with a snapshot. | One entry per compaction point. |

The save and the log are separate S3 objects. The save is the materialized world state for operations (join, resume, crash recovery). The log is the sequential input history for replay and debugging. Snapshot markers in the log are the glue — they tell a replay reader "you can start from this snapshot instead of tick 0."

**Compaction procedure (at tick T):**

1. A client serializes its authoritative state at tick T
2. Uploads to S3 with tick T in object metadata (the existing sync protocol — HEAD, compare, conditional PUT)
3. The relay learns the new snapshot tick (notification from the uploading client, or periodic S3 HEAD)
4. The relay flushes input log entries before tick T to cold storage in S3
5. The relay writes a snapshot marker into the log at tick T
6. The hot buffer now starts at tick T

**Compaction triggers:**

| Trigger | When | Why |
|---------|------|-----|
| **Periodic** | Every 2–5 minutes of session time | Steady-state compaction — bounds hot buffer size and data-loss window |
| **Session end** | Last client disconnects gracefully | Captures the final state — the S3 save reflects the complete session |
| **Player join** | New player requests to join and the hot log has grown large since the last compaction (e.g., >30 seconds) | Reduces catch-up window for the joining player |
| **Size threshold** | Hot buffer exceeds a tick-count ceiling (e.g., 20,000 ticks) | Safety valve — guards against timer drift or failure |

**When compaction should NOT happen:**

- During the first few seconds of a session (let state settle after join handshake)
- If no ticks have advanced since the last compaction
- While a client is mid-catch-up (fast-forwarding to sync)

**Session lifecycle in S3:**

```
Session starts
  │
  ├─ Compaction at tick 7,200 (2 min)
  │    ├─ save written: world state at tick 7,200
  │    └─ cold log written: ticks 0–7,200
  │
  ├─ Compaction at tick 14,400 (4 min)
  │    ├─ save overwritten: world state at tick 14,400
  │    └─ cold log updated: ticks 0–14,400
  │
  ├─ ... more compaction points ...
  │
  └─ Session ends at tick 50,000
       ├─ save overwritten: world state at tick 50,000 (final)
       └─ cold log finalized: ticks 0–50,000 (complete session)
```

**Cold log retention:** Cold logs are cheap (20–35 MB/hour compressed). Retain the last 5–10 sessions via S3 lifecycle rules. At this data volume, keeping everything indefinitely is also viable.

### Cost Comparison

| Model | Monthly cost (idle) | Monthly cost (active) | Who can start a session? |
|-------|--------------------|-----------------------|--------------------------|
| Traditional server-authoritative | $4-50/month (always on) | Same | Anyone (server always running) |
| Local save (Option A) | $0 | $0 | Save owner only |
| Cloud storage (Option B) | ~$0.01 (S3 storage) | ~$0.01 + relay compute | Anyone |
| Client copies + cloud sync (Option C) | ~$0.01 (S3 storage) | ~$0.01 + relay compute | Anyone who has a copy |

The relay compute cost during active play is the same as in the "Connecting Over the Internet" section — a cheap VM at ~$3.50/month. Between sessions, the only cost is S3 storage.

## Persistent Storage Layout

A complete inventory of everything in persistent storage. Infrastructure artifacts exist independent of game sessions. Session artifacts are created during play.

### S3 — Application Infrastructure

| Key | Contents | Written by | Read by |
|-----|----------|-----------|---------|
| `version` | Git commit hash (plain text) | CI deploy job | Every client on startup |
| `seans-arcade-windows.exe` | Windows binary | CI deploy job | Clients during auto-update |
| `seans-arcade-macos` | macOS universal binary | CI deploy job | Clients during auto-update |
| `seans-arcade-linux` | Linux binary | CI deploy job | Clients during auto-update |
| Static website files | HTML/CSS for seanshubin.com | CI or manual | Browsers |

These are version-unaware. The version file points to the commit hash; the binaries correspond to that hash. Overwritten on every release.

### S3 — Per-Session Game Data

Organized by commit hash (version isolation) and session ID.

| Key pattern | Contents | Written by | When |
|-------------|----------|-----------|------|
| `sessions/{commit}/{session-id}/save` | Serialized authoritative world state at tick T. Tick number in S3 object metadata. | Any client (whoever is furthest ahead) | Each compaction point, session end |
| `sessions/{commit}/{session-id}/log` | Cold input log — all input entries, checksums, and snapshot markers from session start | Relay (or a client on relay's behalf) | Each compaction point (grows through session) |

The save and the log are separate objects, correlated by tick number. See [Log Compaction](#log-compaction) for the procedure and lifecycle.

### Relay Memory (Ephemeral)

| Data | Contents | Lifetime |
|------|----------|----------|
| Hot input buffer | Input entries from latest compaction tick to current tick | Lost on relay crash; rebuilt from restart point |
| Connection state | Connected clients, player slots, commit hashes | Lost on relay crash; clients reconnect and re-handshake |

The relay holds no persistent state. Everything in relay memory is either reconstructible (clients reconnect) or backed by S3.

### Local Client Disk

| Data | Contents | Purpose |
|------|----------|---------|
| Application binary | `seans-arcade.exe` (or platform equivalent) | The application |
| Old binary (Windows only) | `seans-arcade-old.exe` from rename dance | Cleanup on next launch |
| Local save copy | Authoritative state at current tick | Every client has this in memory during play; can persist to disk on exit |
| Client-side message log | Full log of sent/received messages | Local debugging, independent of relay's log |
| Config file | `%APPDATA%\seans-arcade\config.toml` (Windows) — display name and relay secret | Persists identity and access between launches ([decision](architecture-decisions.md)) |

### What Is NOT in Persistent Storage

- **Latency state** — disposable, rebuilt every tick, never persisted
- **Checksums** — transmitted live for drift detection, logged in the input log, not stored as separate artifacts
- **Game state at every tick** — derivable from the log, never stored independently

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
