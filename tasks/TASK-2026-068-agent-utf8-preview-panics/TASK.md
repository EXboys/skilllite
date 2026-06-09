# TASK Card

## Metadata

- Task ID: `TASK-2026-068`
- Title: Fix UTF-8 preview panics in agent error paths
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: `GPT-5.5`
- Created: `2026-06-09`
- Target milestone: `N/A`

## Problem

Recent UTF-8 truncation fixes removed several byte-slicing panics, but adjacent agent error paths still slice UTF-8 strings by byte offsets when building previews. A long non-ASCII response or malformed tool argument can panic while the agent is trying to report a recoverable error.

## Scope

- In scope:
  - Replace unsafe byte-sliced previews in `skilllite-agent` error/debug paths with `safe_truncate`.
  - Add non-ASCII regression tests for the affected failure paths.
  - Validate formatting, linting, task artifacts, and Rust tests.
- Out of scope:
  - Broad refactors of LLM response parsing or planning behavior.
  - Unrelated byte slices that are ASCII/index-safe by construction.

## Acceptance Criteria

- [x] Unexpected embedding JSON responses with non-ASCII content around the 500-byte preview boundary return an error instead of panicking.
- [x] Invalid task-planner LLM output with non-ASCII content around the 500-byte debug preview boundary returns the existing parse error instead of panicking.
- [x] Invalid `update_task_plan.tasks` string arguments with non-ASCII content around the 120-byte preview boundary return a tool error instead of panicking.
- [x] Required Rust and task validation commands pass.

## Risks

- Risk: Error preview content could become slightly shorter at UTF-8 boundaries.
  - Impact: Debug output may omit a few bytes near the limit.
  - Mitigation: Preserve existing byte budgets and only move the cut point back to a valid boundary.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - `cargo test`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Commands to run:
  - See required tests.
- Manual checks:
  - Re-read modified files and task board after edits.

## Regression Scope

- Areas likely affected:
  - Agent LLM embedding response error formatting.
  - Task planner parse failure diagnostics.
  - `update_task_plan` tool argument validation errors.
- Explicit non-goals:
  - Changing successful execution behavior.
  - Changing public CLI flags or configuration.

## Links

- Source TODO section: `N/A`
- Related PRs/issues: Recent UTF-8 truncation fixes in `TASK-2026-066` and `TASK-2026-067`.
- Related docs: `spec/rust-conventions.md`, `spec/testing-policy.md`, `spec/verification-integrity.md`
