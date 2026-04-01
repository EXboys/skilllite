# TASK Card

## Metadata

- Task ID: `TASK-2026-002`
- Title: Continue modularization of `agent_loop` core flow
- Status: `ready`
- Priority: `P1`
- Owner: `TBD`
- Contributors: `TBD`
- Created: `2026-03-31`
- Target milestone: `v0.1.x maintenance`

## Problem

`agent_loop` core file size and orchestration complexity have grown, increasing cognitive load and review risk.

## Scope

- In scope:
  - Split `handle_llm_response` and `process_tool_calls` responsibilities into dedicated modules.
  - Keep behavior parity for planning, execution, and reflection loops.
- Out of scope:
  - New user-facing agent features.

## Acceptance Criteria

- [x] Main loop file becomes smaller and clearer (846 → 716 lines).
- [x] New modules have focused tests for key branches.
- [x] Existing agent integration behavior remains unchanged (163/163 tests pass).

## Risks

- Risk: loop control behavior changes unintentionally.
  - Impact: task completion regressions, tool-call handling errors.
  - Mitigation: branch-level tests and before/after behavior checks.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite`
- Manual checks:
  - Run representative chat/tool scenarios.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-agent/src/agent_loop/*`
  - `crates/skilllite-agent/src/chat_session.rs`
- Explicit non-goals:
  - No protocol changes to agent RPC.

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (`0.2 #5`, `0.2 #154`, `5.2`)
- Related PRs/issues: `TBD`
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
