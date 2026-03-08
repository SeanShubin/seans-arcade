# Distribution, Versioning & Auto-Update

How Sean's Arcade is built, versioned, distributed, and updated. For the commit hash decision and version-aware routing decision, see [architecture-decisions.md](../architecture-decisions.md).

This document covers the **game client** (`arcade`) — how it is built, versioned, distributed, and updated. The project produces three binaries (see [architecture-decisions.md](../architecture-decisions.md) — Project Structure); this document focuses on the player-facing one.

The game client is a single compiled Rust binary (Windows `.exe` for v1), downloaded from seanshubin.com. All clients *in a session* must run the same version — this is critical for deterministic lockstep (identical code = identical simulation). The application handles its own updates automatically.

## Version Check

> **Not yet implemented.** The version file and auto-update flow described in this section are the decided design but are not yet built. Currently, players download the binary manually from the website.

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

See [architecture-decisions.md](../architecture-decisions.md) — Commit hash as the single code identifier.

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

The retry interval is a **fixed 30 seconds** — the same interval used for all connectivity retries in the application ([decision](../architecture-decisions.md)).

The relay also isolates clients by version (see below), so a stale client that comes back online either joins its version's group or finds no peers.

## Auto-Update

When a newer version is detected:

1. Download the platform-specific binary from S3 to a temp file in the same directory
2. Replace the currently running binary (platform-specific mechanism — see below)
3. Spawn the new binary as a new process
4. Exit the old process

**What the user sees:** "Updating..." with a progress bar for the download (a few seconds on broadband for a ~50MB binary), then the app starts normally. From their perspective, the app just took a moment longer to launch.

The binary knows its own platform at compile time and fetches the correct artifact:

| Platform | Download URL                                  | Binary name  |
| -------- | --------------------------------------------- | ------------ |
| Windows  | `https://arcade.seanshubin.com/arcade.exe`    | `arcade.exe` |
| macOS    | `https://arcade.seanshubin.com/arcade`        | `arcade`     |

### Platform-Specific Replacement

**Windows:** Cannot delete or overwrite a running executable. Rename the running exe to `arcade-old.exe`, rename the downloaded temp file to `arcade.exe`, then restart. On next startup, detect and delete `arcade-old.exe` as cleanup.

**macOS / Linux:** Unix allows overwriting or unlinking a running binary (the OS keeps the inode alive until the process exits). Download to a temp file, overwrite the original, then restart. No rename dance or cleanup step needed.

## Relay Version Isolation

The relay reads the commit hash from each client's Hello handshake and groups clients by version. Clients with different commit hashes cannot interact — each version group operates independently. There is no rejection; a client running an old version simply won't see clients running a newer version (and vice versa). This naturally handles any number of concurrent versions.

The commit hash in the Hello is also logged to the canonical event log on connection events, enabling deterministic replay — the log tells you exactly which code to check out and build. See [architecture-decisions.md](../architecture-decisions.md) — Commit hash as the single code identifier.

## Release Workflow (Developer Side)

When publishing a new version:
1. Push to GitHub — CI builds per-platform binaries on native runners (cannot cross-compile to macOS)
2. CI uploads all platform binaries to S3
3. CI deploys the relay binary to the Lightsail VM via SSH
4. *(Future)* CI uploads the version file to S3 (`version` containing `$GITHUB_SHA`)

No manual version bumping — the commit hash is the version.

**Note:** Step 4 (version file) is not yet implemented. When it is, ordering will matter: all platform binaries must be uploaded (step 2) before the version file (step 4), so no client downloads a stale binary for a new commit hash.

