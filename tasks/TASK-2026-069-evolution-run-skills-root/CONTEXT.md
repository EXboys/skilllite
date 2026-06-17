# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-core/src/skill/discovery.rs`
- Current behavior:
  - `evolution_desktop::resolve_skills_root` uses `resolve_skills_dir_with_legacy_fallback(&root, "skills")`.
  - `evolution_status` counts pending skills from the same effective skills root.
  - `evolution::cmd_run` now uses the same `skills/` default with `.skills/` legacy fallback policy.
  - When `skills/` exists, generated pending skills and desktop pending/status/confirm operations target the same root.

## Architecture Fit

- Layer boundaries involved:
  - CLI entry crate dispatches to `skilllite-commands`.
  - `skilllite-commands` may call lower-layer `skilllite-core` discovery helpers.
- Interfaces to preserve:
  - Existing `evolution run` CLI flags and JSON response shape.
  - Existing legacy `.skills` fallback behavior for projects without `skills/`.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes:
  - Existing projects with only `.skills/` continue to use `.skills/`.
  - Projects with `skills/` get consistent behavior across run/pending/status/confirm.

## Design Decisions

- Decision: Update `evolution.rs` root resolution to call the same discovery helper used by desktop reads.
  - Rationale: Reuses the established project skills-root policy and avoids duplicating fallback rules.
  - Alternatives considered: Change desktop pending/status/confirm back to `.skills` only.
  - Why rejected: That would undo the newer `skills/` default and break current `skilllite init` style workspaces.

## Open Questions

- [ ] Whether assistant background authorize should pass `--workspace` to `evolution run`; tracked as residual risk outside this minimal skills-root fix.
