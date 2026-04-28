# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/wiki.rs`
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - EN/ZH README and architecture docs.
- Current behavior: `wiki compile` is manual; `query` searches current Markdown and does not check whether raw sources changed.

## Architecture Fit

- Layer boundaries involved: CLI entry -> `skilllite-commands` -> core path helpers.
- Interfaces to preserve: no changes to memory, `chat_root`, SQLite, `.skills`, agent loop, or background processes.

## Dependency and Compatibility

- New dependencies: None planned.
- Backward compatibility notes: New `status` command and skip flags are additive. Manual `compile` remains available.

## Design Decisions

- Decision: Use content fingerprints stored in compiled article frontmatter.
  - Rationale: Stale detection is deterministic and does not require a database.
  - Alternatives considered: SQLite index or file watcher.
  - Why rejected: User wants Markdown-only wiki; watcher would cause surprising background writes.

- Decision: Refresh only from explicit user commands (`ingest`, `query`) by default.
  - Rationale: Makes wiki dynamic at use time while avoiding silent daemon behavior.
  - Alternatives considered: Always-on watcher.
  - Why rejected: Too much Git churn and operational surface for the current phase.

## Open Questions

- [ ] Future task: integrate wiki freshness checks into chat/agent context loading without hidden writes.
