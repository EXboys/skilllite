# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Critical bug sweep for recent commits
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-07-16`
- Target milestone:

## Problem

Run the scheduled high-severity bug sweep against commits merged since the previous sweep and determine whether any review-escaped correctness bug warrants a minimal fix.

## Scope

- In scope:
  - Commits added to `main` since the previous completed sweep.
  - Concrete data loss, crashes, security bypasses, write loss, infinite loops, resource leaks, or significant user-facing breakage.
  - Minimal fix and regression coverage only if a high-confidence critical bug is confirmed.
- Out of scope:
  - Style, low-severity UX issues, theoretical risks without a plausible trigger, broad refactors, and speculative fixes.

## Acceptance Criteria

- [ ] Recent commits are inspected and high-blast-radius changes are traced through callers and downstream effects.
- [ ] Any surfaced issue has a concrete trigger scenario and severity rationale.
- [ ] If no critical issue is confirmed, no fix PR is opened and a concise Slack summary is attempted.
- [ ] If a critical issue is confirmed, a minimal fix is committed, pushed, validated, and a PR is opened.

## Risks

- Risk: False-positive bug report or unnecessary fix.
  - Impact: Reviewer time is wasted and sensitive paths may churn.
  - Mitigation: Require a concrete trigger scenario and trace the full code path before editing.
- Risk: Missing a severe regression in a recent behavioral change.
  - Impact: Data loss, crashes, or significant breakage may remain.
  - Mitigation: Prioritize behavioral diffs in security, persistence, process, and workspace-scoping paths.

## Validation Plan

- Required tests: Investigation commands and task validation; code-change baseline plus targeted regression tests only if a fix is implemented.
- Commands to run:
  - `git log` and commit-specific `git show` / `git diff`
  - `python3 scripts/validate_tasks.py`
  - If code changes: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`
- Manual checks:
  - Trace changed behavior through callers, persisted state, and downstream consumers.
  - Verify Slack/PR output matches the final outcome.

## Regression Scope

- Areas likely affected: Recent desktop evolution authorization changes and adjacent evolution command behavior.
- Explicit non-goals: Refactors, unrelated cleanup, and low-severity findings.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-07-16.
- Related PRs/issues: Recent commits on `origin/main`, including PR #111.
- Related docs: `spec/verification-integrity.md`, `spec/task-artifact-language.md`.
