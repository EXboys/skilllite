# TASK Card

## Metadata

- Task ID: `TASK-2026-004`
- Title: Unify Tool Lifecycle Contract
- Status: `done`
- Priority: `P0`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

Current tool execution has registration unification, but lifecycle steps are implicit and split across modules.
This makes it harder to audit and test input validation, permission checks, execution, and result rendering in one place.

## Scope

- In scope:
  - Add a unified lifecycle contract to `RegisteredTool`: `validate_input`, `check_permissions`, `execute` dispatch hook (via registry flow), and `render_use_result`.
  - Expose per-tool metadata flags: `is_read_only`, `is_destructive`, `is_concurrency_safe`.
  - Ensure registry execution path consistently follows lifecycle order.
  - Add regression tests for lifecycle metadata and validation gate behavior.
- Out of scope:
  - Redesigning every builtin tool implementation internals.
  - Changing public CLI/MCP command semantics.

## Acceptance Criteria

- [x] Registry execution path enforces `validate_input -> check_permissions -> execute -> render/use_result`.
- [x] Tool lifecycle metadata is available through a unified profile API and covered by tests.
- [x] Existing agent loop behavior remains green after full workspace validation.

## Risks

- Risk: duplicate `on_tool_result` emission after centralizing result rendering.
  - Impact: noisy UI/event stream and flaky assertions.
  - Mitigation: remove loop-level duplicate emissions and keep one rendering point in registry.

## Validation Plan

- Required tests:
  - `cargo fmt --check`
  - `cargo clippy --all-targets`
  - `cargo test`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets`
  - `cargo test`
- Manual checks:
  - Verify lifecycle profile values for representative tools (`read_file`, `write_file`, `run_command`).

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-agent/src/extensions/registry.rs`
  - `crates/skilllite-agent/src/agent_loop/execution.rs`
  - Tool result event emission path.
- Explicit non-goals:
  - Skill execution implementation logic.
  - Sandbox security policy behavior.

## Links

- Source TODO section: `todo/skilllite-vs-claude-code.md` (A. 工具统一契约)
- Related PRs/issues:
- Related docs: N/A (no user-facing command/env/architecture-doc behavior change)
