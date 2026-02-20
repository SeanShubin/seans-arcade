# Sean's Arcade

A website at **seanshubin.com** where anyone can download the Sean's Arcade application. The application connects to a lightweight relay server on AWS that coordinates all communication. Clients never talk to each other directly — everyone makes outbound connections to the relay. Global coordination is minimal: AWS hosts the relay, a version file, and binary downloads.

## Starting Point: Chat

The first application is a drop-in/drop-out chat room. No accounts, no login, no signup. Download, launch, talk.

Chat is the right starting point because it exercises the full infrastructure — relay connection, peer discovery, message forwarding — without the complexity of deterministic simulation, tick synchronization, or latency hiding. Once chat works end-to-end, games are an incremental addition on top of the same foundation.

## Architecture

### What Runs on AWS

The goal is to get clients talking to each other. Clients never communicate directly — all traffic flows through a relay. The infrastructure below supports this.

| Service | Purpose | Implementation |
|---------|---------|----------------|
| **Static website** | Serves seanshubin.com — download links, info page | S3 + CloudFront (or just S3 static hosting) |
| **Relay server** | Forwards inputs between clients, assigns player slots, manages connections | Single Rust binary on Lightsail ($3.50/month) |
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
3. Hello handshake — send commit hash
4. Relay assigns a player slot
5. Begin sending/receiving inputs

### Connection Disruptions

**Player leaves:** The relay removes their slot and stops including their inputs in confirmed packages. Other clients see the player's inputs stop arriving. No coordination needed — the relay handles it.

**Relay restarts:** All clients lose their connections. Lightsail's process manager restarts the relay binary. Clients detect the disconnection, reconnect, and resume. Each client already has the full game state, so no state is lost. Clients agree on the last confirmed tick and continue from there.

**Live update:** All clients are updating around the same time. Each client disconnects, replaces the binary, and reconnects. The relay stays running throughout (it's on AWS, not on a player's machine). The grace period handles different update speeds — clients reconnect as they finish updating.

### Message Flow (Chat)

```
User types message
    → Client sends to relay (AWS)
    → Relay broadcasts to all connected clients
    → Each client displays the message
```

Messages are plain text with a sender name and timestamp. No history is stored on the relay (stateless). A joining client sees messages from the moment they connect forward.

## Distribution

The Sean's Arcade application is a single compiled Rust binary (Windows `.exe` for v1). Anyone can download it from seanshubin.com. All clients must run the same version — this is critical for deterministic lockstep (identical code = identical simulation). The application handles its own updates automatically.

### Version Check

A single file on S3 serves as the source of truth:

```
https://seanshubin.com/version
```

Contains a git commit hash, e.g., `abc123def456`. Nothing else.

The application has a compiled-in commit hash, embedded at build time by a build script that reads `git rev-parse HEAD`:

```rust
const COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
```

The commit hash is the single identifier for "this exact code." It changes automatically on every commit — no manual version bumping. Any code change — wire format, gameplay logic, constants, physics — produces a different commit hash. This matters because in deterministic lockstep, any code change produces different simulation results from the same inputs.

See [architecture-decisions.md](architecture-decisions.md) — Commit hash as the single code identifier.

On startup, before doing anything else:
1. HTTP GET `https://seanshubin.com/version`
2. Parse the commit hash
3. If it matches the compiled-in commit hash → proceed to the app
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

**What the user sees:** "Updating..." with a progress bar for the download (a few seconds on broadband for a ~50MB binary), then the app starts normally. From their perspective, the app just took a moment longer to launch.

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

The relay on AWS also knows the current commit hash (from the same S3 version file it polls). When a client connects, it sends its commit hash in the Hello message. The relay compares it against the current commit hash and rejects connections that don't match, telling the client to update. This catches edge cases where the startup version check was skipped (offline launch, network blip during check, etc.). The client can then trigger the auto-update flow.

The commit hash in the Hello is also logged to the canonical event log on connection events, enabling deterministic replay — the log tells you exactly which code to check out and build. See [architecture-decisions.md](architecture-decisions.md) — Commit hash as the single code identifier.

### Release Workflow (Developer Side)

