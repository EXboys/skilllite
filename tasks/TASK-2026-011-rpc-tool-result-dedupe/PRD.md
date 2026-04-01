# PRD

## Background

A tool may execute once, but RPC consumers can still show repeated `tool_result` entries if duplicate emissions happen in the same turn.

## Objective

Ensure RPC output shows each identical tool result only once per turn, while preserving full internal execution flow.

## Functional Requirements

- FR-1: Add same-turn dedupe for `tool_result` events in `RpcEventSink`.
- FR-2: Use key dimensions: `turn_id`, `tool_name`, `content_hash`, and `is_error`.
- FR-3: Reset dedupe state at `on_turn_start`.
- FR-4: Keep existing event schema unchanged.

## Non-Functional Requirements

- Security: No security behavior changes.
- Performance: O(1) average lookup with in-memory hash set.
- Compatibility: RPC event fields remain backward compatible.

## Constraints

- Technical: Implement in `crates/skilllite-agent/src/rpc.rs` sink layer only.
- Timeline: MVP in this task.

## Success Metrics

- Metric: Duplicate identical tool result events per turn.
- Baseline: Same result can be emitted more than once in a turn.
- Target: At most one emission for identical same-turn key.

## Rollout

- Rollout plan: Enable dedupe by default in RPC sink.
- Rollback plan: Remove dedupe set/key logic from sink.
