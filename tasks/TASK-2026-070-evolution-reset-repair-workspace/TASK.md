# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Fix evolution reset and repair workspace roots
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-08`
- Target milestone: critical bugfix

## Problem

Recent workspace-scoping fixes aligned `evolution run`, status, backlog, pending, confirm, and reject with project-local `chat/` and `skills/` roots. Two maintenance commands still use legacy resolution:

- `evolution reset --force` uses `paths::chat_root()` and deletes only `.skills/_evolved`.
- `evolution repair-skills` validates only `.skills`.

In a modern workspace that uses `skills/`, this can reset the wrong chat store, leave evolved skills behind after a destructive reset, or report successful repair while broken evolved skills remain unvalidated.

## Scope

- In scope:
  - Add workspace-aware root resolution for `evolution reset`.
  - Align `evolution repair-skills` with the same effective `skills/` then `.skills/` fallback used by evolution run/pending/confirm.
  - Add regression coverage for modern `skills/` workspaces.
- Out of scope:
  - Changing LLM repair logic.
  - Refactoring unrelated legacy commands (`disable`, `explain`) unless required by tests.
  - Changing sandbox or skill execution policy.

## Acceptance Criteria

- [ ] `skilllite evolution reset --force --workspace <ws>` resets `<ws>/chat` and removes `<ws>/skills/_evolved`.
- [ ] `skilllite evolution reset --force` run from a workspace defaults to that workspace instead of mixing global chat with project skills.
- [ ] `skilllite evolution repair-skills --workspace <ws>` validates the effective skills root, preferring `<ws>/skills` with legacy `.skills` fallback.
- [ ] Focused regression tests cover the destructive reset path without requiring an LLM API key.
- [ ] Required Rust formatting, linting, tests, and task validation are run and recorded.

## Risks

- Risk: Changing the default reset target from legacy global data to the current workspace can surprise users who intentionally reset `~/.skilllite`.
  - Impact: A user may need to pass an explicit workspace for global data.
  - Mitigation: Match the workspace default already used by adjacent evolution subcommands and preserve explicit `--workspace` control.
- Risk: Repair now scans a different tree in workspaces that have both `skills/` and `.skills/`.
  - Impact: Duplicate skills can remain ambiguous.
  - Mitigation: Reuse the existing `resolve_skills_dir_with_legacy_fallback` behavior rather than introducing a new resolver.

## Validation Plan

- Required tests:
  - Focused integration tests in `skilllite/tests/cli_evolution_workspace.rs`.
  - Required command-scope tests from `spec/testing-policy.md`.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite --test cli_evolution_workspace`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read modified files and task board after updates.

## Regression Scope

- Areas likely affected:
  - CLI parsing for `skilllite evolution reset` and `repair-skills`.
  - Desktop bridge repair command arguments.
  - Evolution workspace root selection.
- Explicit non-goals:
  - No changes to skill synthesis prompts or sandbox enforcement.
  - No new dependencies.

## Links

- Source TODO section: N/A (daily critical bug investigation).
- Related PRs/issues: PR #95, PR #101 workspace-scope fixes.
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`.
