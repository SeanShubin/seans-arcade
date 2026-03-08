# TODO

Current work items, grouped by milestone. Check off when done.

## Local End-to-End Chat

Get all three binaries running locally on one machine. Two arcade clients log in as different users, chat through the relay, and arcade-cli can browse the message log. AWS dependencies (S3, version file, remote relay) are behind abstractions with local substitutes.

- [x] Embed commit hash at build time — build script that sets `GIT_COMMIT_HASH` from `git rev-parse HEAD`
- [x] Define relay protocol messages (Hello, Input, Disconnect) as shared types
- [x] Implement relay binary — UDP listener, Hello handshake, version grouping, input broadcast
- [x] Implement identity management — config.toml in platform app-data dir, first-launch name entry, auto-generated identity secret
- [x] Implement arcade client — Bevy app, connect to relay, send/receive chat messages
- [x] Implement chat UI — message display, input field, status bar (connection state, name, peer count)
- [x] Abstract version check — trait/interface that resolves "current version"; local impl reads a file, production impl hits S3
- [x] Abstract relay address — configurable endpoint (localhost for dev, AWS for production)
- [x] Implement message logging on the relay — write forwarded inputs to local log files
- [x] Implement arcade-cli log browsing — read and display relay log files
- [ ] Integration test — launch relay + two arcade clients locally, log in as two different users, exchange chat messages, verify logs via arcade-cli

## CI Build Pipeline

Push to `master` compiles all three binaries on all platforms and embeds the commit hash.

- [x] Create GitHub Actions workflow — builds arcade, relay, and arcade-cli on Windows, macOS, and Linux
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

- [ ] Chat history persistence — save chat state to S3, restore on relay startup
- [ ] Chat history catch-up — send previous messages to newly joining peers
- [ ] Chat history bounding — decide and implement a limit (last N messages, session-only, or unlimited)

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

## Admin Dashboard

Static web dashboard for monitoring and managing the arcade. Replaces arcade-cli for remote operations. All data flows through S3.

- [ ] Relay writes heartbeat to S3 — `admin/heartbeat.json` with timestamp, uptime, client count
- [ ] Relay writes connected users to S3 — `admin/connected.json` with names, commit hashes, latency
- [ ] Relay writes chat history to S3 — `admin/chat-history.json` from the in-memory buffer
- [ ] Relay writes identity registry to S3 — `admin/identities.json`
- [ ] Relay polls for command files — `admin/commands/`, executes and deletes
- [ ] Dashboard static site — HTML/JS served from S3, reads `admin/*` files, renders status
- [ ] Dashboard authentication — admin secret via CloudFront signed cookie or Lambda@Edge
- [ ] Dashboard user management — delete user via command file
