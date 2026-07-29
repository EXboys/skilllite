# TASK Card

## Metadata

- Task ID: `TASK-2026-075`
- Title: Fix evolution skill entry_point path escape
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-29`
- Target milestone:

## Problem

During evolution skill synthesis, LLM-controlled `entry_point` (and generated skill `name`) are joined under `_evolved/_pending` without containment checks. Absolute `entry_point` values replace the skill directory via `Path::join`, and `../` components escape the pending skill tree before `skilllite_fs::write_file` writes script content. This allows arbitrary file create/overwrite as the SkillLite process user during `evolution run`.

## Scope

- In scope:
  - Validate generated skill names as single path segments before joining under `_pending`.
  - Validate `entry_point` as a relative path with only normal/curdir components (allow `scripts/main.py`) and resolve it under the skill directory.
  - Apply the same entry_point containment to generate/refine/repair write paths.
  - Add regression tests with concrete absolute/traversal triggers.
- Out of scope:
  - Pending confirm/reject `skill_name` hardening already covered by open PR #89.
  - Unrelated open critical PRs (#112–#116, #120–#126).
  - Symlink-following in agent file tools.

## Acceptance Criteria

- [x] Absolute `entry_point` (e.g. `/tmp/pwn.py`) is rejected before any write.
- [x] Traversal `entry_point` (e.g. `../../../tmp/pwn.py`) is rejected before any write.
- [x] Valid relative entry points like `scripts/main.py` still resolve under the skill dir.
- [x] Path-escaping generated skill names are skipped/rejected during generation.
- [x] Focused unit tests pass; `skilllite-evolution` tests pass.

## Risks

- Risk: Over-strict entry_point rules reject legitimate nested scripts.
  - Impact: Evolution skill generation/repair could skip valid skills.
  - Mitigation: Allow multi-segment relative paths with only `Normal`/`CurDir` components; keep seed examples (`scripts/main.py`) green.

## Validation Plan

- Required tests: unit tests for entry_point/name validators and generate-path containment.
- Commands to run:
  - `cargo test -p skilllite-evolution entry_point -- --nocapture`
  - `cargo test -p skilllite-evolution`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-evolution --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks: N/A (unit-level path safety).

## Regression Scope

- Areas likely affected: evolution skill generate/refine/repair script writes.
- Explicit non-goals: desktop confirm/reject UX; agent builtin file symlink policy.

## Links

- Source TODO section: critical bug automation cron
- Related PRs/issues: distinct from #89 (pending skill_name confirm/reject)
- Related docs: N/A (fail-closed validation only; no user-facing path grammar docs)
