# TASK Card

## Metadata

- Task ID: `TASK-2026-067`
- Title: Align evolution skills root
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-05-30`
- Target milestone:

## Problem

Recent desktop evolution paths list, count, confirm, and reject pending skills through the current
default `skills/` directory with legacy fallback to `.skills/`. The `evolution run` path still writes
generated pending skills to `.skills` unconditionally, so default `skilllite init` workspaces that use
`skills/` can generate pending skills that the desktop cannot see or confirm.

## Scope

- In scope:
  - Align `evolution run` and related evolution skill operations with the shared `skills/` default plus `.skills` legacy fallback.
  - Add focused regression tests for default and legacy root resolution.
  - Validate Rust formatting, linting, tests, and task artifact shape.
- Out of scope:
  - Broad redesign of skill identity, duplicate skill handling, or desktop error presentation.
  - Changing LLM evolution policy, backlog schema, or sandbox behavior.

## Acceptance Criteria

- [ ] `evolution run` resolves the same effective skills root as desktop pending/status/confirm for default workspaces.
- [ ] Existing `.skills`-only workspaces remain supported.
- [ ] Regression tests cover default `skills/` and legacy `.skills` behavior.
- [ ] Required validation commands pass or any blocker is recorded with output.

## Risks

- Risk: Changing the write target may affect users with both `skills/` and `.skills/`.
  - Impact: New evolved skills follow `skills/`, while older pending skills in `.skills` remain in the legacy tree.
  - Mitigation: Match the already-shipped shared fallback behavior: prefer `skills/` when present and fall back only when it is absent.

## Validation Plan

- Required tests:
  - Regression tests in `crates/skilllite-commands/src/evolution.rs`.
  - Workspace Rust tests required by policy.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read modified files and task board after updates.

## Regression Scope

- Areas likely affected:
  - CLI evolution skill generation, pending/confirm/reject, and repair skill root selection.
  - Desktop evolution UI visibility of generated pending skills.
- Explicit non-goals:
  - No change to chat skill loading, skill import, or assistant frontend components.

## Links

- Source TODO section: N/A
- Related PRs/issues: Recent `feat/runtime-skills-l2-json` and UTF-8 logging follow-up.
- Related docs: `spec/docs-sync.md` reviewed; no docs change needed because the fix restores documented/current default behavior.
