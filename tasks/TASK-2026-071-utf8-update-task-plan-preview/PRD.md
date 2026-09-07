# PRD

## Background

Models frequently pass `update_task_plan.tasks` as a string instead of a JSON array. The handler already recovers when that string is a JSON array. When it is not, the error path previews the raw string with a 120-byte slice. That slice is not UTF-8-safe.

## Objective

- Agent turns that hit the non-array `tasks` string path return a tool error instead of panicking.
- CJK/emoji previews stay on character boundaries.

## Functional Requirements

- FR-1: Invalid `tasks` strings still produce `is_error` with the existing "must be a JSON array" guidance.
- FR-2: Preview truncation must not panic on inputs where byte index 120 is mid-character.
- FR-3: A unit test covers a long CJK prose `tasks` value.

## Non-Functional Requirements

- Security: N/A (error-preview only; no new trust boundary).
- Performance: N/A (single string truncate).
- Compatibility: Error text may be a few bytes shorter when the 120-byte cut lands mid-character; callers must not depend on exact preview length.

## Constraints

- Technical: Reuse existing `safe_truncate`; no new helpers or crate deps.
- Timeline: N/A (automation sweep).

## Success Metrics

- Metric: Focused regression test plus `cargo test -p skilllite-agent`.
- Baseline: Byte slice panics on a 121+ byte CJK string.
- Target: Same input returns a UTF-8 error string.

## Rollout

- Rollout plan: Merge the one-line preview fix with the regression test.
- Rollback plan: Revert the commit; behavior returns to the panic path.
