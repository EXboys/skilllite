# TASK Card

## Metadata

- Task ID: `TASK-2026-003`
- Title: CI hardening (optional deny/dependabot and PR multi-OS matrix)
- Status: `ready`
- Priority: `P1`
- Owner: `TBD`
- Contributors: `TBD`
- Created: `2026-03-31`
- Target milestone: `v0.1.x maintenance`

## Problem

Current PR CI is Ubuntu-only. Optional but high-value hardening items (`cargo deny`, Dependabot, PR multi-OS checks) are pending.

## Scope

- In scope:
  - Evaluate and add `cargo deny` policy check.
  - Add Dependabot config for Rust/Python/GitHub Actions updates.
  - Add PR matrix coverage for macOS/Windows where practical.
- Out of scope:
  - Full release pipeline redesign.

## Acceptance Criteria

- [ ] `cargo deny` check configured (or documented deferral with reason).
- [ ] Dependabot configuration committed.
- [ ] PR workflow covers at least one non-Ubuntu target for regression prevention.

## Risks

- Risk: increased CI duration/noise.
  - Impact: slower iteration and flaky checks.
  - Mitigation: phased rollout and scoped checks.

## Validation Plan

- Required checks:
  - CI workflow dry run in PR.
  - Verify no false-positive policy blocks.
- Commands to run:
  - `cargo deny check` (if enabled)
  - `cargo audit`

## Regression Scope

- Areas likely affected:
  - `.github/workflows/ci.yml`
  - dependency management configuration files
- Explicit non-goals:
  - No change to runtime product behavior.

## Links

- Source TODO section: `todo/06-OPTIMIZATION.md` (`0.3`, `0.2`)
- Related PRs/issues: `TBD`
- Related docs: `docs/en/CONTRIBUTING.md`, `docs/zh/CONTRIBUTING.md`
