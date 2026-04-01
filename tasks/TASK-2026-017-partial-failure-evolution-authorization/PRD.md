# PRD

## Background

Users need a controllable path when a tool cannot fully satisfy a request. Current behavior relies on periodic evolution and does not capture immediate user authorization for capability evolution.

## Objective

Introduce a bounded, explicit UX flow that lets users choose recovery actions for `partial_success` and `failure` outcomes, including immediate authorization to enqueue capability evolution.

## Functional Requirements

- FR-1: Detect tool outcomes as:
  - `failure`: tool result error (`is_error=true`) or structured `success=false`.
  - `partial_success`: structured `partial_success=true`.
- FR-2: For both classes above, show a multi-option prompt with:
  1) Retry current approach
  2) Switch source/params
  3) Defer to scheduled optimization
  4) `【授权进化能力】`
- FR-3: On selecting `【授权进化能力】`, backend receives a structured request and writes a queued proposal into `evolution_backlog`.
- FR-4: The authorization flow must not bypass existing policy runtime; it only creates backlog candidate(s).

## Non-Functional Requirements

- Security:
  - No new privileged execution path; proposal enqueue only.
- Performance:
  - Prompt detection must be local and lightweight (single JSON parse per tool result max).
- Compatibility:
  - Existing confirmation and clarification behaviors remain unchanged.

## Constraints

- Technical:
  - Keep architecture boundaries (`assistant` bridge calls `evolution` crate APIs).
- Timeline:
  - Implement as minimal incremental change.

## Success Metrics

- Metric: User can explicitly authorize evolution at failure/partial points.
- Baseline: No direct option in chat flow.
- Target: Option appears reliably and enqueue succeeds for authorized action.

## Rollout

- Rollout plan:
  - Ship behind existing assistant flow (no flag required).
- Rollback plan:
  - Remove prompt trigger and backend enqueue command without touching core evolution coordinator.
