# TODO

Current work items, grouped by milestone. Check off when done.

## Local End-to-End Chat

Get all three binaries running locally on one machine. Two arcade clients log in as different users, chat through the relay, and arcade-ops can browse the message log. AWS dependencies (S3, version file, remote relay) are behind abstractions with local substitutes.

- [x] Embed commit hash at build time — build script that sets `GIT_COMMIT_HASH` from `git rev-parse HEAD`
- [x] Define relay protocol messages (Hello, Input, Disconnect) as shared types
- [x] Implement relay binary — UDP listener, Hello handshake, version grouping, input broadcast
- [x] Implement identity management — config.toml in platform app-data dir, first-launch name entry, auto-generated identity secret
- [x] Implement arcade client — Bevy app, connect to relay, send/receive chat messages
- [x] Implement chat UI — message display, input field, status bar (connection state, name, peer count)
- [x] Abstract version check — trait/interface that resolves "current version"; local impl reads a file, production impl hits S3
- [x] Abstract relay address — configurable endpoint (localhost for dev, AWS for production)
- [x] Implement message logging on the relay — write forwarded inputs to local log files
- [x] Implement arcade-ops log browsing — read and display relay log files
- [ ] Integration test — launch relay + two arcade clients locally, log in as two different users, exchange chat messages, verify logs via arcade-ops

## CI Build Pipeline

Push to `master` compiles all three binaries on all platforms and embeds the commit hash.

- [x] Create GitHub Actions workflow — builds arcade, relay, and arcade-ops on Windows, macOS, and Linux
- [x] Implement startup version check — arcade client compares compiled-in hash against `arcade.seanshubin.com/version`
- [x] Implement auto-update — download new binary, platform-specific replacement (Windows rename dance / Unix overwrite), restart
- [x] Implement offline mode — launch with current version, show offline indicator, retry every 30 seconds
- [x] Implement manifest-based asset download — CI generates `assets-manifest.json`, client syncs on startup

## AWS Deployment

Connect the CI pipeline to AWS. Binaries go to S3, relay goes to Lightsail, DNS points to seanshubin.com.

- [x] Set up AWS infrastructure — S3 bucket for binaries and version file, Route 53 DNS for arcade.seanshubin.com
- [x] Upload build artifacts to S3 — CI deploys binaries, version file, and assets manifest in a single sync
- [x] Wire version check and auto-update to S3 — client checks `arcade.seanshubin.com/version` on startup
- [x] Deploy relay to Lightsail — relay binary running on a cheap VM, accessible via relay.seanshubin.com

## Chat Persistence

Chat messages currently exist only in memory. Persist them to S3 so they survive restarts and new joiners can see history.

- [x] Chat history persistence — save chat state to S3, restore on relay startup
- [x] Chat history catch-up — client downloads history from S3 on join
- [x] Chat history bounding — 1000-message ring buffer per version group

## Logging Infrastructure

Relay logging currently captures chat text only. Expand to full protocol-level logging for debugging and deterministic replay.

- [ ] Full message logging — log all protocol messages (inputs, broadcasts, peer events) with tick numbers, timestamps, and metadata
- [ ] Log compaction and S3 archival — periodic snapshots, hot/cold log tiers, archive old logs to S3

## Code Signing & Self-Install

Code signing unblocks self-install and removes OS security warnings (SmartScreen, Gatekeeper).

- [ ] Code signing for Windows — sign binaries to avoid SmartScreen warnings
- [ ] Code signing for macOS — sign and notarize binaries for Gatekeeper
- [ ] Code signing for Linux — GPG signing for package managers (lower priority)
- [ ] Self-install on first run — binary copies itself to standard OS location, creates shortcuts, auto-updates in place

## Admin CLI (arcade-ops)

`arcade-ops` is the single operator interface for monitoring, management, analytics, and infrastructure control. Reads state from S3, writes commands to S3, shells out to AWS/SSH/Terraform. Replaces the originally planned static web dashboard.

### Relay S3 integration (done)
- [x] Relay writes heartbeat to S3 — `admin/heartbeat.json` with timestamp, uptime, client count
- [x] Relay writes connected users to S3 — `admin/connected.json` with names, commit hashes, idle time
- [x] Relay writes chat history to S3 — `admin/chat-history.json` from the write cache
- [x] Relay writes identity registry to S3 — `admin/identities.json`
- [x] Relay polls for command files — `admin/commands/`, executes and deletes

### Observe commands
- [ ] `status` — relay health from heartbeat (uptime, client count, commit hash, sync age). `--watch` for auto-refresh.
- [ ] `users` — connected users with idle times and client versions
- [ ] `identities` — registered identity names (no secrets)
- [ ] `history` — chat history, filterable by version or user
- [ ] `logs` — chat logs (local now, remote via S3 once relay uploads logs)

### Control commands
- [ ] `kick <user>` — disconnect user and remove identity registration
- [ ] `reset-identity <user>` — wipe stored secret so user re-registers
- [ ] `broadcast <message>` — send system message to all connected clients
- [ ] `drain` — gracefully disconnect all clients (pre-maintenance)

### Infrastructure commands
- [ ] `relay restart` — restart Docker container via SSH
- [ ] `relay redeploy` — pull latest image and restart via SSH
- [ ] `relay destroy` — terraform destroy relay resources (with confirmation)
- [ ] `relay ssh` — open interactive SSH session
- [ ] `infra plan` / `infra apply` / `infra destroy` — Terraform wrappers

### Data management commands
- [ ] `data versions` — list commit hashes with stored data, message counts, schema diff summary, last activity, storage size
- [ ] `data inspect <hash>` — show messages for a version (per-message decode, schema diff informational not gating)
- [ ] `data delete <hash>` — prefix delete of all stored data for a version (with confirmation)
- [ ] `data prune` — delete data for versions with no connected clients (with confirmation)

### Analytics commands
- [ ] `stats` — message volume, peak hours, active users per day
- [ ] `uptime` — relay uptime history
- [ ] `versions` — client version distribution, who's outdated
- [ ] `health` — composite check (relay responding, S3 syncing, cert valid, DNS resolving)

### Relay changes needed
- [ ] Per-version S3 layout: write to `admin/versions/<hash>/chat-history.json` instead of monolithic file
- [ ] Schema file: write `admin/versions/<hash>/schema.json` on startup from protocol type definitions
- [ ] New command types: `reset-identity`, `broadcast`, `drain`
- [ ] Richer heartbeat data: message counts, cumulative stats, start time
- [ ] Log upload to S3 for remote log browsing (`admin/versions/<hash>/logs/`)
- [ ] Update client chat history download URL to use per-version path
