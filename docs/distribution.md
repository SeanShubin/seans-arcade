# Distribution, Versioning & Auto-Update

How Sean's Arcade is built, versioned, distributed, and updated. For the commit hash decision and version-aware routing decision, see [architecture-decisions.md](architecture-decisions.md).

The Sean's Arcade application is a single compiled Rust binary (Windows `.exe` for v1). Anyone can download it from seanshubin.com. All clients must run the same version — this is critical for deterministic lockstep (identical code = identical simulation). The application handles its own updates automatically.

## Version Check

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

## Offline Mode

When the version check fails, the app launches and works locally but shows:
- An **offline indicator** (visible in the UI at all times while offline)
- A **countdown** to the next version check attempt

The app keeps retrying the version check periodically until it reaches the server. Once online:
- If the version matches → clear the offline indicator, connect normally
- If the version is stale → trigger auto-update

The retry interval strategy (fixed interval, exponential backoff, exact timing) is an open decision.

The relay also enforces version as a backstop (see below), so a stale client that comes back online can't silently connect with wrong code.

## Auto-Update

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

### Platform-Specific Replacement

**Windows:** Cannot delete or overwrite a running executable. Rename the running exe to `seans-arcade-old.exe`, rename the downloaded temp file to `seans-arcade.exe`, then restart. On next startup, detect and delete `seans-arcade-old.exe` as cleanup.

**macOS / Linux:** Unix allows overwriting or unlinking a running binary (the OS keeps the inode alive until the process exits). Download to a temp file, overwrite the original, then restart. No rename dance or cleanup step needed.

## Relay Version Enforcement (Backstop)

The relay on AWS also knows the current commit hash (from the same S3 version file it polls). When a client connects, it sends its commit hash in the Hello message. The relay compares it against the current commit hash and rejects connections that don't match, telling the client to update. This catches edge cases where the startup version check was skipped (offline launch, network blip during check, etc.). The client can then trigger the auto-update flow.

The commit hash in the Hello is also logged to the canonical event log on connection events, enabling deterministic replay — the log tells you exactly which code to check out and build. See [architecture-decisions.md](architecture-decisions.md) — Commit hash as the single code identifier.

## Release Workflow (Developer Side)

When publishing a new version:
1. Push to GitHub — CI builds per-platform binaries on native runners (cannot cross-compile to macOS)
2. CI uploads all platform binaries to S3
3. CI uploads the version file to S3 (`version` containing `$GITHUB_SHA`)
4. If the relay protocol changed, deploy the new relay binary

No manual version bumping — the commit hash is the version. CI writes `$GITHUB_SHA` to the version file automatically.

**Ordering matters:** All platform binaries must be uploaded (step 2) before the version file (step 3), so no client downloads a stale binary for a new commit hash. All platforms share the same commit hash — a release is not published until all platform binaries are ready.

**Why CI, not local builds:** macOS binaries must be built on macOS (the SDK and linker are not freely redistributable). GitHub Actions provides native macOS, Windows, and Linux runners. Bevy has an [official CI template](https://github.com/bevyengine/bevy_github_ci_template) for this.

See the CI Pipeline section below for GitHub Actions implementation details.

## CI Pipeline (GitHub Actions)

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

## Live Update Orchestration

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

## Cross-Platform Considerations (Bevy)

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

## What NOT to Build (v1)

- No differential/patch updates — full binary download every time. At 0-10 users and ~50MB binary, this is fine.
- No rollback mechanism — if a version is bad, publish a new one with a higher number.
- No code signing for v1 — required for macOS when that platform is added (Gatekeeper/notarization); add for Windows if SmartScreen becomes a problem.
