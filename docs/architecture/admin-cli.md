# Admin CLI

The `arcade-cli` binary is the single operator interface for monitoring, managing, and debugging the arcade. It reads state from S3, writes commands to S3, and shells out to AWS/SSH/Terraform for infrastructure operations. There is no separate web dashboard — the CLI covers everything.

## Design Principles

- **One tool** — no separate dashboard and CLI to maintain
- **All-seeing** — the operator can inspect every piece of system state except user identity secrets
- **All data through S3** — the relay writes state, the CLI reads it; the CLI writes commands, the relay executes them
- **Infrastructure control** — restart, redeploy, and destroy from the same tool
- **Analytics built in** — interpret data, don't just display it

## Architecture

```
arcade-cli  ──reads──>  S3 (admin/*)           <──writes──  relay
arcade-cli  ──writes──> S3 (admin/commands/*)   ──polls──>  relay
arcade-cli  ──ssh────>  Lightsail               (restart, logs, shell)
arcade-cli  ──terraform──> AWS                  (infra lifecycle)
```

The relay remains unchanged — it writes JSON state files to S3 and polls `admin/commands/` for instructions. The CLI is a pure consumer/producer of those files plus direct access to AWS for infrastructure operations.

## S3 Layout

All admin data lives under the `admin/` prefix in the existing `arcade.seanshubin.com` bucket:

```
s3://arcade.seanshubin.com/
  admin/heartbeat.json           # relay health — timestamp, uptime, client count
  admin/connected.json           # who's online — names, commit hashes, idle times
  admin/chat-history.json        # persisted chat messages (base64-encoded payloads)
  admin/identities.json          # registered identity names (no secrets)
  admin/logs/                    # chat logs uploaded by the relay (future)
  admin/commands/                # command files written by the CLI, consumed by relay
```

## Command Groups

### Observe (read from S3)

| Command | Description | S3 Source |
|---------|-------------|-----------|
| `status` | Relay health: uptime, client count, commit hash, last sync age. `--watch` for auto-refresh. | `admin/heartbeat.json` |
| `users` | Connected users with idle times, client versions. | `admin/connected.json` |
| `identities` | All registered identity names. No secrets shown. | `admin/identities.json` |
| `history` | Chat history, filterable by version or user. | `admin/chat-history.json` |
| `logs` | Chat logs. Currently local-only; remote via S3 once relay uploads logs. | `admin/logs/` (future) or local filesystem |

### Control (write commands to S3)

| Command | Description | Effect |
|---------|-------------|--------|
| `kick <user>` | Disconnect a user and remove their identity registration. | Extends existing `delete-user` command. |
| `reset-identity <user>` | Wipe a user's stored secret so they re-register on next connect. | New relay command type. |
| `broadcast <message>` | Send a system message to all connected clients. | New relay command type. |
| `drain` | Gracefully disconnect all clients (pre-maintenance). | New relay command type. |

Commands are written as JSON files to `admin/commands/`. The relay polls, executes, and deletes them. The relay remains the single owner of mutable state.

### Infrastructure (AWS/SSH/Terraform)

| Command | Description | Mechanism |
|---------|-------------|-----------|
| `relay restart` | Restart the relay Docker container. | SSH to Lightsail. |
| `relay redeploy` | Pull latest image and restart. | SSH to Lightsail. |
| `relay destroy` | Tear down relay infrastructure (with confirmation). | `terraform destroy` (relay resources). |
| `relay ssh` | Open an interactive SSH session. | SSH to Lightsail. |
| `infra plan` | Preview infrastructure changes. | `terraform plan`. |
| `infra apply` | Apply infrastructure changes (with confirmation). | `terraform apply`. |
| `infra destroy` | Destroy all infrastructure (with confirmation). | `terraform destroy`. |

### Analytics (read + interpret)

| Command | Description | Data Source |
|---------|-------------|-------------|
| `stats` | Message volume over time, peak hours, active users per day. | `admin/chat-history.json`, `admin/logs/` |
| `uptime` | Relay uptime history — when it was up, when it went down, total availability. | `admin/heartbeat.json` (track over time) |
| `versions` | Which client versions are connected, who's outdated. | `admin/connected.json` |
| `health` | Composite check: relay responding? S3 syncing? cert valid? DNS resolving? | Multiple sources. |

Analytics commands interpret raw data rather than just displaying it. `stats` shows trends, not raw numbers. `health` gives a pass/fail verdict with specifics on failures.

## What the Relay Needs

The relay already writes `heartbeat.json`, `connected.json`, `chat-history.json`, and `identities.json` to S3 and supports `delete-user` commands. New relay work:

- **New command types:** `reset-identity`, `broadcast`, `drain`
- **Richer heartbeat data:** message counts since last sync, cumulative message count, start time (for uptime history)
- **Log upload to S3** (future): periodic upload of chat log files so `logs` works remotely

## What Stays Out

- **User identity secrets** — the CLI sees registered names but never secrets (the relay already omits secrets from `admin/identities.json`)
- **Direct UDP to the relay** — all admin flows through S3 or SSH, keeping the relay's network surface minimal
- **Web dashboard** — the CLI is the only admin interface; no static site to build, host, or secure

## Authentication

The CLI runs on the operator's machine with:
- **AWS credentials** for S3 reads/writes (same credentials used for deployment)
- **SSH key** for Lightsail access (same key used by CI)
- **Terraform state** for infrastructure operations (local state file)

No additional auth mechanism needed — access to the operator's machine implies access to these credentials.

## Supersedes

This design replaces the static web dashboard described in the earlier [admin-dashboard.md](admin-dashboard.md) decision. The S3 data flow is identical — only the consumer changed from a browser to a CLI. The relay is unaffected.
