# PRD

## Background

Recent critical-bug work fixed UTF-8 truncation crashes in evolution logging and LLM error summarization. During review of the latest evolution workspace DB scoping changes, the human `evolution status` path still contained byte slicing for event reasons read from SQLite. Those reasons can include Chinese or emoji text from manual evolution runs or desktop-triggered summaries.

## Objective

Prevent `skilllite evolution status` from crashing when rendering long non-ASCII event reasons, without changing persisted data or desktop JSON contracts.

## Functional Requirements

- FR-1: Human status output must shorten long event reasons using UTF-8-safe character iteration.
- FR-2: The existing recent event table should continue to display an ellipsis for long reasons.
- FR-3: A regression test must seed a real evolution log event with non-ASCII text and call the status command path.

## Non-Functional Requirements

- Security: No change to authorization, sandbox, or policy behavior.
- Performance: Reason truncation remains bounded and negligible relative to DB reads.
- Compatibility: JSON output and DB schema remain unchanged; human-only formatting may only change by avoiding invalid byte slicing.

## Constraints

- Technical: Follow Rust UTF-8 safety conventions and avoid new dependencies.
- Timeline: N/A for automation; complete in the current investigation loop.

## Success Metrics

- Metric: `cmd_status(false, workspace)` with a long Chinese/emoji reason.
- Baseline: Panics at a non-character byte boundary.
- Target: Returns `Ok(())` and prints a shortened reason.

## Rollout

- Rollout plan: Ship as a small bug-fix PR with regression coverage.
- Rollback plan: Revert the single code/test commit if unexpected output regressions appear.
