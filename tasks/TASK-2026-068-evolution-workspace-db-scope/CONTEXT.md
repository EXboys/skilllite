# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/dispatch/mod.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-core/src/paths.rs`
- Current behavior:
  - `EvolutionAction::Backlog.workspace` is discarded in dispatch.
  - `cmd_backlog` calls `query_backlog_desktop(limit)` for `--json --hide-closed` and `paths::chat_root()` otherwise.
  - `query_backlog_desktop`, `query_proposal_status`, `authorize_capability_evolution`, and `log_manual_evolution_trigger` use `paths::chat_root()` directly.
  - `build_evolution_status_snapshot` resolves the workspace for config but opens the DB via `paths::chat_root()`.

## Architecture Fit

- Layer boundaries involved:
  - Entry crate dispatches CLI options into `skilllite-commands`.
  - `skilllite-commands` may depend on `skilllite-core` and `skilllite-evolution`.
- Interfaces to preserve:
  - Existing desktop JSON DTO structs and command output shapes.
  - `skilllite_core::paths::chat_root()` global semantics for non-workspace-specific callers.

## Dependency and Compatibility

- New dependencies: none planned.
- Backward compatibility notes:
  - Omitted workspace uses the CLI default `.` and resolves to the current working directory workspace.
  - CLI callers that pass `--workspace` get the documented workspace-scoped behavior.

## Design Decisions

- Decision: use an explicit `chat_root_for_workspace` helper in command code and pass `workspace` through affected APIs.
  - Rationale: avoids relying on process-global `SKILLLITE_WORKSPACE` mutation and keeps the fix localized.
  - Alternatives considered: set `SKILLLITE_WORKSPACE` before each DB open.
  - Why rejected: process-wide env mutation is more fragile and can create test/order side effects.

## Open Questions

- [ ] Whether `reset`, `disable`, and `explain` should grow `--workspace` in a later command cleanup task.
