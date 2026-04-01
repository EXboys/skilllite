# PRD

## Background

P7-C/P7-D introduced proposal governance, coordinator execution gates, and policy runtime.
Execution lifecycle is persisted, but acceptance closure is still incomplete because
`acceptance_status` is not automatically linked to post-execution core metrics.

## Objective

Automatically determine proposal acceptance outcome from a deterministic metrics window so executed
proposals are marked `met`, `not_met`, or kept `pending_validation` based on measurable signals.

## Functional Requirements

- FR-1: Add acceptance evaluation logic that reads the post-execution metrics window.
- FR-2: Use three signals for judgement: `first_success_rate`, `user_correction_rate`, rollback
  rate.
- FR-3: Auto-update `evolution_backlog.acceptance_status` and append a concise reason summary to
  `note`.
- FR-4: Preserve compatibility for proposals without enough window data (stay
  `pending_validation`).

## Non-Functional Requirements

- Security:
  - No relaxed policy or sandbox behavior.
- Performance:
  - Evaluation should use lightweight aggregate SQL; no full-log scans.
- Compatibility:
  - Existing backlog schema and CLI output remain backward compatible.

## Constraints

- Technical:
  - No new dependencies.
- Timeline:
  - One short iteration.

## Success Metrics

- Metric:
  - Auto-judged acceptance coverage for executed proposals.
- Baseline:
  - Executed proposals remain manually interpreted as `pending_validation`.
- Target:
  - Newly executed proposals are deterministically evaluated and status-linked by metrics window.

## Rollout

- Rollout plan:
  - Ship with conservative thresholds and minimum sample gate.
- Rollback plan:
  - Disable auto-link call site and keep previous `pending_validation` behavior.
