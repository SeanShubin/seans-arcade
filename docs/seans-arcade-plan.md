# Sean's Arcade

A website at **seanshubin.com** where anyone can download the Sean's Arcade application. The application is a self-organizing peer network — the first person to launch it becomes the host, and everyone else connects through them. If the host leaves, the next person in line takes over. Global coordination is minimized; when necessary, it runs through AWS behind seanshubin.com.

## Starting Point: Chat

The first application is a drop-in/drop-out chat room. No accounts, no login, no signup. Download, launch, talk.

Chat is the right starting point because it exercises the full infrastructure — host election, peer discovery, host migration, message relay — without the complexity of deterministic simulation, tick synchronization, or latency hiding. Once chat works end-to-end, games are an incremental addition on top of the same foundation.

## Architecture

### What Runs on AWS (Global Coordination)

The only always-on infrastructure. Deliberately minimal.

| Service | Purpose | Implementation |
|---------|---------|----------------|
| **Static website** | Serves seanshubin.com — download links, info page | S3 + CloudFront (or just S3 static hosting) |
| **Presence registry** | Answers: "who is the current host?" | A single small file in S3, or a lightweight API |

The presence registry is the key coordination point. When a user launches the application:
1. Check the registry — is anyone hosting?
2. If no → become the host, register yourself in the registry
3. If yes → connect to the registered host as a peer

The registry stores minimal data: the host's public address (IP or relay endpoint) and a heartbeat timestamp. If the heartbeat is stale, the host is considered gone.

### What Runs on the User's Machine (Everything Else)

The downloaded application handles all of these roles:

| Role | When |
|------|------|
| **Host (relay)** | First user to launch, or next in succession |
| **Client** | Every user, including the host |
| **Successor candidate** | Every non-host client, ordered by join time |

The host runs a lightweight relay — the same stateless input coordinator described in the existing [network-architecture.md](network-architecture.md). For chat, the relay receives messages from all clients and broadcasts them to everyone. No chat history is stored on the relay (stateless). Each client keeps their own local scrollback.

### Host Succession

The host maintains an ordered list of connected peers (by join time). This list is shared with all peers so everyone agrees on the succession order.

**When the host leaves gracefully:**
1. Host announces departure to all peers
2. Next peer in the succession list becomes the new host
3. New host starts listening for connections
4. New host registers itself in the AWS presence registry
5. All other peers connect to the new host

**When the host disappears (crash, network loss):**
1. Peers detect the host is gone (connection timeout)
2. The deterministic succession rule selects the new host (first in the list)
3. New host starts listening, registers in the presence registry
4. Other peers attempt to connect to the new host
5. If the new host is also unreachable, try the next in line

**When everyone updates simultaneously (live update):**
The host updating is a graceful departure — it announces exit before restarting, triggering host succession. Since all clients are updating around the same time (different download speeds), host succession may cascade briefly as each new host also restarts. Within seconds, all clients are offline. As clients restart with the new version, they reconnect to the relay — the first to reconnect becomes the new host. This is equivalent to everyone's internet blipping for a few seconds; the system handles it via the same crash recovery path (all clients preserved full game state, resume from last confirmed tick).

This mirrors the relay host migration protocol from [network-operations.md](network-operations.md), adapted for the case where the relay runs on a player's machine rather than a cloud VM.

### NAT Traversal

The existing architecture assumes peers make outbound connections to a relay with a public IP. When the host is a home user behind a NAT router, incoming connections are blocked by default.

Options (in order of preference):

1. **Relay through AWS** — The host doesn't accept direct connections. Instead, all peers (including the host) make outbound connections to a lightweight relay endpoint on AWS. The host role is logical (succession, coordination) but the actual packet forwarding happens on the always-on relay. This keeps NAT out of the picture entirely but adds a small AWS cost during active sessions.

2. **STUN/TURN** — Use standard NAT traversal protocols. STUN handles the common case (peers behind simple NATs can often establish direct connections after a handshake). TURN is the fallback relay for restrictive NATs. AWS can host the TURN server.

3. **UPnP / port forwarding** — The host application automatically opens a port on the router (UPnP), or the user manually configures port forwarding. Simplest conceptually but least reliable — many routers disable UPnP, and manual configuration is a barrier.

