# TASK Card

## Metadata

- Task ID: TASK-2026-071
- Title: Merge consecutive Claude user messages after tool results
- Status: `done`
- Priority: `P0`
- Owner: agent
- Contributors: agent
- Created: 2026-09-13
- Target milestone: next patch

## Problem

`convert_messages_for_claude` flushes batched `tool_result` blocks as a `user` message, then emits a following in-memory `user` nudge (planning continue, depth-limit, or closing summary) as a second consecutive `user` message. Anthropic's Messages API requires alternating `user` / `assistant` roles, so the next LLM call returns HTTP 400 and the Claude planning run cannot produce a closing reply.

## Scope

- In scope:
  - Merge consecutive same-role Claude messages after conversion
  - Attach following user text to the same `user` message as the `tool_result` blocks
  - Regression tests for the closing-summary / planning-nudge sequences
- Out of scope:
  - OpenAI-compatible conversion (consecutive user roles are valid there)
  - Reloading tool rows from transcript into later turns
  - Planning-loop nudge wording

## Acceptance Criteria

- [x] `tool_result` followed by a user nudge converts to a single Claude `user` message
- [x] Existing Claude conversion tests still pass (system extract, tool batching)
- [x] Non-ASCII closing-summary text is preserved (UTF-8 safe merge)
- [x] `cargo test -p skilllite-agent` and workspace `cargo fmt` / `clippy` / `cargo test` pass

## Risks

- Risk: Over-merging two unrelated user turns could hide a real conversation boundary
  - Impact: Model sees combined user text in one turn
  - Mitigation: Only consecutive same-role messages already violate the API; merging is the required wire format. Do not merge across an assistant turn.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent`
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
- Manual checks: inspect converted JSON roles for the closing-summary fixture

## Regression Scope

- Areas likely affected: Claude native `/v1/messages` conversion
- Explicit non-goals: OpenAI path, transcript reload, sandbox

## Links

- Source TODO section: daily critical-bug sweep (2026-09-13)
- Related PRs/issues: sibling of #154 (transcript persistence) — this is the live in-memory Claude wire format
- Related docs: N/A (internal conversion; no env/command change)
