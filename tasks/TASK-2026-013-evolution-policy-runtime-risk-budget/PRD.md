# PRD

## Background

P7-C introduced proposal governance with coordinator and shadow mode, but safety control is still
coarse-grained. To support P7-D, the system needs a transparent policy runtime and explicit risk
budgeting so high-risk evolution decisions are explainable and bounded.

## Objective

Introduce policy-runtime gating in evolution coordinator so every execution decision has an
auditable reason chain, and enforce per-risk daily budgets for auto execution.

## Functional Requirements

- FR-1: Coordinator evaluates selected proposal through a policy runtime that emits one of
  `allow`, `ask`, `deny` with deterministic reason chain.
- FR-2: Policy runtime enforces per-risk daily budget for auto execution (at least low risk path).
- FR-3: Coordinator writes policy decision summary into `evolution_backlog.note`.
- FR-4: Force run keeps bypass behavior to preserve manual override path.

## Non-Functional Requirements

- Security:
  - Defaults must remain non-more-permissive than current behavior.
- Performance:
  - Policy evaluation adds only lightweight SQL reads on backlog table.
- Compatibility:
  - Existing env vars and coordinator behavior remain backward compatible when new vars are unset.

## Constraints

- Technical:
  - No new crate dependencies.
- Timeline:
  - One iteration (within 1-2 weeks roadmap slot).

## Success Metrics

- Metric: Auto execution policy decisions are traceable in backlog notes.
- Baseline: Backlog notes do not include structured policy reason chain.
- Target: 100% of coordinator decisions include policy runtime reason summary.

## Rollout

- Rollout plan:
  - Ship with conservative defaults.
  - Observe behavior in shadow/off combinations.
- Rollback plan:
  - Disable policy runtime via env flag and keep prior coordinator gating path.
