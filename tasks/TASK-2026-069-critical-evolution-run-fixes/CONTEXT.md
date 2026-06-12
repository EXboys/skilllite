# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-commands/src/evolution_status.rs`
  - `crates/skilllite-commands/src/evolution.rs`
  - `crates/skilllite-commands/src/evolution_desktop.rs`
  - `crates/skilllite-assistant/src-tauri/src/life_pulse.rs`
  - `crates/skilllite-core/src/skill/discovery.rs`
- Current behavior:
  - Human status display truncates event `reason` with byte slicing.
  - `evolution run` resolves evolved skill output under `.skills`.
  - Desktop pending-skill paths resolve `skills/` with `.skills` fallback.
  - Life Pulse growth due checks use the selected workspace, but the growth run
    subprocess omits `--workspace`.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-commands` may depend on `skilllite-core` discovery helpers.
  - Desktop bridge invokes CLI behavior through subprocess arguments.
- Interfaces to preserve:
  - Existing CLI flags and JSON contracts.
  - Existing `resolve_skills_dir_with_legacy_fallback` behavior.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes:
  - Workspaces with only `.skills/` must continue to receive evolved skills there.
  - Workspaces with `skills/` should receive evolved skills under `skills/`.

## Design Decisions

- Decision: Introduce small local helpers instead of new abstractions.
  - Rationale: The fix is narrow and can reuse existing core path resolution.
  - Alternatives considered: Refactor all evolution workspace/path handling.
  - Why rejected: Too broad for a critical bug-fix PR and higher regression risk.
- Decision: Pass `--workspace` from Life Pulse growth execution.
  - Rationale: The due check and run should operate on the same workspace.
  - Alternatives considered: Rely on `SKILLLITE_WORKSPACE` env only.
  - Why rejected: Explicit CLI arguments already define the scoped desktop pattern.

## Open Questions

- [x] Are docs required? No CLI contract or user-facing flag semantics change.
- [x] Are security specs required? No sandbox, auth, or policy gates change.
