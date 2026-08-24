# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Reject empty recovered write_file content
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-08-16`
- Target milestone:

## Problem

When an LLM `write_file` / `write_output` tool-call JSON is truncated after `"content": "`, recovery treats the missing payload as an empty string and overwrites the target file with 0 bytes. The tool returns success, so the agent may continue as if the rewrite completed. Existing user files are silently wiped.

Concrete trigger: user asks the agent to rewrite an existing source file; the provider hits `SKILLLITE_MAX_TOKENS` / stream end right as the content string opens (path-first argument order). Recovery yields `{path, content:""}` and `write_file` overwrites the file.

## Scope

- In scope:
  - Fail closed when truncated-JSON recovery for `write_file` / `write_output` has missing or empty `content`.
  - Keep existing recovery of non-empty partial content (with the truncation warning).
  - Keep valid complete JSON that intentionally writes empty content.
  - Regression tests that prove the wipe path and the kept recovery path.
- Out of scope:
  - Inner-`"path"` mis-parse when content precedes path (known near-miss; typical tool calls are path-first).
  - Atomic write / `search_replace` recovery.
  - Dotenv / symlink / workspace-root issues covered by open PRs.

## Acceptance Criteria

- [x] Truncated `write_file` JSON with empty recovered content does not overwrite an existing file and returns `is_error`.
- [x] Truncated `write_output` JSON with empty recovered content does not overwrite an existing output file and returns `is_error`.
- [x] Recovered non-empty partial content still writes and includes the truncation warning.
- [x] Valid JSON `{"path":"...","content":""}` still writes empty content (intentional).
- [x] Validation commands actually run; task artifacts and `tasks/board.md` stay in sync.

## Risks

- Risk: Rejecting a legitimate empty recovered write that a caller wanted.
  - Impact: Tool call fails instead of creating/clearing a file.
  - Mitigation: Complete valid JSON with empty `content` still succeeds; only the invalid-JSON recovery path is tightened.
- Risk: False positive from this sweep.
  - Impact: Unnecessary PR.
  - Mitigation: Reproduced the wipe with `execute_builtin_tool` against an existing file before changing production code.

## Validation Plan

- Required tests:
  - New builtin-tool tests for empty recovered `write_file` / `write_output`, partial recovery, and intentional empty JSON.
- Commands to run:
  - `cargo test -p skilllite-agent recovered_empty -- --nocapture`
  - `cargo test -p skilllite-agent`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-agent --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read the recovery gate after the edit to confirm empty recovered writes fail closed.

## Regression Scope

- Areas likely affected:
  - `write_file` / `write_output` truncated-JSON recovery in `execute_builtin_tool`.
- Explicit non-goals:
  - Changing valid JSON empty-content writes.
  - Changing non-empty partial recovery.
  - Sandbox, evolution, or desktop chat-root work.

## Links

- Source TODO section: scheduled critical-bug automation
- Related PRs/issues: none for this wipe path; dotenv/symlink/bash issues remain in open PRs
- Related docs: `docs/en/ENV_REFERENCE.md` (`SKILLLITE_MAX_TOKENS` truncation note)
