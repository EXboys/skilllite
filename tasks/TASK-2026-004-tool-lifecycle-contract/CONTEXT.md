# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/extensions/registry.rs`
  - `crates/skilllite-agent/src/agent_loop/execution.rs`
  - `crates/skilllite-agent/src/extensions/mod.rs`
- Current behavior:
  - Tool registration is already centralized in `ExtensionRegistry`, but lifecycle steps were implicit.
  - Input validation, permission gating, dispatch, and result rendering were split across modules.
  - `on_tool_result` was emitted in the loop layer, which made lifecycle ownership unclear and could cause duplicate reporting when refactoring.

## Architecture Fit

- Layer boundaries involved:
  - Changes are contained in the `skilllite-agent` crate extension layer and loop orchestration.
  - No dependency direction changes between workspace layers.
- Interfaces to preserve:
  - `ExtensionRegistry::execute(...)` remains the single execution entry.
  - Existing `ToolHandler` dispatch variants remain compatible.
  - `ToolResult` contract and planner control behavior (`complete_task`, `update_task_plan`) remain unchanged.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Added lifecycle hooks and metadata on `RegisteredTool` without breaking existing call sites.
  - Kept tolerant JSON recovery behavior for `write_file`/`write_output` via `validate_input` exception.
  - Added `ToolExecutionProfile` and `tool_profile()` as additive APIs.

## Design Decisions

- Decision: enforce lifecycle order in registry execution path:
  - `validate_input -> check_permissions -> execute(dispatch) -> render_use_result`
  - Rationale:
    - Centralized lifecycle makes auditing and tests deterministic.
    - New tools can plug in via registration without changing loop orchestration logic.
  - Alternatives considered:
    - Keep lifecycle split between loop + builtin handlers.
    - Implement a fully trait-object based tool runtime replacing current enum handlers.
  - Why rejected:
    - Split lifecycle does not solve “统一契约”目标 and keeps ownership ambiguous.
    - Full runtime rewrite is high-risk for this task scope and unnecessary for immediate value.

- Decision: infer default tool metadata from capabilities (`ToolExecutionProfile::from_capabilities`):
  - Rationale:
    - Avoid per-tool manual flag wiring drift.
    - Keep profile derivation deterministic and testable.
  - Alternatives considered:
    - Manual profile assignment for every tool registration.
  - Why rejected:
    - Higher maintenance cost and easier to miss on new tools.

## Open Questions

- [ ] Should `ToolExecutionProfile` be surfaced into telemetry/audit logs for runtime policy analysis?
- [ ] Do we need explicit per-tool override policies for edge cases where capability-based inference is too coarse?