When publishing a new version:
1. Push to GitHub — CI builds per-platform binaries on native runners (cannot cross-compile to macOS)
2. CI uploads all platform binaries to S3
3. CI uploads the version file to S3 (`version` containing `$GITHUB_SHA`)
4. If the relay protocol changed, deploy the new relay binary

No manual version bumping — the commit hash is the version. CI writes `$GITHUB_SHA` to the version file automatically.

**Ordering matters:** All platform binaries must be uploaded (step 2) before the version file (step 3), so no client downloads a stale binary for a new commit hash. All platforms share the same commit hash — a release is not published until all platform binaries are ready.

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
3. Write `$GITHUB_SHA` to the S3 version file
4. If relay code changed: deploy new relay binary

No manual version bumping. The commit hash is available automatically in CI.

**Version file timing safety:** CI uploads platform binaries BEFORE updating the version file. The version file update is the atomic "go" signal — it only changes after all binaries are in place. This prevents clients from seeing a new version before the binary is available for download.

### Live Update Orchestration

The startup auto-update flow (above) handles clients that launch after a new version is published. This section covers what happens to clients that are **already running** when a new version is pushed.

**Relay polling:**
The relay polls `https://seanshubin.com/version` every ~30 seconds. When it sees a commit hash different from the one it expects, it sends an "update required" message to all connected clients.

**Client background download:**
1. Client receives "update required" from the relay
2. Client begins downloading its platform-specific binary **in the background** — the game continues running
3. When the download completes: client finishes the current tick, replaces the binary, spawns the new process, exits
4. New process starts, version check passes (commit hash matches), connects to relay

**Grace period:**
After notifying clients, the relay begins a **grace period** (~5 minutes) during which it accepts connections from both the old and new commit hashes. This accommodates different download speeds — fast connections update in seconds, slow ones may take a minute or two. After the grace period expires, the relay hard-rejects any remaining old-commit connections.

**Version-aware routing during the grace period:**
The relay does not mix clients with different commit hashes into the same input broadcast. The relay knows each client's commit hash from the Hello handshake. During the grace period, a reconnecting client with the new commit hash either waits in a lobby or joins a separate input pool — it is never merged into the active pool of clients with the old commit hash. This prevents simulation corruption: even if the wire format hasn't changed, different code produces different simulation results from the same inputs. Checksums can detect the divergence but can't recover from a code mismatch. One comparison at connection time; zero per-message cost.

The relay logs each client's commit hash on the connection event in the canonical log, and records version-change points in a separate version index for random access. See [network-operations.md](network-operations.md) — Version Index.

**Envelope vs. payload — why most updates skip the relay:**
The relay understands the **protocol envelope** — message type, tick number, player slot, commit hash in Hello — but treats input **payloads as opaque bytes**. Game logic changes (new input types, new features, balance tweaks) only change the payload; the relay forwards them without knowing or caring. Only changes to the envelope itself (new message types, framing format, handshake changes) require a relay update. This is why "client code changed, relay protocol unchanged" is the common case in the table below.

**Relay protocol changes vs. client-only changes:**

| Scenario | What happens |
|----------|-------------|
| **Client code changed, relay protocol unchanged** | Relay stays running, discovers new commit hash via polling, notifies clients. Clients update and reconnect. No relay downtime. |
| **Relay protocol changed** | CI deploys new relay binary. Relay restart disconnects all clients. Clients try to reconnect, get commit hash mismatch rejection, auto-update via the startup flow, then reconnect with the correct version. |

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
| **v1: Chat** | Drop-in text chat | Relay connection, peer discovery, message forwarding, AWS coordination |
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

### Relay

The relay needs to be reachable by all clients. An always-on VM is the simplest way to achieve this, but not the only way — on-demand startup, serverless WebSocket, or other approaches could also work. The requirement is that clients can connect to the relay, not that the relay is always running.

**Option A: Lightsail VM (recommended for v1)**

| Service | What | Monthly Cost |
|---------|------|-------------|
| Lightsail | 512MB instance running the relay | $3.50 |
| Data transfer | Chat messages, 0-10 users | $0.00 (included) |
| S3 requests | Presence registry reads/writes | $0.00 (pennies) |

**Total: ~$5/month.** The machine sits idle most of the time, but at $3.50/month the simplicity of "always ready, no cold starts" is worth more than the savings from on-demand startup.

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
