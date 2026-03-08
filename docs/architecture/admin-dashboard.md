# Admin Dashboard

An operator dashboard for monitoring and managing the arcade. Served as a static website from S3, with all data flowing through S3 — no direct connection to the relay.

## Design Principles

- The relay stays simple — it writes data to S3, it does not serve admin endpoints
- The dashboard is a static site — no backend, no WebSocket, no server-side rendering
- All data flows through S3 — the relay writes, the dashboard reads
- Near-real-time is sufficient — data can be 5-15 seconds stale
- Admin commands flow through S3 — the dashboard writes command files, the relay polls and executes them

## Architecture

```
Relay ──writes──> S3 (admin/*) ──reads──> Dashboard (static site)
Dashboard ──writes──> S3 (admin/commands/*) ──polls──> Relay
```

The relay periodically (every 5-15 seconds) writes JSON files to S3 under the `admin/` prefix. The dashboard fetches these files and renders them. For write operations (e.g. deleting a user), the dashboard writes a command file to S3, and the relay polls for and executes command files.

## S3 Layout

All admin data lives under the `admin/` prefix in the existing `arcade.seanshubin.com` bucket:

```
s3://arcade.seanshubin.com/
  ...existing files (binaries, version, assets)...
  admin/heartbeat.json       # relay health — timestamp, uptime
  admin/connected.json       # who's online — names, commit hashes, latency
  admin/chat-history.json    # recent messages from the in-memory buffer
  admin/identities.json      # registered users from the identity registry
  admin/commands/             # command files written by the dashboard
```

## Relay Health Check

The relay writes `admin/heartbeat.json` on every loop iteration (same cadence as the existing 5-second read timeout). Contents:

```json
{
  "timestamp": "2026-03-08T12:00:00Z",
  "uptime_secs": 3600,
  "client_count": 3,
  "commit_hash": "abc123..."
}
```

The dashboard checks the timestamp. If it's older than 30 seconds, the relay is considered down. This keeps the "everything through S3" model — no HTTP health endpoint needed on the relay.

## Admin Commands

The relay is the single owner of mutable state (identity registry, connected clients). The dashboard never modifies state directly. Instead:

1. Dashboard writes a command file to `admin/commands/` (e.g. `admin/commands/delete-user-alice.json`)
2. Relay polls `admin/commands/` periodically
3. Relay executes the command, modifies its own state
4. Relay deletes the command file after execution
5. Dashboard sees the updated state on the next sync cycle

This avoids sync issues between the dashboard and the relay's in-memory state.

## Authentication

The dashboard uses an admin secret — entered once, stored in browser localStorage, sent with each S3 request (or used to derive access). Details TBD — the simplest approach may be a separate CloudFront behavior for `admin/*` with a signed cookie or Lambda@Edge auth check.

## Features

| Feature | S3 file | Write direction |
|---------|---------|-----------------|
| Relay status (up/down) | `admin/heartbeat.json` | Relay → S3 |
| Connected users | `admin/connected.json` | Relay → S3 |
| Latency/lag monitoring | `admin/connected.json` | Relay → S3 |
| Chat history browsing | `admin/chat-history.json` | Relay → S3 |
| Registered users | `admin/identities.json` | Relay → S3 |
| Delete user | `admin/commands/delete-user-*.json` | Dashboard → S3 → Relay |

## What This Replaces

The `arcade-cli` binary was originally designed as a local operator tool for browsing relay log files. The admin dashboard replaces its monitoring and management functions with a web interface accessible from anywhere. The `arcade-cli` binary remains in the project for any future local-only tooling needs.
