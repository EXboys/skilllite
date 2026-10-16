# PRD

## Background

The macOS sandbox profile claimed that `(allow default)` does not override earlier explicit denies. Seatbelt evaluates last-match-wins, so a trailing `(allow default)` is the decision for network, file writes, process execution, and IPC.

## Objective

A default macOS Level 2/3 skill cannot use the trailing allow to escape network, filesystem, or process restrictions.

## Functional Requirements

- FR-1: `(allow default)` is emitted once, before any security deny.
- FR-2: Non-interpreter `process-exec` matches only `(deny process-exec)`.
- FR-3: Domain-filtered networking matches `(deny network*)` except the local proxy ports.
- FR-4: Writes to mandatory-deny paths (shell rc, git hooks, IDE config) stay denied even inside the project write allow.

## Non-Functional Requirements

- Security: Default level semantics stay L1 off, L2 isolation, L3 isolation plus scan. This change makes L2/L3 isolation actually apply on macOS.
- Performance: Profile generation only reorders rules.
- Compatibility: No new environment variables. Skills that relied on the bypass will see Seatbelt denials.

## Constraints

- Technical: Profile string order is the enforcement mechanism. Linux CI cannot run `sandbox-exec`.
- Timeline: Same-day bugfix.

## Success Metrics

- Metric: Rule order in the generated profile.
- Baseline: `(allow default)` was the last matching rule for denied operations.
- Target: Unit tests fail if `(allow default)` moves back after a deny.

## Rollout

- Rollout plan: Ship with the next engine build. Desktop bundles that engine.
- Rollback plan: Revert the profile-order commit.
