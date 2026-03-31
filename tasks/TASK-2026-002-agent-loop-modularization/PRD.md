# PRD

## Background

The `agent_loop` orchestration path is a central stability point and has accumulated complexity over time.

## Objective

Improve maintainability and testability by modularizing orchestration logic without changing external behavior.

## Functional Requirements

- FR-1: Isolate major orchestration branches into dedicated modules.
- FR-2: Preserve behavior contracts for planning/execution/reflection.

## Non-Functional Requirements

- Security: no reduction in tool safety checks.
- Performance: no measurable slowdown in normal iterations.
- Compatibility: no changes to public CLI/RPC semantics.

## Constraints

- Technical: keep extension registration model unchanged.
- Timeline: staged delivery in small PRs.

## Success Metrics

- Metric: size and complexity of main loop file.
- Baseline: current single-file orchestration concentration.
- Target: smaller core file and clearer boundaries.

## Rollout

- Rollout plan: refactor in isolated steps with parity tests.
- Rollback plan: revert latest split if regression appears.
