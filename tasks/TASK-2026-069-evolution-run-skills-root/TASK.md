# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Evolution run skills-root alignment
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-17`
- Target milestone:

## Problem

Recent L2 evolution bridge work made desktop pending/status/confirm paths resolve the effective workspace skills directory as `skills/` with legacy fallback to `.skills/`. The synchronous `skilllite evolution run` path still hardcodes `workspace/.skills`. In a workspace where `skills/` exists, evolution can generate pending skills under `.skills/_evolved/_pending` while desktop UI reads `skills/_evolved/_pending`, making generated skills invisible and unconfirmable.

## Scope

- In scope:
  - Align `skilllite evolution run` skills-root resolution with desktop pending/status/confirm paths.
  - Add focused regression coverage proving workspaces with `skills/` present use `skills/_evolved`.
  - Keep the legacy `.skills` fallback when `skills/` is absent.
- Out of scope:
  - Broad evolution run architecture changes.
  - Changing pending skill file formats or evolution DB schema.
  - Reworking assistant background process API key handling.

## Acceptance Criteria

- [ ] `evolution run` resolves the same effective skills root as pending/status/confirm for explicit workspaces.
- [ ] Regression tests cover `skills/` preference and `.skills` legacy fallback.
- [ ] Required Rust and task validation commands pass.

## Risks

- Risk: Changing the evolution run write root could surprise legacy workspaces.
  - Impact: Existing `.skills`-only workspaces must keep working.
  - Mitigation: Use the existing `resolve_skills_dir_with_legacy_fallback` helper so `.skills` remains selected only when `skills/` is absent.
- Risk: Over-widening path semantics.
  - Impact: Could change unrelated skill discovery behavior.
  - Mitigation: Limit the fix to the command-layer skills root used by `evolution run`.

## Validation Plan

- Required tests:
  - Unit regression tests in `skilllite-commands`.
  - CLI/commands behavior tests required by policy.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-commands --features agent`
  - `cargo test -p skilllite`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Inspect the changed files after edits.

## Regression Scope

- Areas likely affected:
  - `skilllite evolution run`
  - Desktop evolution manual trigger and post-authorize background run
  - Pending evolved skill visibility and confirmation
- Explicit non-goals:
  - DB workspace default semantics when `--workspace` is omitted.
  - Tauri UI copy or frontend state management.

## Links

- Source TODO section: daily critical bug-finding automation, 2026-06-17.
- Related PRs/issues:
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
