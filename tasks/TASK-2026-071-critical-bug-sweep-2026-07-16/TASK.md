# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Critical bug sweep for recent commits
- Status: `done`
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

- [x] Recent commits are inspected and high-blast-radius changes are traced through callers and downstream effects.
- [x] Any surfaced issue has a concrete trigger scenario and severity rationale. No new issue was surfaced.
- [x] If no critical issue is confirmed, no fix PR is opened and a concise Slack summary is attempted.
- [x] If a critical issue is confirmed, a minimal fix is committed, pushed, validated, and a PR is opened. N/A: no critical issue was confirmed.

## Risks

- Risk: False-positive bug report or unnecessary fix.
  - Impact: Reviewer time is wasted and sensitive paths may churn.
  - Mitigation: Require a concrete trigger scenario and trace the full code path before editing.
- Risk: Missing a severe regression in a recent behavioral change.
  - Impact: Data loss, crashes, or significant breakage may remain.
  - Mitigation: Prioritize behavioral diffs in security, persistence, process, and workspace-scoping paths.

## Validation Plan

- Required tests: Investigation commands, task validation, and the existing targeted authorization argument regression test.
- Commands to run:
  - `git log` and commit-specific `git show` / `git diff`
  - `python3 scripts/validate_tasks.py`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml authorized_run_args_include_target_workspace_and_proposal`
- Manual checks:
  - Trace changed behavior through callers, persisted state, and downstream consumers.
  - Verify Slack/PR output matches the final outcome.

## Regression Scope

- Areas likely affected: Reviewed desktop evolution authorization changes and adjacent evolution command behavior; no additional runtime change was made.
- Explicit non-goals: Refactors, unrelated cleanup, and low-severity findings.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-07-16.
- Related PRs/issues: Reviewed PR #111 and compared duplicate open PR #117; no fix PR opened for this sweep.
- Related docs: `spec/verification-integrity.md`, `spec/task-artifact-language.md`.