**Recommended approach for v1:** Option 1 (relay through AWS). It's the simplest to implement, works for everyone regardless of network configuration, and the relay is trivially cheap for chat traffic. The relay can run as a small always-on process on the cheapest AWS instance, or as a serverless WebSocket endpoint. The host succession model still works — the "host" is the peer responsible for coordination (ordering messages, managing the peer list), even though packets flow through the AWS relay.

### Message Flow (Chat)

```
User types message
    → Client sends to relay (AWS or host)
    → Relay broadcasts to all connected peers
    → Each peer's client displays the message
```

Messages are plain text with a sender name and timestamp. No history is stored server-side. A joining peer sees messages from the moment they connect forward.

## Distribution

The Sean's Arcade application is a single compiled Rust binary (Windows `.exe` for v1). Anyone can download it from seanshubin.com. All clients must run the same version — this is critical for deterministic lockstep (identical code = identical simulation). The application handles its own updates automatically.

### Version Check

A single file on S3 serves as the source of truth:

```
https://seanshubin.com/version
```

Contains one integer, e.g., `7`. Nothing else.

The application has a compiled-in version constant:

```rust
const VERSION: u32 = 7;
```

On startup, before doing anything else:
1. HTTP GET `https://seanshubin.com/version`
2. Parse the integer
3. If it matches the compiled-in version → proceed to the app
4. If it doesn't → auto-update (see below)
5. If the request fails (no internet, S3 down) → launch with current version in **offline mode**

### Offline Mode

When the version check fails, the app launches and works locally but shows:
- An **offline indicator** (visible in the UI at all times while offline)
- A **countdown** to the next version check attempt

The app keeps retrying the version check periodically until it reaches the server. Once online:
- If the version matches → clear the offline indicator, connect normally
- If the version is stale → trigger auto-update

The retry interval strategy (fixed interval, exponential backoff, exact timing) is an open decision.

The relay also enforces version as a backstop (see below), so a stale client that comes back online can't silently connect with wrong code.

### Auto-Update

When a newer version is detected:

1. Download the platform-specific binary from S3 to a temp file in the same directory
2. Replace the currently running binary (platform-specific mechanism — see below)
3. Spawn the new binary as a new process
4. Exit the old process

**What the user sees:** "Updating to v8..." with a progress bar for the download (a few seconds on broadband for a ~50MB binary), then the app starts normally. From their perspective, the app just took a moment longer to launch.

The binary knows its own platform at compile time and fetches the correct artifact:

| Platform | Download URL | Binary name |
|----------|-------------|-------------|
| Windows | `https://seanshubin.com/seans-arcade-windows.exe` | `seans-arcade.exe` |
| macOS | `https://seanshubin.com/seans-arcade-macos` | `seans-arcade` |
| Linux | `https://seanshubin.com/seans-arcade-linux` | `seans-arcade` |

#### Platform-Specific Replacement

**Windows:** Cannot delete or overwrite a running executable. Rename the running exe to `seans-arcade-old.exe`, rename the downloaded temp file to `seans-arcade.exe`, then restart. On next startup, detect and delete `seans-arcade-old.exe` as cleanup.

**macOS / Linux:** Unix allows overwriting or unlinking a running binary (the OS keeps the inode alive until the process exits). Download to a temp file, overwrite the original, then restart. No rename dance or cleanup step needed.

### Relay Version Enforcement (Backstop)

The relay on AWS also knows the current version. When a client connects, it sends its version in the Hello message. If the relay sees a version mismatch, it rejects the connection with a message telling the client to update.

This catches edge cases where the startup version check was skipped (offline launch, network blip during check, etc.). The client can then trigger the auto-update flow.

### Release Workflow (Developer Side)

When publishing a new version:
1. Bump the `VERSION` constant in the client code
2. Push to GitHub — CI builds per-platform binaries on native runners (cannot cross-compile to macOS)
3. CI uploads all platform binaries to S3
4. CI uploads the version file to S3 (`version` containing the new integer)
5. If the relay protocol changed, deploy the new relay binary to Lightsail

**Ordering matters:** All platform binaries must be uploaded (step 3) before the version file (step 4), so no client downloads a stale binary for a new version number. All platforms share the same version number — a release is not published until all platform binaries are ready.

