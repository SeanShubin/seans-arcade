# Sean's Arcade

A multiplayer arcade application built with Bevy, distributed as a single Rust binary from seanshubin.com.

## Documentation Structure

| Folder | Purpose |
|--------|---------|
| [`docs/decisions.md`](docs/decisions.md) | **Decision register** — terse "what was decided" bullets with links to rationale |
| [`docs/project-overview.md`](docs/project-overview.md) | **Plan** — evolution path, current phase, full document index |
| [`docs/architecture-decisions.md`](docs/architecture-decisions.md) | **Decision rationale** — analysis, alternatives considered, justifications |
| [`docs/architecture/`](docs/architecture/) | **Architecture** — how things work (networking, distribution, engine) |
| [`docs/research/`](docs/research/) | **Research** — game design studies, technical investigations, learning notes |

## Where to Start

- **What is this project?** → [project-overview.md](docs/project-overview.md)
- **What has been decided?** → [decisions.md](docs/decisions.md)
- **Why was something decided?** → [architecture-decisions.md](docs/architecture-decisions.md)
- **How does networking work?** → [network-architecture.md](docs/architecture/network-architecture.md)
- **Full document list** → [Document Index](docs/project-overview.md#document-index) in project-overview.md

## Adding Documentation

1. **Choose the right location:**
   - New decision → add a bullet to `docs/decisions.md`
   - Rationale for a decision → add a section to `docs/architecture-decisions.md`
   - How something works → add a file to `docs/architecture/`
   - Investigation or study → add a file to `docs/research/`

2. **Link it in:**
   - Add the new file to the [Document Index](docs/project-overview.md#document-index) in `project-overview.md`
   - If it supports a decision, link from the relevant bullet in `decisions.md`

3. **Naming:** Use lowercase kebab-case (`my-new-topic.md`).

## Maintaining Documentation

- **decisions.md** stays terse — state *what* was decided, not *why*. Link to `architecture-decisions.md` or architecture docs for rationale.
- **architecture-decisions.md** holds the *why* — alternatives considered, tradeoffs, justifications.
- **project-overview.md** is the canonical document index — every doc should appear there.
- Keep links relative. From repo root use `docs/...`; from inside `docs/` omit the prefix.
