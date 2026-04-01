# TASK Card

## Metadata

- Task ID: `TASK-2026-011`
- Title: Deduplicate tool_result events per turn in RPC sink
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors: `Cursor agent`
- Created: `2026-04-01`
- Target milestone:

## Problem

RPC clients may display duplicate `tool_result` events in the same turn, creating confusion even when the tool executed only once.

## Scope

- In scope:
  - Add per-turn dedupe logic in RPC event output for `tool_result`.
  - Dedupe key uses `turn_id + tool_name + content_hash (+ is_error)`.
  - Keep internal execution/messages unchanged; only event emission is deduped.
  - Add unit tests for dedupe-key behavior.
- Out of scope:
  - No changes to tool execution pipeline or model message history.
  - No UI client-side rendering changes in this task.

## Acceptance Criteria

- [x] Duplicate `tool_result` events within a turn are emitted once.
- [x] Dedupe resets on each new turn.
- [x] Deterministic tests validate key stability and distinction.

## Risks

- Risk: Over-aggressive dedupe could hide legitimate repeated outputs.
  - Impact: User may miss expected repeated tool results.
  - Mitigation: Scope dedupe to same-turn identical `(tool, content, is_error)` only.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - workspace baseline (`fmt`, `clippy`, `test`)
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test`
- Manual checks:
  - Verify duplicated identical `tool_result` in one turn is shown once by RPC event stream.

## Regression Scope

- Areas likely affected:
  - `RpcEventSink` event emission path.
- Explicit non-goals:
  - No protocol shape change for existing RPC events.

## Links

- Source TODO section: user feedback on duplicated weather tool result display
- Related PRs/issues:
- Related docs:
  - `spec/task-artifact-language.md`
