# TODO

Current work items, grouped by milestone. Check off when done.

## Local End-to-End Chat

Get all three binaries running locally on one machine. Two arcade clients log in as different users, chat through the relay, and arcade-cli can browse the message log. AWS dependencies (S3, version file, remote relay) are behind abstractions with local substitutes.

- [ ] Embed commit hash at build time — build script that sets `GIT_COMMIT_HASH` from `git rev-parse HEAD`
- [ ] Define relay protocol messages (Hello, Input, Disconnect) as shared types
- [ ] Implement relay binary — UDP listener, Hello handshake, version grouping, input broadcast
- [ ] Implement identity management — config.toml in platform app-data dir, first-launch name entry, auto-generated identity secret
- [ ] Implement arcade client — Bevy app, connect to relay, send/receive chat messages
- [ ] Implement chat UI — message display, input field, status bar (connection state, name, peer count)
- [ ] Abstract version check — trait/interface that resolves "current version"; local impl reads a file, production impl hits S3
- [ ] Abstract relay address — configurable endpoint (localhost for dev, AWS for production)
- [ ] Implement message logging on the relay — write forwarded inputs to local log files
- [ ] Implement arcade-cli log browsing — read and display relay log files
- [ ] Integration test — launch relay + two arcade clients locally, log in as two different users, exchange chat messages, verify logs via arcade-cli

## CI Build Pipeline

Push to `main` compiles all three binaries and embeds the commit hash. No AWS dependency — this is pure GitHub Actions.

- [ ] Create GitHub Actions workflow — Windows build on push to `main`, compile arcade, relay, and arcade-cli
- [ ] Implement startup version check — arcade client compares compiled-in hash against version source (local file for now)
- [ ] Implement auto-update — download new binary, Windows rename dance, restart
- [ ] Implement offline mode — launch with current version, show offline indicator, retry every 30 seconds

## AWS Deployment

Connect the CI pipeline to AWS. Binaries go to S3, relay goes to Lightsail, DNS points to seanshubin.com.

- [ ] Set up AWS infrastructure — S3 bucket for binaries and version file, Route 53 DNS for seanshubin.com
- [ ] Upload build artifacts to S3 — CI deploys binaries, then updates version file as atomic "go" signal (ordering matters)
- [ ] Wire version check and auto-update to S3 — production impl of the version/download abstractions
- [ ] Deploy relay to Lightsail — relay binary running on a cheap VM, accessible via public endpoint
