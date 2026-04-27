# TASK Card

## Metadata

- Task ID: `TASK-2026-003`
- Title: CI hardening (optional deny/dependabot and PR multi-OS matrix)
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors: `airlu`
- Created: `2026-03-31`
- Target milestone: `v0.1.x maintenance`

## Problem

PR CI was Ubuntu-only for Rust checks. Optional but high-value hardening items (`cargo deny`, Dependabot, PR multi-OS checks) needed to be completed and reflected in task status.

## Scope

- In scope:
  - Keep the existing `cargo deny` policy check enforced in CI.
  - Add Dependabot config for Rust/Python/GitHub Actions updates.
  - Add PR matrix coverage for macOS/Windows smoke checks.
  - Enforce Rust clippy warnings as CI failures.
- Out of scope:
  - Full release pipeline redesign.

## Acceptance Criteria

- [x] `cargo deny` check configured (or documented deferral with reason).
- [x] Dependabot configuration committed.
- [x] PR workflow covers at least one non-Ubuntu target for regression prevention.
- [x] `cargo clippy --all-targets` fails on warnings.

## Risks

- Risk: increased CI duration/noise.
  - Impact: slower iteration and flaky checks.
  - Mitigation: phased rollout and scoped checks.

## Validation Plan

- Required checks:
  - CI workflow dry run in PR.
  - Verify no false-positive policy blocks.
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo check -p skilllite --bin skilllite --no-default-features --features sandbox_binary`
  - `cargo check -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary`

## Regression Scope

- Areas likely affected:
  - `.github/workflows/ci.yml`
  - dependency management configuration files
  - contributor CI documentation
- Explicit non-goals:
  - No change to runtime product behavior.

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (`0.3`, `0.2`)
- Related PRs/issues: `TBD`
- Related docs: `docs/en/CONTRIBUTING.md`, `docs/zh/CONTRIBUTING.md`
