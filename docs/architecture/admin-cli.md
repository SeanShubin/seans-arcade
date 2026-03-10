# Admin CLI

The `arcade-ops` binary is the single operator interface for monitoring, managing, and debugging the arcade. It reads state from S3, writes commands to S3, and shells out to AWS/SSH/Terraform for infrastructure operations. There is no separate web dashboard — the CLI covers everything.

## Design Principles

- **One tool** — no separate dashboard and CLI to maintain
- **All-seeing** — the operator can inspect every piece of system state except user identity secrets
- **All data through S3** — the relay writes state, the CLI reads it; the CLI writes commands, the relay executes them
- **Infrastructure control** — restart, redeploy, and destroy from the same tool
- **Analytics built in** — interpret data, don't just display it

## Architecture

```
arcade-ops  ──reads──>  S3 (admin/*)           <──writes──  relay
arcade-ops  ──writes──> S3 (admin/commands/*)   ──polls──>  relay
arcade-ops  ──ssh────>  Lightsail               (restart, logs, shell)
arcade-ops  ──terraform──> AWS                  (infra lifecycle)
```

The relay remains unchanged — it writes JSON state files to S3 and polls `admin/commands/` for instructions. The CLI is a pure consumer/producer of those files plus direct access to AWS for infrastructure operations.

## S3 Layout

All admin data lives under the `admin/` prefix in the existing `arcade.seanshubin.com` bucket. Global state (relay health, connected users, identities, chat history, commands) lives at the top level. Per-version data (schema) is nested under `admin/versions/<hash>/`.

```
s3://arcade.seanshubin.com/
  admin/
    heartbeat.json                       # relay health — timestamp, uptime, client count
    connected.json                       # who's online — names, commit hashes, idle times
    identities.json                      # registered identity names (no secrets)
    chat-history.json                    # chat messages — shared across all versions
    commands/                            # command files written by the CLI, consumed by relay
    versions/
      abc123/
        schema.json                      # payload type description for this version
      def456/
        schema.json
```

### Why chat history is version-independent

Chat has no simulation context — messages are independent events where one does not affect another. There is no state to drift even if delivery is out of order. Chat history persists across version changes because it was never tied to a transition function. Only simulation context inputs (game inputs, Arcade inputs) are version-isolated.

Per-version nesting under `admin/versions/<hash>/` is retained for schema metadata, which describes the payload types for that version's simulation contexts.

### Schema file

Each version's `schema.json` describes the payload types used by that version. The relay writes it once on startup. Format:

```json
{
  "schema_version": 1,
  "fingerprint": "a1b2c3d4...",
  "types": {
    "ChatPayload": {
      "kind": "enum",
      "variants": [
        { "name": "Text", "index": 0, "fields": [{ "type": "String" }] }
      ]
    }
  }
}
```

- **`fingerprint`** — hash of the canonical `types` structure. Same fingerprint = same schema = safe to decode, regardless of commit hash. Fast equality check without inspecting the full type tree.
- **`types`** — human-readable structural description of payload types. When fingerprints differ, diffing two schemas shows exactly what changed (new variant, renamed field, type change).
- **`schema_version`** — version of the schema format itself, so the description format can evolve.

The schema is generated from the protocol crate's type definitions (the source of truth). Generation approach TBD — options include `postcard-schema` (derives from types, experimental) or a hand-written const with a unit test that fails on drift.

## Command Groups

### Observe (read from S3)

| Command | Description | S3 Source |
|---------|-------------|-----------|
| `status` | Relay health: uptime, client count, commit hash, last sync age. `--watch` for auto-refresh. | `admin/heartbeat.json` |
| `users` | Connected users with idle times, client versions. | `admin/connected.json` |
| `identities` | All registered identity names. No secrets shown. | `admin/identities.json` |
| `history` | Chat history. | `admin/chat-history.json` |
| `logs` | Chat logs. Currently local-only; remote via S3 once relay uploads logs. | Local filesystem or S3 (future) |

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
| `stats` | Message volume, active users. | `admin/chat-history.json` |
| `uptime` | Relay uptime history — when it was up, when it went down, total availability. | `admin/heartbeat.json` (track over time) |
| `versions` | Which client versions are connected, who's outdated. | `admin/connected.json` |
| `health` | Composite check: relay responding? S3 syncing? cert valid? DNS resolving? | Multiple sources. |

Analytics commands interpret raw data rather than just displaying it. `stats` shows trends, not raw numbers. `health` gives a pass/fail verdict with specifics on failures.

### Data Management (S3 reads + deletes)

| Command | Description | Mechanism |
|---------|-------------|-----------|
| `data versions` | List commit hashes with stored data. Shows schema notes and storage size. | `list_keys("admin/versions/")` + read each version's `schema.json`. |
| `data inspect <hash>` | Show schema info and stored files for a version. | Reads `admin/versions/<hash>/schema.json` and lists files. |
| `data delete <hash>` | Delete all stored data for a version (with confirmation). | Prefix delete of `admin/versions/<hash>/`. |
| `data prune` | Delete data for all versions with no connected clients (with confirmation). Combines `data versions` with `users` to find stale versions. | S3 key enumeration + prefix deletion. |

#### Cross-version compatibility

Chat history is version-independent — it lives at `admin/chat-history.json`, shared across all versions. The JSON wrapper (`from`, `payload` as base64) is stable. Chat payloads are decoded per-message; most messages decode across versions since schema evolution is typically additive.

**Schema is informational, not a gate.** Each version's `schema.json` describes the payload types for that version's simulation contexts. Schema fingerprints allow quick compatibility checks between versions.

- **Same fingerprint** — identical schema, all payloads will decode.
- **Different fingerprint** — schema evolved. The diff shows what changed.
- **Missing schema** — old version written before schema support.

## What the Relay Needs

The relay already writes `heartbeat.json`, `connected.json`, `identities.json`, and `chat-history.json` to S3, and supports admin commands (`delete-user`, `reset-identity`, `broadcast`, `drain`). Chat history is stored as a single flat file at `admin/chat-history.json`, shared across all versions.

- **Schema file:** write `admin/versions/<hash>/schema.json` once on startup. Generated from the protocol crate's payload type definitions.
- **Context-based routing:** inputs tagged with context `"chat"` are broadcast to all clients; other inputs are broadcast only to same-version clients.
- **Log upload to S3** (future): periodic upload of chat log files.

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
