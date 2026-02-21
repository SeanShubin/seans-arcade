# Session Architecture: Multiplexed Sessions vs. Unified World State

This is an **open v3 decision**. v1 and v2 operate as a single lockstep session — one relay, one tick stream, one group of clients. v3 introduces multiple simultaneous games with different player groups. The question: expand the relay to manage multiple sessions, or keep the relay unchanged and expand the simulation?

## Approach A: Multiplexed Sessions

Each game is a separate lockstep session with its own tick stream, tick counter, and player group. Chat is one session; each game instance is another. The relay routes inputs to the correct session.

### What changes from v2

- Relay gains **session management** (create, join, leave, list)
- Protocol messages gain a **session ID field** (envelope change — requires relay update)
- Relay maintains **per-session state**: tick counters, input buffers, player slots
- Spectating requires **joining a session as observer**

### Pros

- Clients only simulate games they're participating in — CPU scales with games joined, not total arcade activity
- Sessions are independent — a determinism bug in one game doesn't affect others (fault isolation)
- Tick rates can differ per game type (60 Hz for Pong, 1 Hz for Chess)
- Natural "join/leave" UX maps directly to protocol primitives

### Cons

- Relay is no longer "just a mailbox" — it must understand session lifecycle
- Multiplexing protocol adds complexity (session IDs in every message, routing logic)
- Session creation and discovery is new infrastructure (lobby protocol, session listing)
- Relay protocol changes from v1 — the v1/v2 relay binary cannot be reused unchanged

## Approach B: Unified World State

The entire arcade is one simulation with one tick stream. Chat, Pong inputs, Chess moves — all are input payload variants within the same stream. Starting a game means spawning entities; ending a game means despawning them.

### What changes from v2

- Relay: **nothing**. Still one session, one tick stream, opaque inputs. The v1 relay binary works unchanged.
- Client simulation grows: ECS world contains all arcade state (chat + all active games)
- New input types are **client-only payload changes** — no relay or protocol modifications

### Pros

- Relay identical to v1/v2 — no session management, no multiplexing protocol
- Spectating is trivially free — every client already has the full world state
- Consistent with the "chat messages as game inputs" decision (everything is one input stream)
- Consistent with the "relay treats inputs as opaque bytes" decision (game changes are client-only)
- Adding new game types requires no relay changes, no protocol changes
- All existing infrastructure (logging, replay, persistence) works unmodified

### Cons

- Every client simulates every game — CPU scales with total arcade activity, not games joined
- Tick rate must accommodate the most demanding game (can't run Chess at 1 Hz and Pong at 60 Hz)
- World state grows with concurrent games — more memory, larger snapshots
- Determinism bug in any game affects everything (no fault isolation)

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

## Scale Considerations

- At **0–10 users**, CPU waste from simulating all games is negligible. The candidate games (Pong, Breakout, Space Invaders, Asteroids) are trivially cheap to simulate.
- Candidate games all run at **similar tick rates** — tick rate disparity is not a current problem.
- **Turn-based games** (Chess) would make tick rate disparity more acute. A unified 60 Hz tick rate wastes cycles for a game that only needs input processing on moves.
- Unified is **simpler now**; multiplexed **scales better later**.

## Relationship to Existing Decisions

- **"Chat messages as game inputs"** — aligns with Approach B. Everything is one input stream; the relay doesn't distinguish chat from Pong from Chess.
- **"Relay treats inputs as opaque bytes"** — aligns with Approach B. Game changes are client-only payload changes; the relay never needs to know what games exist.
- **Persistence layout** (`sessions/{commit}/{session-id}/`) — works for either approach. Under unified, there's one session ID for the whole arcade. Under multiplexed, each game instance has its own.

## Open Questions

- **Hybrid approach?** Start unified for v3, migrate to multiplexed if scale demands it later. Is this a clean migration path or a trap?
- **If unified: entity namespacing?** How are game instances separated within one ECS world? Options: game-scoped components, game ID field on entities, separate Bevy worlds within one app.
- **If multiplexed: session discovery/join protocol?** How do clients discover available sessions, create new ones, and join?
- **How does persistence interact?** One save per session (multiplexed) vs. one save for the whole arcade (unified). What are the implications for save size, load time, and partial restores?