**Why CI, not local builds:** macOS binaries must be built on macOS (the SDK and linker are not freely redistributable). GitHub Actions provides native macOS, Windows, and Linux runners. Bevy has an [official CI template](https://github.com/bevyengine/bevy_github_ci_template) for this.

See the CI Pipeline section below for GitHub Actions implementation details.

### CI Pipeline (GitHub Actions)

The Release Workflow above describes the logical steps. This section describes the GitHub Actions implementation.

**Trigger:** push to `main`

**Build jobs** run in parallel (matrix strategy):
- **Windows** (`windows-latest`)
- **macOS** (`macos-latest`) — build both x86_64 and aarch64 targets, combine with `lipo` for universal binary
- **Linux** (`ubuntu-latest`)

**Deploy job** (depends on all three build jobs succeeding):
1. Download all platform artifacts
2. Upload all platform binaries to S3
3. Read the `VERSION` constant from the code, write that value to the S3 version file
4. If relay code changed: deploy new relay binary to Lightsail

The version number is bumped manually in the source code before pushing. CI reads it from the code so the S3 version file always matches. No auto-incrementing.

**Version file timing safety:** CI uploads platform binaries BEFORE updating the version file. The version file update is the atomic "go" signal — it only changes after all binaries are in place. This prevents clients from seeing a new version before the binary is available for download.

### Live Update Orchestration

The startup auto-update flow (above) handles clients that launch after a new version is published. This section covers what happens to clients that are **already running** when a new version is pushed.

**Relay polling:**
The relay polls `https://seanshubin.com/version` every ~30 seconds. When it sees a version newer than the one it expects, it sends an "update required" message to all connected clients.

**Client background download:**
1. Client receives "update required: version N+1" from the relay
2. Client begins downloading its platform-specific binary **in the background** — the game continues running
3. When the download completes: client finishes the current tick, replaces the binary, spawns the new process, exits
4. New process starts, version check passes (already have N+1), connects to relay

**Grace period:**
After notifying clients, the relay begins a **grace period** (~5 minutes) during which it accepts connections from both the old version (N) and the new version (N+1). This accommodates different download speeds — fast connections update in seconds, slow ones may take a minute or two. After the grace period expires, the relay hard-rejects any remaining version N connections.

**Relay protocol changes vs. client-only changes:**

| Scenario | What happens |
|----------|-------------|
| **Client code changed, relay protocol unchanged** | Relay stays running, discovers new version via polling, notifies clients. Clients update and reconnect. No relay downtime. |
| **Relay protocol changed** | CI deploys new relay binary to Lightsail. Relay restart disconnects all clients. Clients try to reconnect, get version mismatch rejection, auto-update via the startup flow, then reconnect with the correct version. |

### Cross-Platform Considerations (Bevy)

Design is cross-platform from the start even though v1 is Windows-only. Key Bevy/Rust constraints that affect all platforms:

**Determinism:** Bevy's `enhanced-determinism` feature flag must be enabled for all builds. This forces `libm` software math instead of hardware-specific intrinsics (x86 SSE vs ARM NEON produce different results for `sin`/`cos`/etc.). Without this, lockstep breaks between Windows and Mac players. All game state computation must stay on the CPU — GPU results are never deterministic across platforms.

**Build configuration:**
- `dynamic_linking` feature must be **disabled** for all release builds (breaks macOS and WASM)
- `#![windows_subsystem = "windows"]` suppresses the console window on Windows (conditionally, so debug builds still get console output)
- macOS builds should be **universal binaries** (x86_64 + aarch64) to support both Intel and Apple Silicon Macs

**Distribution format:** All platforms are distributed as **bare binaries**, not platform-specific bundles. macOS conventionally uses `.app` bundles, but a bare binary works fine and keeps the auto-update mechanism simple across all platforms. The `assets/` folder ships alongside the binary on all platforms.

**Platform-specific input quirks:**
- macOS scroll events produce non-integer values (OS-level acceleration) — do not assume discrete scroll steps
- macOS keyboard: Command = `SuperLeft`/`SuperRight`, Option = `AltLeft`/`AltRight`

**Linux:**
- Requires Vulkan drivers for full rendering (OpenGL ES 3 fallback is severely degraded)
- Wayland support requires opt-in via cargo feature flag; X11 is the default

### What NOT to Build (v1)

- No differential/patch updates — full binary download every time. At 0-10 users and ~50MB binary, this is fine.
- No rollback mechanism — if a version is bad, publish a new one with a higher number.
- No code signing for v1 — required for macOS when that platform is added (Gatekeeper/notarization); add for Windows if SmartScreen becomes a problem.

## What the Application Looks Like (v1 — Chat)

A simple window with:
- A text area showing chat messages (sender name + message)
- An input field at the bottom for typing
- A status bar showing: connection state, your name, number of connected peers, whether you're the host
- A name picker on first launch (stored locally for next time)

The entire application is built with Bevy, including v1 chat. Bevy's ECS and rendering pipeline are used from the start, so there's no framework migration when games are added later.

## Evolution Path

| Phase | What's added | What it exercises |
|-------|-------------|-------------------|
| **v1: Chat** | Drop-in text chat | Host election, peer discovery, host migration, message relay, AWS coordination |
| **v2: Chat + Pong** | Side-by-side chat and game lobby. Two peers can launch a pong match from within the app. | Deterministic lockstep, latency hiding, game state alongside chat |
| **v3: Game library** | Multiple game options in the lobby. Chat persists across games. | Game-agnostic relay, modular game loading |
| **v4: Persistence** | Game state saved to S3 between sessions | Cloud storage, tick-based sync protocol |

Each phase builds on the previous infrastructure. The chat channel persists through all phases — it becomes the social backbone of the arcade.

## Cost Estimate (0-10 Users)

Assumes 0-10 users online randomly throughout the month, averaging ~2 hours/day of activity, not all at once.

### Fixed Costs (Always Running)

| Service | What | Monthly Cost |
|---------|------|-------------|
| Route 53 hosted zone | DNS for seanshubin.com | $0.50 |
| S3 storage | Website + binaries (~100MB) | $0.003 |
| Domain renewal | seanshubin.com (already owned) | ~$1 amortized ($10-12/year) |

### Relay Options

**Option A: Lightsail always-on relay (recommended for v1)**

| Service | What | Monthly Cost |
|---------|------|-------------|
| Lightsail | 512MB instance running the relay 24/7 | $3.50 |
| Data transfer | Chat messages, 0-10 users | $0.00 (included) |
| S3 requests | Presence registry reads/writes | $0.00 (pennies) |

**Total: ~$5/month.** You're paying $3.50 for a machine that sits idle 95%+ of the time. But it's simple — one Rust binary, always ready, no cold starts.

**Option B: Serverless relay (API Gateway WebSocket + Lambda)**

| Service | What | Monthly Cost |
|---------|------|-------------|
| API Gateway WebSocket | Connection relay | $0.04 |
| Lambda | Message routing logic | $0.00 (free tier) |
| S3 requests | Presence registry | $0.00 |

~600 hours total connection time/month (36,000 connection-minutes), ~50,000 messages/month including broadcasts. **Total: ~$1.50/month.** Cheaper but more complex to implement — WebSocket API Gateway + Lambda is a different programming model than a plain TCP/UDP relay.

### What's Effectively Free at This Scale

| Thing | Why |
|-------|-----|
| S3 data transfer | Under 100GB/month free tier |
| CloudFront | 1TB/month free tier (if added) |
| Binary downloads | 10 users x 50MB = 500MB — free |
| SSL certificate | Free via ACM |
| Presence registry | A few hundred GET/PUT requests — fractions of a cent |

### Recommendation

At 0-10 users the difference between the cheapest and most expensive option is negligible. Lightsail at $3.50/month buys simplicity: deploy one Rust binary and forget about it. Optimize to serverless later only if you want to, not because you need to.

## Open Questions

1. **Identity** — v1 uses a self-chosen display name with no uniqueness enforcement. Is this sufficient long-term, or will some form of lightweight identity (e.g., a locally generated keypair) be needed?

2. **AWS relay cost model** — If the relay runs on AWS continuously, what's the monthly cost at various peer counts? Is it worth making the relay spin up on demand (e.g., Lambda + API Gateway WebSocket) vs. a persistent $3.50/month Lightsail instance?

3. **Chat history** — v1 has no history (join and see messages going forward). Should later versions offer opt-in history stored in S3?

4. **Version check retry interval** — Fixed interval, exponential backoff, or something else? What starting duration?
