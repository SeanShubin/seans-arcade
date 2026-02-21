# Session Architecture

**Decision: Unified world state.** The entire arcade is one simulation with one tick stream. No multiplexed sessions.

## Context

v1 and v2 operate as a single lockstep session — one relay, one tick stream, one group of clients. v3 introduces multiple simultaneous games with different player groups. The question was: expand the relay to manage multiple sessions, or keep the relay unchanged and expand the simulation?

## Decision

Unified world state. All games share one tick stream. Starting a game means spawning entities; ending a game means despawning them. Chat, Pong inputs, Chess moves — all are input payload variants within the same stream.

What this means for the relay: **nothing changes**. The v1 relay binary works for v3. Still one session, one tick stream, opaque inputs. New game types are client-only payload changes — no relay modifications, no protocol changes.

### Why unified

- The relay stays minimal — no session management, no routing, no multiplexing protocol
- Consistent with "chat messages as game inputs" — everything is one input stream; the relay doesn't distinguish chat from Pong from Chess
- Consistent with "relay treats inputs as opaque bytes" — game changes are client-only payload changes; the relay never needs to know what games exist
- Spectating is trivially free — every client already has the full world state
- Adding new game types requires no relay changes, no protocol changes
- All existing infrastructure (logging, replay, persistence) works unmodified

### Why not multiplexed sessions

Multiplexed sessions would mean each game is a separate lockstep session with its own tick stream, tick counter, and player group. The relay routes inputs to the correct session. This was considered and rejected because it contradicts core architectural decisions:

- **Breaks "relay treats inputs as opaque bytes"** — the relay must parse an envelope to extract a session ID before routing. Every protocol change gains a relay-side component.
- **Breaks "chat messages are game inputs"** — chat becomes a special session. Does chat have its own tick stream? Its own tick rate? Chat messages get routed differently than game inputs.
- **Breaks "the relay stays minimal"** — the relay becomes stateful in a new dimension: session lifecycle, membership, per-session tick counters. The attack surface of the internet-facing component grows.
- **Increases relay redeployment frequency** — game logic changes are currently relay-transparent. With multiplexing, game types with different session semantics (team games, different player caps) could require relay changes. The relay becomes an iteration bottleneck.
- **Multiplies infrastructure** — logging, replay, persistence, desync-check all currently assume one stream. With multiplexing, every tool needs session awareness.
- **Solves problems that don't exist** — per-game tick rates, fault isolation, and client CPU savings matter at scale. At 0–10 users playing Pong and Breakout, they're irrelevant.

### Accepted tradeoffs

- Every client simulates every game — CPU scales with total arcade activity, not games joined. At the target scale (0–10 users, simple arcade games) this is negligible.
- Tick rate must accommodate the most demanding game. Candidate games (Pong, Breakout, Space Invaders, Asteroids) all run at similar rates, so this isn't a current problem. Turn-based games (Chess) would waste cycles at 60 Hz but still function correctly.
- World state grows with concurrent games — more memory, larger snapshots. Again, negligible at target scale.
- A determinism bug in any game affects the entire arcade (no fault isolation). This is the real cost, mitigated by the existing checksum and replay infrastructure.

## Comparison

| Dimension | Multiplexed Sessions | Unified World State |
|-----------|---------------------|---------------------|
| Relay complexity | Session management, routing | Unchanged from v1 |
| Protocol changes | Session ID envelope, join/leave messages | None |
| Client CPU | Scales with games joined | Scales with total arcade activity |
| Tick rate flexibility | Per-session | Single rate for all games |
| Spectating | Join session as observer | Free (already simulating) |
| Fault isolation | Per-session | None (shared simulation) |
| Adding new game types | Relay-transparent (payload only) | Relay-transparent (payload only) |
| Persistence | Per-session saves | One save for whole arcade |
| Determinism blast radius | Contained to session | Entire arcade |
| Infrastructure reuse from v1/v2 | Relay requires changes | Relay unchanged |

## Remaining Questions

- **Entity namespacing:** How are game instances separated within one ECS world? Options: game-scoped components, game ID field on entities, separate Bevy worlds within one app.
- **Persistence granularity:** One save for the whole arcade — what are the implications for save size, load time, and partial restores?
