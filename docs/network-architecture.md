# Network Multiplayer in Bevy

Architecture and concepts for the lockstep relay networking model. For the key decisions and their rationale, see [architecture-decisions.md](architecture-decisions.md). For operations (diagnostics, debugging, deployment, persistence, connection handling), see [network-operations.md](network-operations.md).

## Terminology

The networking pattern we prefer (see design philosophy #11) is **deterministic lockstep with a relay server**. The pieces:

| Term | Meaning |
|------|---------|
| **Deterministic lockstep** | All clients run identical simulation from shared inputs. Same inputs on tick N → same state on every machine. |
| **Relay server** (thin server) | Lightweight server that forwards inputs without simulating. No game state, no game logic — just a mailbox. |
| **Input authority** (sequencer) | The role of ordering inputs into a canonical per-tick sequence. The relay server fills this role. |
| **State-free server** | Server holds no game state. If it crashes, it restarts and clients reconnect — no game state is lost. |

Other terms you'll encounter in networking discussions:

| Term | Meaning |
|------|---------|
| **Server-authoritative** | Server runs the simulation and is the source of truth. Clients are thin. (Not our model.) |
| **Client-side prediction** | Client guesses the result of its own input before the server confirms. Used in server-authoritative to hide latency. |
| **Rollback** | When a prediction was wrong, rewind to the known-good state and re-simulate forward with correct inputs. |
| **Latency hiding** | Local prediction layer that makes actions feel instant despite round-trip delay. Factorio's term for their approach. |
| **Input buffering** | Sending inputs a few ticks early so the coordinator has them ready before they're needed. Absorbs jitter. |
| **Desync** | When two clients' game states diverge. Detected via checksums. Treated as a bug to fix. |

## Bevy Has No Built-In Networking

Networking is handled entirely by community crates, consistent with Bevy's modular plugin philosophy.

## Three Main Architectures

### Server-Authoritative (most common)
One Bevy app is the authoritative server. Clients send inputs, server simulates, server replicates state back. Can be a dedicated server or a listen server (one player also hosts).

### Peer-to-Peer Rollback
All peers run the full simulation and only exchange inputs. When a late input arrives that contradicts a prediction, the peer rolls back, inserts the correct input, and re-simulates forward. Used for fast-paced games (fighting, action).

### Lockstep
All peers exchange inputs each tick; simulation only advances when all inputs arrive. Common in RTS games. Artificial input delay is added to give slow peers time before stalling.

## Main Crates

| Crate | Architecture | Notes |
|-------|-------------|-------|
| **lightyear** | Server-authoritative | Most feature-complete. Built-in prediction, rollback, interpolation, lag compensation. UDP/WebTransport/Steam transports. |
| **bevy_replicon** | Server-authoritative | Simpler, modular. Automatic component replication via Bevy's change detection. Bring-your-own transport (renet, quinnet). |
| **bevy_ggrs** + **matchbox** | P2P rollback | GGRS is a Rust GGPO reimagination. Matchbox provides WebRTC transport (works in browsers). |

**Transport layers** (used standalone or under replicon): **renet** (UDP), **bevy_quinnet** (QUIC).

## ECS Integration

All crates provide Bevy plugins that register systems into schedules:

- Mark entities for replication with a marker component (e.g., `Replicated`)
- Register which components should sync (uses Bevy's change detection to track mutations)
- Inputs are buffered as resources and sent to the server through system parameters
- System ordering is managed via provided SystemSet values that slot into PreUpdate/FixedUpdate/Update
- Connected clients are represented as entities on the server

## General Flow (Server-Authoritative)

**Server each frame:** receive client inputs → simulate in FixedUpdate → detect component changes → send state updates to clients

**Client each frame:** capture local input → send to server → apply local prediction (immediate feel) → receive server state → reconcile (if prediction was wrong, roll back and re-simulate) → interpolate remote entities (other players shown slightly behind, smoothed between snapshots)

**P2P each frame:** exchange inputs with peers → simulate locally → if late input contradicts prediction, roll back and re-simulate forward

## How Lockstep Relay Works

A progressive walkthrough of the networking concepts specific to our chosen strategy (deterministic lockstep with a relay server). Later sections go deeper on individual topics; this section shows how they fit together.

### The Fundamental Problem

Two players on different machines need to see the same game. Data takes time to travel between them. Everything else follows from this.

### Ticks: A Shared Clock That Isn't a Clock

The simulation doesn't advance in real-world time — it advances in **ticks**. Tick 150 means "the game state after 150 simulation steps." Every client agrees on what tick 150 means in terms of game state, but they don't process tick 150 at the same wall-clock moment.

This is the foundation. Without a shared logical clock, you can't even talk about whether two machines agree. See [Logical Clocks and Frame Agreement](#logical-clocks-and-frame-agreement) for how different architectures handle tick synchronization.

### The Relay Server: Ordering, Not Simulating

The relay server solves one specific problem: **input ordering**. If player A presses "up" and player B presses "left" during tick 50, every client needs to process those inputs in the same order. The relay collects inputs from all clients, stamps them with a tick number, and broadcasts the canonical package.

The relay has no game state. It doesn't know what "up" means. It's a mailbox that ensures everyone reads the same letters in the same order. If it crashes, it restarts and clients reconnect — no game state is lost because every client holds the full simulation. See [Who Coordinates Inputs in P2P?](#who-coordinates-inputs-in-p2p) for coordination topology options.

### Determinism: Same Inputs, Same State

Since every client runs the full simulation, they must all produce **identical results** from identical inputs. Not "approximately the same" — bit-for-bit identical.

Where this breaks: floating-point transcendentals (`sin`, `cos`) can return slightly different results on different hardware. A difference of 0.000001 on tick 50 compounds into a completely different world by tick 500. See [Why Not Just Share Inputs?](#why-not-just-share-inputs-the-determinism-problem) for what breaks determinism and [Making P2P Determinism Practical](#making-p2p-determinism-practical) for the mitigation strategy.

### The Latency Problem

The round trip from client to relay and back takes time — say 50ms. Without any mitigation:

1. You press "move right" on tick 100
2. Your input travels to the relay (~25ms)
3. The relay broadcasts it to everyone (~25ms)
4. Everyone (including you) applies it on tick 100
5. **You see your own action 50ms after you pressed the button**

At 60 ticks/second, that's 3 ticks of delay before your own input takes effect. This feels sluggish.

### Latency Hiding: Prediction Without Rollback

Our strategy uses Factorio's approach, which is distinct from rollback. The difference matters.

**Rollback** (used in P2P fighting games via GGRS) means the authoritative simulation advances using predicted inputs, then **rewinds and re-simulates** when the real input arrives and contradicts the prediction. The authoritative state is temporarily wrong and must be corrected.

**Latency hiding** (our model) never lets the authoritative state be wrong. Instead, each client maintains two separate states:

| State | What it is | What it's for |
|-------|-----------|---------------|
| **Authoritative state** | The simulation using only confirmed inputs (received back from the relay) | The "real" game state — identical on every client |
| **Latency state** | Authoritative state + unconfirmed local inputs replayed on top | What the player sees on screen — feels responsive |

Each tick:
1. Advance the authoritative state using the latest confirmed input package from the relay
2. Copy the authoritative state
3. Replay all unconfirmed local inputs on top of the copy
4. Render the latency state

When the relay confirms an input, it enters the authoritative state and is removed from the unconfirmed list. The latency state and authoritative state converge naturally. Nothing is rewound. The latency state is disposable — rebuilt fresh every tick.

### Input Buffering: Absorbing Jitter

Network latency isn't constant. One packet takes 20ms, the next takes 45ms. If the simulation stalls whenever an input packet is late, the game stutters.

**Input buffering** means sending inputs a few ticks early. If the round-trip time is ~50ms (3 ticks), inputs are sent 4-5 ticks ahead. The relay collects them and has them ready before they're needed. The extra tick or two absorbs jitter — a delayed packet still arrives in time.

The tradeoff: more buffering = smoother play but slightly more input delay. Less buffering = more responsive but more risk of stalls.

### Stalls vs. Dropping: What Happens When Someone Lags

In pure lockstep, if one player's input for tick 200 hasn't arrived, **nobody simulates tick 200**. One slow connection freezes everyone. This is why pure P2P lockstep caps at ~24 players.

The relay model (following Factorio's evolution) solves this: if a player's input is missing, the relay **omits it** rather than stalling. The slow player's character just doesn't act that tick. Everyone else keeps going. The slow player catches up by processing multiple ticks in one frame when their connection recovers.

### Checksums: Trust But Verify

Even with all the determinism precautions, bugs happen. Every 30-60 ticks, each client hashes its authoritative game state and sends the hash through the relay. 8 bytes — negligible bandwidth. If hashes match, everything is fine. If they diverge, it's a determinism bug to find and fix. See [State Checksums: Detecting Drift](#state-checksums-detecting-drift) for implementation details.

### One Tick, End to End

Putting it all together — what one client does each tick:

```
1. Receive confirmed input package from relay for tick N
2. Advance authoritative state: tick N-1 → tick N
3. Capture local input for tick N + buffer_size
4. Send local input to relay
5. Rebuild latency state:
   a. Copy authoritative state
   b. Replay all unconfirmed local inputs on top
6. Render the latency state
7. If checksum tick: hash authoritative state, send to relay
```

Every client does this identically (except step 3, which captures that player's local inputs). The authoritative states converge. The latency states differ per player (each sees their own unconfirmed inputs predicted). The relay just keeps the mail flowing.

### Concepts That Don't Apply to This Strategy

These are relevant to other networking architectures but not to lockstep relay:

- **Client-side prediction (server-authoritative sense)** — where the server can override the client's predicted state. In our model, the authoritative state is never wrong, so there's nothing to override.
- **Rollback / re-simulation of authoritative state** — the authoritative state only advances with confirmed inputs. The latency state is rebuilt fresh, not rolled back.
- **State replication** — sending entity data (positions, health, etc.) over the wire. Only inputs travel over the network; each client computes entity state locally.
- **Interpolation of remote entities** — smoothing other players' positions between state snapshots. Every client runs the full simulation, so remote entities are already at their correct positions.

## Logical Clocks and Frame Agreement

Clients don't need to be on the same frame at the same time. Each architecture handles this differently.

### Server-Authoritative
- Server owns the canonical tick counter (e.g., tick 100, 101, 102...)
- At connection, client and server negotiate a time offset using RTT (round-trip time) measurement
- Client runs slightly ahead of the server — far enough that inputs arrive "just in time"
- Server stamps every state update with its tick number so the client knows where it sits
- If a client lags: server can repeat last known input, buffer inputs to absorb jitter, or the client catches up by processing multiple ticks in one frame

### P2P Rollback (GGRS)
- All peers agree on tick rate at session start
- Each peer advances its own simulation and sends inputs stamped with tick numbers
- If remote inputs for tick N haven't arrived yet, predict (repeat last known input) and simulate anyway
- When real input arrives late, roll back to tick N, replace prediction, re-simulate forward
- Input delay is dynamically adjusted to give slow peers more time before predictions are needed

### Lockstep
- No peer simulates tick N until all inputs for tick N are received
- If one peer is slow, everyone waits
- Artificial input delay (4-6 frames) gives all peers a window

### Key Concepts

| Concept | Purpose |
|---------|---------|
| **Tick numbering** | Shared frame identity — everyone agrees tick 150 means the same game state |
| **RTT measurement** | Clients know how far ahead to run so inputs arrive on time |
| **Input buffering** | Absorbs jitter — send inputs a few ticks early, server buffers them |
| **Input delay** | Buys time for slow connections |
| **Prediction + rollback** | Don't wait — guess and correct later |
| **Catch-up** | Lagging client processes multiple ticks in one frame to get back on track |

The fundamental insight: **tick number is a logical clock, not a wall clock.** All peers agree on what tick 150 means in terms of game state, but they don't need to process tick 150 at the same real-world moment.

## Why Not Just Share Inputs? (The Determinism Problem)

The idea of "server as input coordinator, clients simulate independently" is the P2P model. It works when you can guarantee determinism — same inputs on tick N produce bit-identical results on every machine.

### What breaks determinism

- **Floating point:** `a + b + c` can give different results than `a + c + b` depending on CPU, compiler, and instruction ordering. An f32 that differs by the last bit compounds every tick.
- **Iteration order:** HashMap iteration is not deterministic across runs.
- **System parallelism:** Bevy's scheduler runs non-conflicting systems in parallel. Interleaving can differ per machine.
- **Platform differences:** WASM, x86, ARM may all produce subtly different results.

If tick 200 diverges by 0.001 on one client, by tick 500 the worlds can be completely different.

### Architecture Comparison

| Factor | P2P (input sharing) | Server-authoritative |
|--------|---------------------|---------------------|
| Bandwidth | Low (inputs only) | Higher (state updates) |
| Determinism required | Yes, perfectly | No |
| Cheat resistance | Low (each client can lie) | High (server validates) |
| Complexity | Determinism is hard | Prediction/reconciliation is hard |
| Best for | Fighting games, small player count, same platform | Shooters, RPGs, mixed platforms, competitive |

### For Bevy Specifically

Bevy's parallel scheduler and f32-based transforms make perfect determinism difficult out of the box. You'd need to:
- Force single-threaded system execution
- Use a fixed-point math library instead of f32
- Avoid HashMap and use deterministic collections
- Carefully audit every system for ordering dependencies

This is why most Bevy multiplayer games use server-authoritative (lightyear or replicon) rather than pure P2P deterministic simulation.

## Making P2P Determinism Practical

The "trust everyone" P2P model (share inputs, each client simulates independently) has appealing properties for non-adversarial games: minimal bandwidth, no dedicated server needed. The challenge is ensuring all clients produce identical results.

### What's Already Deterministic in Rust

The problem is narrower than it first appears. Basic IEEE 754 operations are fully specified:

- `+`, `-`, `*`, `/` on f32/f64 — deterministic with round-to-nearest-even (Rust's default)
- `sqrt` — IEEE 754 mandates correct rounding
- All integer math — always deterministic
- Comparisons and int↔float conversions — well-specified
- Rust does NOT enable fast-math by default (unlike C/C++), so it won't reorder float operations

### What's NOT Deterministic

- **Transcendental functions** (`sin`, `cos`, `exp`, `log`, `pow`) — IEEE 754 does not require correctly rounded results. Different libm implementations return results that differ in the last bit. These differences compound every tick.
- **Fused multiply-add (FMA)** — `a * b + c` as one operation gives a different result than two separate operations. Compilers can silently insert FMA instructions on hardware that supports it.
- **x87 vs SSE** — x87 FPU uses 80-bit internal precision, SSE uses 64-bit. Same code, different results. (Only a concern on 32-bit x86; x86_64 defaults to SSE.)
- **HashMap iteration order** — not deterministic across runs or platforms.
- **Bevy's parallel scheduler** — non-conflicting systems can interleave differently per machine.

### The JVM Precedent

Java had `strictfp` to force IEEE 754 semantics (default since Java 17). But even strict IEEE 754 doesn't cover transcendental functions. Java's `StrictMath` uses a specific software implementation (fdlibm) to guarantee identical results everywhere — slower, but deterministic. So Java solved it with a deterministic math library, not at the compiler level.

### Could Compilers Solve This?

In principle, yes. Each machine is individually deterministic — the divergence comes from the toolchain making different choices per target. You'd need to lock down:

1. **Disable FMA fusion** — Rust flag: `-C llvm-args=-fp-contract=off`
2. **Force SSE, not x87** — already the default on x86_64
3. **Disable fast-math** — already the default in Rust
4. **Use deterministic transcendentals** — software implementation, same on all platforms

No mainstream compiler currently offers a "fully deterministic floating point" mode that handles all of these. It's solvable in principle but not yet solved as a single switch.

### Practical Options (Easiest to Hardest)

**Option 1: Constrained f32 with software transcendentals** (recommended for Bevy)
- Keep f32 for basic math — already deterministic for +,-,*,/,sqrt
- Use the `libm` crate for sin/cos/etc. instead of hardware implementations
- Compile with `-C llvm-args=-fp-contract=off` to prevent FMA fusion
- Use deterministic collections (BTreeMap, sorted iteration)
- Disable Bevy's parallel system execution for gameplay systems
- Lowest friction — most code stays normal, you patch the known problem spots

**Option 2: Fixed-point math** (sidestep floats entirely)
- Use integers with implicit scale: 1000 = 1.0, 1500 = 1.5
- All math is integer — perfectly deterministic everywhere
- Crate: `fixed` (Rust fixed-point arithmetic)
- Downside: awkward API, limited range, no Bevy integration (Transform uses f32)

**Option 3: WASM as a determinism layer**
- WASM specifies IEEE 754 precisely for basic ops + sqrt
- Compile game simulation to WASM, run in a WASM runtime on each client
- All clients produce identical results regardless of host hardware
- Downside: performance overhead, complexity of splitting simulation from rendering

**Option 4: Periodic state checksums** (safety net)
- Don't guarantee determinism — detect when it breaks
- Each peer periodically hashes its game state and compares with others
- If they diverge, one peer resyncs from another (full state transfer)
- Can combine with Option 1 as a safety net to catch anything you missed

### Recommended Approach for Bevy P2P

Option 1 (constrained f32) + Option 4 (checksums as safety net). The engineering burden is real but bounded — you're not rewriting your math library, you're constraining a few specific things. Checksums catch anything you missed without requiring perfect confidence in your determinism guarantees.

## State Checksums: Detecting Drift

Each client computes a hash of its game state at a given tick. Peers compare hashes. If they match, simulations are in sync. If they diverge, someone drifted.

### What Gets Hashed

Hash the parts of game state that affect the simulation:

- **Component values on gameplay entities** — positions, health, velocities, inventory
- **Resource values** — score, wave number, timers
- **Entity count** — ensures nobody has extra or missing entities

Skip presentation-only state:

- Particle positions, animation frames, camera shake
- Local UI state (menus, tooltips)
- Audio state

The rule: if it affects the simulation, hash it. If it only affects rendering, skip it.

### How It Works

1. At tick N, each client serializes gameplay state into bytes and runs a fast hash
2. Clients exchange checksums periodically — every 30-60 ticks (half a second to a second)
3. A 64-bit hash is 8 bytes per exchange per peer — essentially free bandwidth

**On mismatch:**
1. **Detection** — "My checksum for tick 300 differs from yours"
2. **Diagnosis** — Hash subsections separately (positions vs health vs inventory) to narrow it down
3. **Recovery** — Diverged client receives a full state snapshot from the host and overwrites its local state

### Hash Algorithm Choice

Use **non-cryptographic hashes**, not cryptographic ones. The distinction matters:

- **Cryptographic hashes** (SHA-256) resist *intentional* collisions — someone crafting two inputs with the same hash. Slow because that security property is expensive.
- **Non-cryptographic hashes** (xxhash, CRC32, FNV) detect *accidental* differences. Orders of magnitude faster.

Since players are trusted (see design philosophy #11), nobody is forging matching hashes to hide tampered state. We're only detecting accidental drift. Using a cryptographic hash here would be paying a CPU cost for distrust — exactly the kind of cost the design philosophy rejects.

**Practical choices for Rust:**
- **xxhash** — extremely fast, excellent distribution, 64-bit output. Best default. Crate: `xxhash-rust`
- **CRC32** — simpler, hardware-accelerated on most CPUs, 32-bit output. Fine for small state.
- **FNV-1a** — simple, fast, good for small inputs. Rust's `FnvHasher`.

With 64-bit xxhash, probability of an accidental collision is 1 in 2^64 (~18 quintillion). False "all clear" from a non-cryptographic hash will never happen in practice.

### Cost

- **Computation:** Hashing game state takes microseconds, even with thousands of entities. Negligible next to simulation cost.
- **Bandwidth:** 8 bytes per half-second per peer. Free.
- **Recovery:** Full state transfer on desync is expensive — same cost as a new player joining. But should be rare (ideally never once determinism bugs are fixed).

### Checksums as a Development Tool

Factorio checksums every single tick during development. Every desync is treated as a bug to investigate and fix. Over time, desyncs become extremely rare. The checksums shift from active debugging tool to quiet confirmation that everything is fine.

For Bevy games, the same approach:
1. Define which components and resources are authoritative state
2. Serialize and hash every 30-60 ticks
3. Exchange hashes via the coordinator
4. On mismatch — log what diverged, resync from host
5. Treat every desync as a determinism bug to fix

## Who Coordinates Inputs in P2P?

If we trust everybody and just share inputs, someone still needs to ensure every peer ends up with the same complete input history in the same order.

### Option A: Dedicated Relay Server

A lightweight server that does no simulation — it receives inputs from all peers and broadcasts them to everyone else.

- Peers send inputs to the relay
- Relay stamps them with the tick number and forwards to all other peers
- Relay is the authority on tick ordering
- Much cheaper than a simulation server — just a mailbox

### Option B: Full Mesh P2P

Every peer sends inputs directly to every other peer. No server at all.

- 2 players: A sends to B, B sends to A. Simple.
- 4 players: each sends to 3 others. Manageable.
- 20 players: each sends to 19 others. Bandwidth scales as N*(N-1). Gets expensive.
- This is what **matchbox** does — WebRTC connections between all peers. Works well for 2-8 players.
- One peer is designated "host" for clock purposes without being a simulation authority.

### Option C: Host-as-Relay

One player's machine acts as the relay. Star topology — all traffic flows through one node.

- No separate server infrastructure needed
- If the host disconnects, the game dies (unless you implement host migration)
- The host has a latency advantage — sees their own inputs immediately
- Common in console "peer-to-peer" multiplayer

### Input History Storage

For rollback to work, each peer keeps a **local buffer** of recent inputs (its own and everyone else's):

- Buffer the last 7-10 ticks of inputs (enough for the rollback window)
- When a late input arrives, roll back to the local state snapshot at that tick
- Old inputs beyond the rollback window can be discarded — already baked into simulation state
- No peer or server needs the full history from tick 0

### The Matchbox Approach (What bevy_ggrs Uses)

1. **Signaling server** — a tiny server that helps peers discover each other and establish WebRTC connections. Handles "player A wants to connect to player B" handshaking. Once connections are established, the signaling server is no longer needed.
2. **Direct WebRTC connections** — after signaling, peers talk directly. Inputs flow peer-to-peer with no intermediary.

The signaling server is like a matchmaking lobby — it introduces players but doesn't participate in the game.

### Coordination Summary

| Approach | Server needed? | Scales to | Complexity |
|----------|---------------|-----------|------------|
| Full mesh P2P | Only signaling for discovery | 2-8 players | Low |
| Host-as-relay | No server, one player relays | 2-16 players | Medium (host migration) |
| **Dedicated relay** | **Lightweight, no simulation** | **Many players** | **Low (but requires infrastructure)** |

**Our choice: Dedicated relay on AWS (Option A).** A single Rust binary on a cheap cloud VM (e.g., AWS Lightsail at ~$3.50/month). All clients make outbound connections to the relay — no client ever accepts incoming connections, no player hosts the relay, and NAT traversal is a non-issue. See [network-operations.md](network-operations.md) — How Clients Connect.

## Case Studies: Factorio and Minecraft

Two games that represent opposite ends of the networking spectrum, both achieving massive player counts through fundamentally different approaches.

### Factorio: Deterministic Lockstep (The "Trust Everyone" Model at Scale)

Factorio does exactly what the P2P model describes — share inputs, every client simulates independently. Demonstrated with **500+ players**.

**Why lockstep:** A late-game factory has **millions of entities** (belts, inserters, bots). Sending state updates for all of them would be impossible. Network traffic is proportional to **player actions** (~500 kbps per player), not entity count.

**Determinism:** Bit-for-bit identical across Windows, Mac, Linux, different CPUs. Custom RNG seeded from shared state, careful floating-point handling. Every tick, each client computes a **checksum** of game state. If checksums diverge, the desynced client redownloads the map.

**Evolution from pure P2P to server-coordinated:**

Factorio started with pure P2P lockstep where one slow peer froze everyone. They rewrote to a **server-client hybrid** where the server coordinates but does not simulate:

1. Clients send inputs only to the server
2. Server merges inputs into a single ordered package per tick
3. Server broadcasts that package to all clients
4. All clients simulate identically using the same ordered inputs
5. If one client lags, the server **omits that player's inputs** rather than stalling everyone

This change took them from ~24 players to 400+.

**Latency hiding:** Since actions must round-trip to the server before they're "real," Factorio maintains a **latency state** prediction layer:

- Every tick, the latency state is rebuilt from the real game state
- All unconfirmed local inputs are replayed on top
- The latency state is used for rendering and creating new actions
- When the server confirms, prediction and reality converge — self-correcting
- Character movement is predicted locally; world interactions await server confirmation

**Anti-cheat as side effect:** A player sending invalid inputs causes only their own client to desync. They cannot corrupt others' state because every client independently simulates the same canonical input sequence.

### Minecraft: Server-Authoritative (Opposite Approach)

Minecraft uses traditional server-authoritative. The server simulates everything, clients are mostly renderers. Even singleplayer runs an internal server.

**Why server-authoritative:** The world is procedurally generated, infinitely mutable, and has physics-like interactions (gravity, water flow, mob AI pathfinding). Making all of that deterministic across Java's JVM on different platforms would be extremely difficult. The mod ecosystem also requires servers to run custom logic that clients don't understand.

**Protocol details:**
- Java Edition uses TCP with a custom binary protocol
- Bedrock Edition uses RakNet (UDP-based), giving snappier feel on high-latency connections
- Entity positions use delta compression (fixed-point relative offsets)
- Only chunks within a player's view distance are tracked and sent

**Scaling limitations:** A single server handles 20-100 players before tick rate (target: 20 TPS) degrades. The community solves this with:

- **Optimized server forks** (Paper, Purpur) — 2-3x more players than vanilla
- **Proxy networks** (BungeeCord, Velocity) — route players across thousands of backend servers
- **Folia** — experimental region-based multithreading (different world regions tick on different threads)

Large networks like Hypixel run thousands of separate servers. Each handles 20-100 players. This works because Minecraft has natural spatial locality — players in different areas don't interact.

### Comparison

| Dimension | Factorio (Lockstep) | Minecraft (Server-Auth) |
|-----------|-------------------|----------------------|
| Bandwidth | Scales with **players**, not entities | Scales with **entities near players** |
| Server CPU | Server does NO simulation | Server does ALL simulation |
| Join time | Must download entire world state | Only downloads nearby chunks |
| Determinism burden | Extreme — bit-identical everywhere | None — server is truth |
| Player scaling | 500+ in one simulation | 20-100 per server, shard for more |
| Cheat resistance | Excellent (invalid inputs = self-desync) | Moderate (needs anti-cheat plugins) |
| Mod compatibility | Mods must maintain determinism | Server-side mods work freely |

### The Architecture Follows the Game Design

**Factorio works as lockstep because:**
- Massive shared state + low input rate (millions of entities, a few player clicks per second)
- Simulation is closed-form — given same inputs, same outputs. No unseeded randomness.
- All players share one interconnected factory. Sharding is not an option.
- Cooperative, not competitive PvP — round-trip latency is acceptable.

**Minecraft needs server-authoritative because:**
- Open, mutable, procedurally generated world with unpredictable state changes
- Natural spatial locality — chunk tracking is efficient, players don't need the whole world
- Mod ecosystem requires server logic that clients don't understand
- Joining mid-game is trivial — download nearby chunks, not the entire world

### Lessons for P2P Bevy Games

Factorio validates the "trust everyone, coordinate inputs" model. Key takeaways:

1. **Pure P2P lockstep doesn't scale** — one slow peer freezes everyone
2. **Server-as-input-coordinator solves it** — server orders and distributes inputs but doesn't simulate
3. **Determinism is achievable** but requires discipline — checksums catch mistakes
4. **Latency hiding** makes round-trip delay acceptable for non-twitch gameplay
5. The right architecture depends on what dominates your bandwidth: if entity count >> input rate, lockstep wins

## Why the Relay Is Cheap

The relay (on AWS, not on a player's machine) does trivially little work compared to what each client does:

### What the Relay Does
- Receive a few hundred bytes per player per tick
- Merge into one ordered package
- Broadcast to all clients
- No game logic, no simulation, no state — just packet forwarding

### What Every Client Does
- Run the full game simulation
- Render the game
- Process local input

The relay's work is negligible — packet forwarding at ~19 KB/sec for 4 players. The cheapest VM handles this with resources to spare. See [network-operations.md](network-operations.md) — Cost Estimate.

All clients have equal latency — everyone's inputs travel the same outbound path to the AWS relay and back. No player has a latency advantage.

For connection disruptions (player leave, relay restart, player join mid-game), deployment, persistent state, diagnostics, and debugging, see [network-operations.md](network-operations.md).

## Quick Recommendation

- Full-featured with prediction/interpolation built in → **lightyear**
- Simple replication, compose your own features → **bevy_replicon**
- P2P rollback for action games → **bevy_ggrs** + **matchbox**
- Turn-based multiplayer → custom over any transport; complexity is low

## Sources

- [Bevy Discussion #4388: What networking library to use](https://github.com/bevyengine/bevy/discussions/4388)
- [Bevy Discussion #8675: What kind of networking should X game use?](https://github.com/bevyengine/bevy/discussions/8675)
- [lightyear GitHub](https://github.com/cBournhonesque/lightyear) / [book](https://cbournhonesque.github.io/lightyear/book/)
- [bevy_replicon GitHub](https://github.com/simgine/bevy_replicon)
- [bevy_ggrs GitHub](https://github.com/gschup/bevy_ggrs)
- [matchbox GitHub](https://github.com/johanhelsing/matchbox)
- [renet GitHub](https://github.com/lucaspoffo/renet)
- [bevy_quinnet GitHub](https://github.com/Henauxg/bevy_quinnet)

### Factorio Networking (Factorio Friday Facts)
- [FFF-76: MP Inside Out](https://www.factorio.com/blog/post/fff-76) — Lockstep architecture fundamentals
- [FFF-83: Hide the Latency](https://www.factorio.com/blog/post/fff-83) — Latency state mechanism
- [FFF-99: MP Forwarding](https://www.factorio.com/blog/post/fff-99) — Server forwarding model
- [FFF-147: Multiplayer Rewrite](https://www.factorio.com/blog/post/fff-147) — Server-client rewrite details
- [FFF-149: Deep Down in Multiplayer](https://www.factorio.com/blog/post/fff-149) — Input actions, map upload
- [FFF-156: Massive Multiplayer](https://www.factorio.com/blog/post/fff-156) — 400+ player scaling
- [FFF-188: Bug, Bug, Desync](https://factorio.com/blog/post/fff-188) — Desync debugging
- [FFF-302: The Multiplayer Megapacket](https://www.factorio.com/blog/post/fff-302) — Bandwidth optimization
- [FFF-412: Car Latency Driving](https://factorio.com/blog/post/fff-412) — Latency hiding for vehicles

### Minecraft Networking
- [wiki.vg: Minecraft Protocol](https://wiki.vg/Protocol) — Complete protocol documentation
- [Bedrock Wiki: RakNet Protocol](https://wiki.bedrock.dev/servers/raknet)

### General Networking Architecture
- [Game Networking Demystified Part I: State vs Input](https://ruoyusun.com/2019/03/28/game-networking-1.html)
- [Gabriel Gambetta: Client-Server Game Architecture](https://www.gabrielgambetta.com/client-server-game-architecture.html)
