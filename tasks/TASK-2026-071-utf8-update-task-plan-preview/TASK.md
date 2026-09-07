# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: UTF-8-safe update_task_plan error preview
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-09-07`
- Target milestone:

## Problem

`handle_update_task_plan` previews a non-array `tasks` string with `&s[..s.len().min(120)]`. When the LLM (common on long Chinese plans) sends `tasks` as prose longer than 120 UTF-8 bytes, byte 120 is often mid-character and the agent process panics instead of returning a tool error.

## Scope

- In scope:
  - Replace the byte slice in the `update_task_plan` non-array-string error path with `safe_truncate`.
  - Add a CJK regression test that would panic before the fix.
- Out of scope:
  - Changing how stringified JSON arrays are accepted.
  - Broader truncation cleanup in other crates.
  - Opening additional PRs for already-tracked drafts (#96, #150, #152).

## Acceptance Criteria

- [ ] `handle_update_task_plan` does not panic when `tasks` is a long CJK string that is not a JSON array.
- [ ] The error still states that `tasks` must be a JSON array and includes a UTF-8-safe preview.
- [ ] A regression test fails if the byte-slice preview is restored.

## Risks

- Risk: Error message length/content changes slightly for long ASCII previews if `safe_truncate` stops earlier than 120 bytes on a boundary.
  - Impact: Low; preview text only.
  - Mitigation: Keep the 120-byte budget; `safe_truncate` only walks back to the previous char boundary.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent`
- Commands to run:
  - `cargo test -p skilllite-agent update_task_plan_rejects_non_array_cjk_string_without_panic -- --nocapture`
  - `cargo test -p skilllite-agent`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks: N/A (unit-reproducible panic)

## Regression Scope

- Areas likely affected:
  - Agent planning-mode `update_task_plan` error replies only.
- Explicit non-goals:
  - No planner merge/accept behavior changes.
  - No CLI/docs/env changes.

## Links

- Source TODO section: N/A (critical-bug sweep 2026-09-07)
- Related PRs/issues: sibling UTF-8 drafts #96, #150
- Related docs: N/A
