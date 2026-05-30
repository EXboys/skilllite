# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-core/src/skill/discovery.rs`
- Current behavior:
  - Desktop pending/status/confirm paths use `resolve_skills_dir_with_legacy_fallback(workspace, "skills")`.
  - `evolution run` and repair helper resolution return `workspace/.skills` directly.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-commands` should call shared `skilllite-core` discovery helpers rather than duplicating root policy.
- Interfaces to preserve:
  - Existing CLI flags, JSON payloads, and legacy `.skills` fallback semantics.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - `.skills` remains the effective root when `skills/` is absent.
  - Workspaces with both roots follow the established default of preferring `skills/`.

## Design Decisions

- Decision: Reuse `skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback` for evolution skill root resolution.
  - Rationale: It is already the shared policy for skill command and desktop evolution read paths.
  - Alternatives considered: Change desktop pending/status/confirm back to `.skills`.
  - Why rejected: That would diverge from current `skilllite init` default behavior and other command helpers.

## Open Questions

- [x] Is a docs update required? No user-facing command or documented default changes; the fix restores the current documented `skills/` default with `.skills` compatibility.
