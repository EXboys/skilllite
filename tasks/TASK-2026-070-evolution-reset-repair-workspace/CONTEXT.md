# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs`: `EvolutionAction` argument definitions.
  - `skilllite/src/dispatch/mod.rs`: CLI dispatch to command crate.
  - `crates/skilllite-commands/src/evolution.rs`: reset, run, repair command implementations.
  - `crates/skilllite-commands/src/evolution_status.rs`: workspace and chat-root helpers.
  - `crates/skilllite-core/src/skill/discovery.rs`: effective skills-root resolver.
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/skill_rpc.rs`: desktop repair subprocess.
- Current behavior:
  - `cmd_run` resolves the requested workspace, sets `SKILLLITE_WORKSPACE` to the absolute workspace, uses `paths::chat_root()`, and uses `resolve_skills_dir_with_legacy_fallback`.
  - `cmd_reset` does not accept a workspace and directly uses `paths::chat_root()`, which ignores relative `SKILLLITE_WORKSPACE` values.
  - `cmd_reset` deletes only `<workspace>/.skills/_evolved` through a legacy helper.
  - `cmd_repair_skills` validates only the legacy helper result, so `skills/_evolved` can be skipped.

## Architecture Fit

- Layer boundaries involved:
  - Entry CLI (`skilllite`) remains responsible for argument parsing and dispatch.
  - `skilllite-commands` remains responsible for command behavior.
  - `skilllite-core` continues to provide shared skills-root discovery.
- Interfaces to preserve:
  - Existing command names and `--from-source` repair behavior.
  - Legacy `.skills` fallback when `skills/` does not exist.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - New `--workspace/-w` flags align reset/repair with adjacent evolution commands.
  - Default workspace `"."` changes reset away from legacy global fallback, but fixes the destructive split-brain behavior and remains explicit for callers.

## Design Decisions

- Decision: Reuse `resolve_run_skills_root` for reset and repair.
  - Rationale: It already implements the modern `skills/` then `.skills` fallback used by `evolution run`.
  - Alternatives considered: Keep the old helper and special-case `_evolved`.
  - Why rejected: The old helper encodes the broken `.skills`-only behavior.
- Decision: Use `chat_root_for_workspace` for reset.
  - Rationale: It gives deterministic `<workspace>/chat` behavior for relative and absolute workspaces without mutating process-global environment.
  - Alternatives considered: Set `SKILLLITE_WORKSPACE` and keep `paths::chat_root()`.
  - Why rejected: Environment mutation is unnecessary and easier to get wrong in tests or embedded callers.

## Open Questions

- [x] Should unrelated legacy commands be changed in this task? No; keep the fix scoped to reset/repair because they are the concrete high-impact paths found.
- [x] Are docs required? No user docs currently document reset/repair workspace flags in detail; PR/task artifacts and CLI help cover the new flags.
