# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Critical bug sweep for recent commits
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-07-06`
- Target milestone:

## Problem

Run the scheduled high-severity bug sweep against recent commits and determine whether any review-escaped correctness bug warrants a minimal fix and PR.

## Scope

- In scope:
  - Recent commits on `main` / `cursor/critical-bug-investigation-4841`, especially behavioral changes with meaningful blast radius.
  - Concrete data loss, crashes, security bypasses, write/read path splits, infinite loops, resource leaks, or significant user-facing breakage.
  - Minimal fix plus targeted tests only if a high-confidence critical bug is confirmed.
- Out of scope:
  - Style, low-severity UX issues, theoretical risks without a plausible trigger, broad refactors, and speculative fixes.

## Acceptance Criteria

- [ ] Recent commits are inspected and high-blast-radius changes are traced through callers and downstream effects.
- [ ] Any surfaced issue has a concrete trigger scenario and severity rationale.
- [ ] If no critical issue is confirmed, no PR is opened and a concise Slack summary is sent.
- [ ] If a critical issue is confirmed, a minimal fix is committed, pushed, validated, and a PR is opened with bug/impact/root cause/fix/validation.

## Risks

- Risk: False positive bug report or unnecessary PR.
  - Impact: Reviewer time wasted and possible churn in sensitive paths.
  - Mitigation: Require a concrete trigger scenario and trace the full code path before editing.
- Risk: Missing a recent high-severity regression.
  - Impact: Data loss, crashes, or major user-visible breakage could remain.
  - Mitigation: Focus on behavioral diffs with write paths, workspace scoping, sandbox/security, and crash-prone code.

## Validation Plan

- Required tests: Targeted tests only if code changes are made; task validation for task artifacts.
- Commands to run:
  - `git log --oneline --decorate -n 20`
  - `git diff` / commit-specific diffs for candidate changes.
  - `python3 scripts/validate_tasks.py`
  - Additional targeted `cargo test` commands if a fix is implemented.
- Manual checks:
  - Trace caller chains and downstream effects for any candidate issue.
  - Verify Slack/PR output matches the final outcome.

## Regression Scope

- Areas likely affected: None unless a confirmed fix is implemented.
- Explicit non-goals: Refactors, docs-only cleanup, and low-severity findings.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-07-06.
- Related PRs/issues: Recent commits from `git log`.
- Related docs: `spec/verification-integrity.md`, `spec/task-artifact-language.md`.
