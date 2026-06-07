# TASK Card

## Metadata

- Task ID: `TASK-2026-068`
- Title: Critical Bug Audit
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-07`
- Target milestone:

## Problem

Inspect recent SkillLite commits for high-severity correctness regressions that escaped review. Only actionable findings with a concrete trigger scenario should result in a code change or PR.

## Scope

- In scope: Recent commits on `main` / current branch, with emphasis on behavioral changes affecting agent loops, CLI/runtime bridges, sandbox/security behavior, persistence, and UTF-8/error handling paths.
- Out of scope: Style-only issues, theoretical concerns without a plausible trigger, broad refactors, and low-severity UX degradation.

## Acceptance Criteria

- [x] Recent behavioral commits are reviewed beyond the diff by tracing relevant caller and downstream paths.
- [x] Any reported bug has a concrete trigger scenario and critical impact.
- [x] If a critical bug is fixed, the fix is minimal and covered by targeted validation.
- [ ] Validation commands pass and evidence is recorded.

## Risks

- Risk: Over-reporting speculative issues.
  - Impact: Unnecessary PR noise and reviewer distraction.
  - Mitigation: Require a concrete trigger scenario and critical user impact before opening a PR.
- Risk: Missing a regression outside the highest-risk recent changes.
  - Impact: Critical bug remains undetected.
  - Mitigation: Review commit metadata first, then inspect broad-blast-radius code paths.

## Validation Plan

- Required tests: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p skilllite-agent`, `cargo test`, and `python3 scripts/validate_tasks.py`.
- Commands to run: `git log`, targeted `git show` / `git diff`, targeted agent regression tests, full workspace validation.
- Manual checks: Trace caller/downstream paths and confirm whether each candidate has a plausible critical trigger.

## Regression Scope

- Areas likely affected: Agent behavior, command routing, sandbox execution, persistence/logging, UTF-8 truncation and user-facing error summaries.
- Explicit non-goals: Cosmetic cleanup, unrelated TODO execution, documentation rewrite.

## Links

- Source TODO section: N/A - scheduled critical bug-finding automation.
- Related PRs/issues: Recent commits on current branch and `main`.
- Related docs: `spec/verification-integrity.md`, `spec/README.md`.
