# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/mod.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`
  - `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
- Current behavior:
  - Most evolution subcommands accept `--workspace/-w` and default to `.`.
  - `reset`, `disable`, and `explain` do not accept a workspace flag.
  - `reset` uses `paths::chat_root()` for prompt/log/database state and `resolve_skills_root(None)` for evolved skill deletion, which can point at different roots.

## Architecture Fit

- Layer boundaries involved:
  - Entry crate `skilllite` owns CLI argument parsing and dispatch.
  - `skilllite-commands` owns command behavior and can call lower-layer `skilllite-core` path helpers.
  - No lower-layer crate should depend on CLI or assistant code.
- Interfaces to preserve:
  - Existing `EvolutionAction` variants and human output text.
  - Existing `skills` with `.skills` legacy fallback behavior.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Adds optional flags to existing subcommands.
  - Defaults change from implicit global/home chat root to `--workspace .`, matching sibling evolution commands.

## Design Decisions

- Decision: Add `workspace: String` to the three unscoped `EvolutionAction` variants and pass it into command handlers.
  - Rationale: This matches the existing CLI shape for status/backlog/pending/run/confirm/reject and makes the target explicit in help.
  - Alternatives considered: Continue relying on `SKILLLITE_WORKSPACE`.
  - Why rejected: It does not protect the common project-directory default and can mix chat and skills roots.
- Decision: Reuse `evolution_status::chat_root_for_workspace` and `resolve_run_skills_root`.
  - Rationale: These helpers already encode the intended workspace root and skills fallback semantics.
  - Alternatives considered: Add a new reset-specific resolver.
  - Why rejected: It risks another path policy drift.

## Open Questions

- [x] Should this change alter desktop UI reset flows? No desktop reset/disable/explain L2 surface exists in current code.
- [x] Should nested workspace project-root discovery be changed now? No; keep this fix focused on destructive command root consistency.
