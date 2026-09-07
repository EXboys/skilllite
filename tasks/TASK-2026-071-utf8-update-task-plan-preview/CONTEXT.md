# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/agent_loop/helpers.rs` (`handle_update_task_plan`)
  - `crates/skilllite-agent/src/types/string_utils.rs` (`safe_truncate`)
  - Caller: `crates/skilllite-agent/src/agent_loop/execution.rs` (no `catch_unwind`)
- Current behavior:
  - Valid JSON-array strings are parsed and accepted.
  - Non-array strings format an error with `&s[..s.len().min(120)]`, which panics when index 120 is not a char boundary.

## Architecture Fit

- Layer boundaries involved: agent planning control only; no crate graph change.
- Interfaces to preserve: `ToolResult` error shape for `update_task_plan`.

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: error preview may shrink by 1–3 bytes on a mid-character cut; message prefix is unchanged.

## Design Decisions

- Decision: Use existing `safe_truncate(s, 120)` already imported in `helpers.rs`.
  - Rationale: Same helper used for other agent previews; matches `spec/rust-conventions.md`.
  - Alternatives considered: `chars().take(40)` (char count, different budget); `catch_unwind` at the caller.
  - Why rejected: Char-count changes the visible budget; `catch_unwind` hides the root cause.

## Open Questions

- [x] Is this already covered by #96/#150? No — those are evolution status reasons and `normalize_date` compact dates.
- [x] Docs sync required? No — crash fix, unchanged command/env semantics.