**Why CI, not local builds:** macOS binaries must be built on macOS (the SDK and linker are not freely redistributable). GitHub Actions provides native macOS, Windows, and Linux runners. Bevy has an [official CI template](https://github.com/bevyengine/bevy_github_ci_template) for this.

See the CI Pipeline section below for GitHub Actions implementation details.

## CI Pipeline (GitHub Actions)

The Release Workflow above describes the logical steps. This section describes the GitHub Actions implementation.

**Trigger:** push to `master`

**Build jobs** run in parallel (separate jobs, not a matrix):
- **`build-windows`** (`windows-latest`) — builds `arcade`, `relay`, and `arcade-cli`
- **`build-macos`** (`macos-latest`) — builds `arcade`, `relay`, and `arcade-cli`
- **`build-linux-relay`** (`ubuntu-latest`) — builds `relay` only (for deployment to the Linux VM)

**Deploy job** (depends on all three build jobs succeeding):
1. Download all platform artifacts and stage them into a flat `deploy/` directory
2. Generate `index.html` with build info (commit hash, timestamp)
3. Upload to S3 (`aws s3 sync`), excluding the Linux relay binary
4. Invalidate CloudFront cache
5. Deploy relay binary to the Lightsail VM via SCP + SSH (Docker rebuild and restart)

No manual version bumping. The commit hash is available automatically in CI.

**Note:** The version file mechanism described in the Version Check section above is the decided design but is not yet implemented in CI. Currently, there is no `version` file on S3 — the auto-update flow is a future feature. The design is retained here as the intended approach.

## Version Isolation

Running clients continue on their current version until the user relaunches. There are no mid-session updates, no background downloads, and no grace period. The startup auto-update flow (above) ensures that every new launch gets the latest version.

The relay groups clients by commit hash. Clients with different commit hashes cannot interact — each version group operates independently. Multiple versions can coexist simultaneously. For a small invite-only group, coordinating a relaunch is trivial; the complexity of live update machinery is not justified.

Logs are split by version — every entry in a given log shares the same commit hash. The commit hash is recorded once per log, and replaying requires checking out the corresponding commit.

**Envelope vs. payload — why most updates skip the relay:**
The relay understands the **protocol envelope** — message type, tick number, player slot, commit hash in Hello — but treats input **payloads as opaque bytes**. Game logic changes (new input types, new features, balance tweaks) only change the payload; the relay forwards them without knowing or caring. Only changes to the envelope itself (new message types, framing format, handshake changes) require a relay update. This is why "client code changed, relay protocol unchanged" is the common case in the table below.

**Relay protocol changes vs. client-only changes:**

| Scenario                                          | What happens                                                                                                                                                          |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Client code changed, relay protocol unchanged** | Relay stays running. New clients arrive with a new commit hash and form a new version group. Old-version clients continue undisturbed. No relay downtime.             |
| **Relay protocol changed**                        | CI deploys new relay binary. Relay restart disconnects all clients. Clients auto-update on next launch via the startup flow, then reconnect with the correct version. |

## Cross-Platform Considerations (Bevy)

Design is cross-platform from the start even though v1 is Windows-only. Key Bevy/Rust constraints that affect all platforms:

**Determinism:** Bevy's `enhanced-determinism` feature flag must be enabled for all builds. This forces `libm` software math instead of hardware-specific intrinsics (x86 SSE vs ARM NEON produce different results for `sin`/`cos`/etc.). Without this, lockstep breaks between Windows and Mac players. All game state computation must stay on the CPU — GPU results are never deterministic across platforms.

**Build configuration:**
- `dynamic_linking` feature must be **disabled** for all release builds (breaks macOS and WASM)
- `#![windows_subsystem = "windows"]` suppresses the console window on Windows (conditionally, so debug builds still get console output)
- macOS builds should be **universal binaries** (x86_64 + aarch64) to support both Intel and Apple Silicon Macs

**Distribution format:** All platforms are distributed as **bare binaries**, not platform-specific bundles. macOS conventionally uses `.app` bundles, but a bare binary works fine and keeps the auto-update mechanism simple across all platforms. Assets are embedded in the binary via `include_bytes!` — no external files needed at runtime.

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
