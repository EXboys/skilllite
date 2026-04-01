# PRD

## Background

TASK-2026-015 completed acceptance auto-link, but thresholds are fixed constants and rollback scope
still partially covers only prompt files. For secure production operation, acceptance policy must be
tunable and rollback boundaries must match actual mutation scope.

## Objective

Make acceptance judgement tunable by environment configuration and ensure rollback restores all core
evolution artifacts touched in normal runs (prompts, memory knowledge, evolved skills).

## Functional Requirements

- FR-1: Add env-based acceptance threshold configuration (window days, success floor, correction
  ceiling, rollback ceiling).
- FR-2: Extend snapshot/restore flow for rollback to include memory knowledge and evolved skills.
- FR-3: Preserve backward compatibility when new env vars are unset.

## Non-Functional Requirements

- Security:
  - Rollback behavior must be more conservative and no less safe than existing implementation.
- Performance:
  - Snapshot/restore overhead should remain bounded and use existing retention pruning.
- Compatibility:
  - Existing commands and status output remain stable.

## Constraints

- Technical:
  - No new dependencies.
- Timeline:
  - One focused iteration.

## Success Metrics

- Metric:
  - Acceptance policy tunability and rollback coverage completeness.
- Baseline:
  - Hard-coded acceptance thresholds; rollback restores prompt files only.
- Target:
  - Env-tunable thresholds and rollback restores prompts + memory + evolved skills when snapshot exists.

## Rollout

- Rollout plan:
  - Ship with old defaults as fallback; operators can opt-in to threshold tuning via env.
- Rollback plan:
  - Revert to previous snapshot-only prompt rollback and fixed acceptance constants.
