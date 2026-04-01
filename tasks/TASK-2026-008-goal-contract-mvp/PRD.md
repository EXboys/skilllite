# PRD

## Background

`todo/12-SELF-EVOLVING-ENGINE.md` defines `goal_contract` as a minimum core module for P7-A, but the current codebase does not yet convert user goals into a structured contract. This leaves downstream planning and risk governance without key inputs.

## Objective

Introduce executable goal-contract extraction in the `skilllite-agent` planning phase, structuring key execution dimensions from user text and injecting them into planning context to improve task decomposition consistency.

## Functional Requirements

- FR-1: Provide a `GoalContract` data structure with `goal`, `acceptance`, `constraints`, `deadline`, and `risk_level`.
- FR-2: Provide extraction functions that parse common Chinese/English markers (for example: goal/acceptance/constraints/deadline/risk level).
- FR-3: Render non-empty contracts as a unified block injected into planning user context.
- FR-4: Provide a minimum test set covering empty input, field extraction, and injection text.

## Non-Functional Requirements

- Security: Do not expand tool permissions and do not change execution gate logic.
- Performance: Keep extraction low-overhead, with no unnecessary default LLM calls beyond required behavior.
- Compatibility: Do not break existing `goal_boundaries` and task planner main flow.

## Constraints

- Technical: Follow existing crate boundaries and implement only inside `skilllite-agent`.
- Timeline: Deliver MVP only in this task, excluding strategy linkage and scoring mechanisms.

## Success Metrics

- Metric: Structured Goal Contract appears consistently in planning input.
- Baseline: Boundary-only extraction exists; full contract is absent.
- Target: Extract at least 3/5 fields when markers are present; avoid noisy injection for empty inputs.

## Rollout

- Rollout plan: Integrate directly into the default planning flow while preserving backward compatibility.
- Rollback plan: Revert the module and injection call sites, restoring `GoalBoundaries`-only behavior.
