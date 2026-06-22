# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-agent/src/chat_session.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/chat.rs`
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
- Current behavior:
  - `evolution_desktop` pending/confirm/status use `resolve_skills_dir_with_legacy_fallback`, preferring `skills/` with `.skills` fallback.
  - `evolution::cmd_run` resolves generated skill output as `workspace/.skills`.
  - Desktop `agent-rpc` receives workspace in JSON config, but `chat_root()` is environment based and can still resolve `~/.skilllite/chat`.
  - Agent in-process A9 evolution resolves generated skill output as `workspace/.skills`.
  - Desktop manual trigger includes `--workspace`; authorize follow-up and Life Pulse growth currently start `evolution run` without `--workspace`.

## Architecture Fit

- Layer boundaries involved:
  - Desktop assistant bridge starts CLI subprocesses.
  - CLI entry dispatch calls `skilllite-commands`.
  - `skilllite-commands` calls `skilllite-evolution` with explicit chat and skills roots.
- Interfaces to preserve:
  - Existing `skilllite evolution run --workspace <path>` CLI.
  - Existing desktop pending/confirm/reject JSON contract.
  - Existing legacy `.skills` fallback behavior.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Workspaces containing only `.skills` should continue to use `.skills`.
  - Workspaces containing `skills` should use `skills` consistently for generated pending skills and UI review.

## Design Decisions

- Decision: Reuse the existing core skill discovery fallback helper for `cmd_run`.
  - Rationale: It is already the source of truth for desktop pending/confirm/status paths.
  - Alternatives considered: Keep `.skills` for run and teach pending to search both roots.
  - Why rejected: It preserves the split write/read model and risks duplicate or ambiguous pending skills.
- Decision: Set `SKILLLITE_WORKSPACE` explicitly for desktop `agent-rpc` children after resolving the UI workspace.
  - Rationale: `skilllite_core::paths::chat_root()` uses that env var, while the L2 evolution UI reads `<workspace>/chat`.
  - Alternatives considered: Change L2 UI to read the global chat root.
  - Why rejected: It would undo the workspace scoping fixed in `TASK-2026-068` and reintroduce cross-workspace leakage.
- Decision: Test subprocess run arguments through pure helper functions.
  - Rationale: It avoids spawning an LLM-backed evolution run while still locking the high-risk contract.
  - Alternatives considered: Full desktop integration tests.
  - Why rejected: Current Tauri/LLM environment makes full integration tests heavy and brittle for this narrow fix.

## Open Questions

- [x] Should this change add a new CLI flag? No; the existing `--workspace` flag is sufficient.
- [x] Should force/manual policy be changed now? No; that is a broader governance behavior change outside the minimal critical fix.
