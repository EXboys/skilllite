# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Fix critical evolution run correctness bugs
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-12`
- Target milestone:

## Problem

Recent evolution changes left critical correctness gaps in user-facing paths:
human `skilllite evolution status` can panic when displaying multibyte event
reasons, and evolution skill generation can target a different skills root or
workspace than desktop status/listing paths.

## Scope

- In scope:
  - Make human evolution status reason previews UTF-8 boundary safe.
  - Align `evolution run` skill root resolution with desktop pending-skill paths.
  - Ensure Life Pulse growth subprocesses pass the selected workspace to `evolution run`.
  - Add focused regression tests where feasible.
- Out of scope:
  - Broad evolution architecture refactors.
  - Changing CLI flags or user-facing command contracts.
  - Fixing pre-existing reset/disable/explain workspace inconsistencies.

## Acceptance Criteria

- [ ] `skilllite evolution status` no longer panics on long CJK/emoji event reasons.
- [ ] `evolution run` writes evolved skills to the same effective `skills/` root that desktop pending-skill list/confirm uses.
- [ ] Life Pulse growth execution scopes the child `evolution run` to the selected workspace.
- [ ] Regression tests cover UTF-8 preview safety and skills-root alignment.

## Risks

- Risk: changing path resolution for evolved skills could affect legacy `.skills` workspaces.
  - Impact: evolved skill output may move if fallback behavior is wrong.
  - Mitigation: use existing `resolve_skills_dir_with_legacy_fallback` helper and test both primary and legacy cases.

## Validation Plan

- Required tests:
  - Focused Rust tests for `skilllite-commands`.
  - Task artifact validation.
  - Workspace Rust format, lint, and test baseline as feasible.
- Commands to run:
  - `cargo test -p skilllite-commands`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read modified files.
  - Inspect `tasks/board.md` after final update.

## Regression Scope

- Areas likely affected:
  - Evolution CLI status rendering.
  - Evolution run skill output paths.
  - Desktop Life Pulse growth subprocess invocation.
- Explicit non-goals:
  - API/provider credential behavior.
  - Sandbox policy behavior.
  - Python SDK behavior.

## Links

- Source TODO section: N/A, daily critical bug investigation.
- Related PRs/issues: recent evolution workspace scoping and UTF-8 truncation fixes.
- Related docs: `spec/verification-integrity.md`, `spec/rust-conventions.md`, `spec/testing-policy.md`
