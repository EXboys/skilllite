# PRD

## Background

Sandbox platform files are large and partially repetitive. This slows feature updates and increases risk of platform divergence.

## Objective

Reduce duplicated execution flow code while preserving current sandbox security and compatibility behavior.

## Functional Requirements

- FR-1: Introduce shared flow for resource limits, logging, and error handling.
- FR-2: Keep platform-specific isolation code isolated behind clear interfaces.

## Non-Functional Requirements

- Security: no weakening of current policy gates.
- Performance: no material execution slowdown.
- Compatibility: existing env/config behavior remains backward compatible.

## Constraints

- Technical: must preserve level semantics (L1/L2/L3).
- Timeline: suitable for incremental PRs, not a big-bang rewrite.

## Success Metrics

- Metric: duplicated logic blocks across platform modules.
- Baseline: repeated across 3 files.
- Target: shared flow and smaller platform-specific modules.

## Rollout

- Rollout plan: merge in incremental PRs with parity checks.
- Rollback plan: revert the PR if backend parity breaks.
