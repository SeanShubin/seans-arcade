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

All admin data lives under the `admin/` prefix in the existing `arcade.seanshubin.com` bucket. Global state (relay health, connected users, identities, commands) lives at the top level. Per-version data (chat history, schema, logs) is nested under `admin/versions/<hash>/`.

```
s3://arcade.seanshubin.com/
  admin/
    heartbeat.json                       # relay health — timestamp, uptime, client count
    connected.json                       # who's online — names, commit hashes, idle times
    identities.json                      # registered identity names (no secrets)
    commands/                            # command files written by the CLI, consumed by relay
    versions/
      abc123/
        schema.json                      # payload type description for this version
        chat-history.json                # chat messages for this version only
        logs/                            # chat log files for this version (future)
      def456/
        schema.json
        chat-history.json
        logs/
```

### Why per-version nesting

Previously, all versions shared one `admin/chat-history.json` file with an internal HashMap keyed by commit hash. The nested layout is better because:

- **`data versions`** is a single `list_keys("admin/versions/")` — no downloading or parsing
- **`data delete <hash>`** is a prefix delete — no read-modify-write of a shared file
- **Schema lives next to the data it describes** — self-contained per version
- **Per-version files stay small** — no monolithic file growing with every deployment
- **Client join** only fetches its own version's history — no downloading other versions' data

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
| `history` | Chat history, filterable by version or user. | `admin/versions/<hash>/chat-history.json` |
| `logs` | Chat logs. Currently local-only; remote via S3 once relay uploads logs. | `admin/versions/<hash>/logs/` (future) or local filesystem |

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
| `stats` | Message volume over time, peak hours, active users per day. | `admin/versions/*/chat-history.json`, `admin/versions/*/logs/` |
| `uptime` | Relay uptime history — when it was up, when it went down, total availability. | `admin/heartbeat.json` (track over time) |
| `versions` | Which client versions are connected, who's outdated. | `admin/connected.json` |
| `health` | Composite check: relay responding? S3 syncing? cert valid? DNS resolving? | Multiple sources. |

Analytics commands interpret raw data rather than just displaying it. `stats` shows trends, not raw numbers. `health` gives a pass/fail verdict with specifics on failures.

### Data Management (S3 reads + deletes)

| Command | Description | Mechanism |
|---------|-------------|-----------|
| `data versions` | List commit hashes with stored data. Shows message count, schema notes, last activity, storage size. | `list_keys("admin/versions/")` + read each version's `schema.json` and `chat-history.json` metadata. |
| `data inspect <hash>` | Show messages for a version: who sent what, when. Decodes each payload individually — most messages decode across versions. | Reads `from` fields from JSON (always works). Attempts `postcard` deserialization of each payload; shows decoded text on success, `[undecoded: N bytes]` on failure. |
| `data delete <hash>` | Delete all stored data for a version (with confirmation). | Prefix delete of `admin/versions/<hash>/`. |
| `data prune` | Delete data for all versions with no connected clients (with confirmation). Combines `data versions` with `users` to find stale versions. | S3 key enumeration + prefix deletion. |

#### Cross-version compatibility

Each version's data directory contains a `schema.json` alongside its `chat-history.json`. The chat history JSON wrapper (`from`, `payload` as base64) is stable across versions — only the payload blob inside is version-specific.

**Most messages decode across versions.** Postcard encodes enum variants by index. Normal schema evolution is additive — new variants for new game types, new payload kinds. Additive changes mean:

- **New binary reading old data** — always works. The new binary knows all old variants.
- **Current binary reading newer data** — messages using unchanged variants (e.g. `ChatPayload::Text`) still decode. Only messages using variants the current binary doesn't know about fail.

The only truly breaking changes are reordering variant indices, changing field types of existing variants, or removing variants — things that would never happen accidentally.

**Per-message, not per-version.** Decoding is attempted on every message individually. A version with 200 chat messages and 50 game-input messages will show all 200 chat messages decoded and 50 as `[undecoded: N bytes]` — not 250 failures because the schema "doesn't match."

**Schema is informational, not a gate.** The schema diff tells the operator what changed between versions (e.g. "added variant `GameInput` at index 1") so they can predict which messages won't decode. It never prevents decoding from being attempted.

- **Same fingerprint** — identical schema, all payloads will decode.
- **Different fingerprint** — schema evolved. The diff shows what changed. Most messages likely still decode (additive changes). The CLI decodes each message and reports individual failures.
- **Missing schema** — old version written before schema support. The CLI decodes each message with no advance info and handles failures gracefully.

Example output:

```
$ arcade-ops data versions
  HASH      MESSAGES  SCHEMA                              LAST ACTIVITY         SIZE
  abc123    47        identical                           2026-03-08 14:22:01   12 KB
  def456    250       +GameInput (1 new variant)          2026-03-07 09:15:03   48 KB
  * 9c8acd7 12        current                             2026-03-08 16:00:00   3 KB

$ arcade-ops data inspect def456
  Schema: +GameInput at index 1 (1 new variant; Text unchanged)
  Alice  2026-03-07 09:15:00  Hello!
  Bob    2026-03-07 09:15:03  Hey Alice!
  Alice  2026-03-07 09:16:12  [undecoded: 48 bytes]  (variant 1)
  Bob    2026-03-07 09:16:15  Nice shot!
  ...
  250 messages: 203 decoded, 47 undecoded
```

## What the Relay Needs

The relay already writes `heartbeat.json`, `connected.json`, `identities.json` to S3, writes chat history (currently as one monolithic file), and supports `delete-user` commands. New relay work:

- **Per-version S3 layout:** write chat history to `admin/versions/<hash>/chat-history.json` instead of one shared `admin/chat-history.json`. One write per active version group per sync cycle.
- **Schema file:** write `admin/versions/<hash>/schema.json` once on startup. Generated from the protocol crate's payload type definitions.
- **New command types:** `reset-identity`, `broadcast`, `drain`
- **Richer heartbeat data:** message counts since last sync, cumulative message count, start time (for uptime history)
- **Log upload to S3** (future): periodic upload of chat log files to `admin/versions/<hash>/logs/`

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
