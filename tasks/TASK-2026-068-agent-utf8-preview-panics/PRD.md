# PRD

## Background

The latest critical bug investigations fixed UTF-8 boundary panics in several truncation paths. Follow-up review found the same byte-slicing class in nearby agent error paths. These paths should never crash the agent while reporting malformed upstream responses or malformed tool arguments.

## Objective

Prevent recoverable agent error paths from panicking on long CJK/emoji text by using UTF-8-safe preview truncation.

## Functional Requirements

- FR-1: Embedding unexpected-format errors must include a bounded preview without slicing through a UTF-8 code point.
- FR-2: Task planner parse-failure diagnostics must log a bounded preview without slicing through a UTF-8 code point.
- FR-3: `update_task_plan` invalid string previews must return a tool error without slicing through a UTF-8 code point.

## Non-Functional Requirements

- Security: No new trust boundary or permission behavior changes.
- Performance: Preview truncation remains O(limit) and only runs in error/debug paths.
- Compatibility: Preserve existing error categories and byte budgets, allowing previews to be a few bytes shorter at character boundaries.

## Constraints

- Technical: Reuse existing `safe_truncate` helper instead of adding new truncation utilities.
- Timeline: `N/A` for autonomous execution.

## Success Metrics

- Metric: Non-ASCII boundary regression tests for all affected paths.
- Baseline: Existing byte slices can panic when the limit lands inside a multi-byte character.
- Target: Tests exercise those inputs and receive structured errors instead of panics.

## Rollout

- Rollout plan: Ship as a small Rust fix with regression tests.
- Rollback plan: Revert the commit if unexpected behavior appears; no data migration is involved.
