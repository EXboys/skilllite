# PRD

## Background

CI quality improved substantially, but risk coverage still needed automated dependency update cadence and PR checks beyond Ubuntu.

## Objective

Increase CI confidence without overloading contributor workflow.

## Functional Requirements

- FR-1: Keep reproducible dependency policy checks enforced in PR CI.
- FR-2: Add automated dependency update cadence.
- FR-3: Extend PR CI beyond Ubuntu for key regression paths.
- FR-4: Treat clippy warnings as CI failures.

## Non-Functional Requirements

- Security: preserve existing audit guarantees.
- Performance: keep CI duration reasonable.
- Compatibility: no changes to release artifact naming contracts.

## Constraints

- Technical: keep workflows maintainable and understandable.
- Timeline: incremental additions with rollback options.

## Success Metrics

- Metric: CI gate coverage.
- Baseline: Ubuntu-only PR checks.
- Target: broadened policy and platform confidence.

## Rollout

- Rollout plan: add blocking Dependabot metadata, strict clippy, and lightweight macOS/Windows cargo-check smoke in PR CI.
- Rollback plan: temporarily narrow the platform smoke command set if a platform-specific toolchain issue is noisy.
