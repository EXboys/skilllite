# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/wiki.rs`
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - EN/ZH README and architecture docs.
- Current behavior: `wiki init`, `ingest`, `query`, and `lint` exist. Query scans Markdown by terms. Lint validates required files/directories and basic frontmatter.

## Architecture Fit

- Layer boundaries involved: entry CLI -> `skilllite-commands` -> `skilllite-core` path helpers.
- Interfaces to preserve: no changes to memory, chat root, skills discovery, or SQLite behavior.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Additive command and flags only.

## Design Decisions

- Decision: Add deterministic `compile` before LLM-backed compilation.
  - Rationale: Enables the raw -> wiki article layer now, while keeping tests deterministic and no API dependency.
  - Alternatives considered: LLM compile first.
  - Why rejected: Requires API/model behavior choices and larger validation surface.

- Decision: Query modes are filesystem scopes rather than model modes.
  - Rationale: Preserves Markdown-only/no SQLite behavior and can be verified mechanically.
  - Alternatives considered: Vector or SQLite search.
  - Why rejected: User explicitly wanted wiki without SQLite.

## Open Questions

- [ ] Future task: LLM-backed `wiki compile` and multi-agent `wiki research`.
