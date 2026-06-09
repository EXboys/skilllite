# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/llm/mod.rs`
  - `crates/skilllite-agent/src/task_planner.rs`
  - `crates/skilllite-agent/src/agent_loop/helpers.rs`
- Current behavior:
  - Some error previews use expressions like `&s[..s.len().min(N)]`.
  - These expressions panic if the byte index lands inside a multi-byte UTF-8 character.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-agent` internal error and tool-argument handling.
- Interfaces to preserve:
  - Existing `LlmClient` embedding API error behavior.
  - Existing `TaskPlanner::parse_task_list` result shape.
  - Existing `update_task_plan` tool result schema.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Error previews keep the same byte limits but may end earlier to preserve UTF-8 validity.
  - Successful runtime paths are unchanged.

## Design Decisions

- Decision: Reuse `crate::types::safe_truncate` for all affected previews.
  - Rationale: It is the existing crate-level UTF-8 boundary helper and is already used in nearby truncation paths.
  - Alternatives considered: Add local helper functions.
  - Why rejected: Local helpers would duplicate established behavior and increase drift risk.

## Open Questions

- [x] Are the affected slices in user-controlled or upstream-controlled text paths? Yes.
- [x] Can this be fixed without changing public command behavior? Yes.
