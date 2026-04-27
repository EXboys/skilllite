# Technical Context

## Current State

- Relevant files:
  - `.github/workflows/ci.yml`
  - `.github/dependabot.yml`
  - `.github/workflows/release.yml`
  - `docs/en/CONTRIBUTING.md`
  - `docs/zh/CONTRIBUTING.md`
- Current behavior:
  - PR checks run full Rust/Python gates on Ubuntu.
  - PR checks run lightweight cargo-check smoke on macOS and Windows.
  - Release builds already cover a wider artifact matrix.

## Architecture Fit

- Layer boundaries involved:
  - no runtime architecture impact.
- Interfaces to preserve:
  - existing CI command contracts and release behavior.

## Dependency and Compatibility

- New dependencies:
  - no new runtime dependencies.
  - CI continues installing `cargo-deny`; Dependabot now tracks Cargo, npm, pip, and GitHub Actions ecosystems.
- Backward compatibility notes:
  - no user-facing runtime compatibility impact.

## Design Decisions

- Decision: stage CI hardening incrementally.
  - Rationale: reduce rollout risk and avoid contributor friction.
  - Alternatives considered: full strict checks immediately.
  - Why rejected: likely high initial noise and slower merges.
- Decision: use macOS and Windows `cargo check` smoke instead of full test suites.
  - Rationale: catch platform compile regressions while keeping PR latency bounded.
  - Alternatives considered: full `cargo test` on every OS.
  - Why rejected: higher CI cost and more noise for this maintenance task.

## Open Questions

- [x] Which non-Ubuntu target should be mandatory first? Answer: macOS and Windows lightweight smoke checks.
- [x] Should `cargo deny` start as warning-only then become blocking? Answer: blocking for `bans`, matching existing CI.
