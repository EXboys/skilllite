# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-core/src/paths.rs` for shared path helpers.
  - Docs under `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.
- Current behavior: memory uses `chat_root()` / `chat_root()/memory`; existing root `.skills/` discovery is already present and must remain unchanged. There is no confirmed project Repo Wiki root.

## Architecture Fit

- Layer boundaries involved: `core` may expose path helpers; agent/commands may consume them according to existing dependency direction.
- Interfaces to preserve: Existing memory APIs, `chat_root()`, and evolution feedback/backlog paths must remain unchanged.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: This task is additive for project wiki only. It must not invent fallback paths for roots that are not implemented.

## Design Decisions

- Decision: Use `<project>/.skilllite/wiki/` for the project Repo Wiki.
  - Rationale: Matches Qoder's tool-owned project directory pattern while keeping the wiki commit-friendly Markdown.
  - Alternatives considered: Root-level `wiki/`.
  - Why rejected: It occupies the project namespace and can conflict with existing user directories.

- Decision: Keep memory on the existing `chat_root` path.
  - Rationale: Memory, transcripts, feedback, and evolution backlog are currently coupled to existing chat storage.
  - Alternatives considered: Project-local memory.
  - Why rejected: It would expand scope and risks state split.

- Decision: Do not use SQLite for Repo Wiki.
  - Rationale: Wiki is Markdown source-of-truth; any future index must be a rebuildable cache outside this task.
  - Alternatives considered: SQLite-backed wiki search.
  - Why rejected: Not needed for this first project wiki root and contradicts the confirmed design.

## Open Questions

- [ ] None.
