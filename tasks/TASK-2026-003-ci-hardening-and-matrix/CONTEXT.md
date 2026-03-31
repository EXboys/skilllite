# Technical Context

## Current State

- Relevant files:
  - `.github/workflows/ci.yml`
  - `.github/workflows/release.yml`
- Current behavior:
  - PR checks run on Ubuntu; release builds already cover a wider matrix.

## Architecture Fit

- Layer boundaries involved:
  - no runtime architecture impact.
- Interfaces to preserve:
  - existing CI command contracts and release behavior.

## Dependency and Compatibility

- New dependencies:
  - optional `cargo-deny` tool in CI.
- Backward compatibility notes:
  - no user-facing runtime compatibility impact.

## Design Decisions

- Decision: stage CI hardening incrementally.
  - Rationale: reduce rollout risk and avoid contributor friction.
  - Alternatives considered: full strict checks immediately.
  - Why rejected: likely high initial noise and slower merges.

## Open Questions

- [ ] Which non-Ubuntu target should be mandatory first?
- [ ] Should `cargo deny` start as warning-only then become blocking?
