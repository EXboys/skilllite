# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs` and dispatch modules for CLI command wiring.
  - `crates/skilllite-commands/src/init.rs` has `ensure_project_wiki`.
  - `crates/skilllite-core/src/paths.rs` has `project_wiki_root`.
  - Docs under `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.
- Current behavior: `skilllite init` creates `.skilllite/wiki/` skeleton; no `skilllite wiki` subcommands exist.

## Architecture Fit

- Layer boundaries involved: entry CLI routes to `skilllite-commands`; `skilllite-commands` uses lower-layer `core` path helpers.
- Interfaces to preserve: existing init, memory, and skill discovery behavior.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Additive CLI commands only.

## Design Decisions

- Decision: Implement deterministic Markdown operations first.
  - Rationale: Matches the user's requirement that LLM Wiki does not need SQLite and avoids over-scoping into research/compile agents.
  - Alternatives considered: LLM-backed compile/query in the first pass.
  - Why rejected: Larger blast radius and requires model/API behavior decisions.

- Decision: `query` scans `_index.md`, `wiki/`, `lessons/`, and `raw/` Markdown.
  - Rationale: This approximates LLM Wiki's index-first behavior while keeping MVP deterministic.
  - Alternatives considered: SQLite full-text search.
  - Why rejected: User explicitly confirmed no SQLite for wiki.

## Open Questions

- [ ] Whether follow-up work should add LLM compile/research commands after MVP.
