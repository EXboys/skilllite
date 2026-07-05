# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Scope destructive evolution commands to workspace
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-07-05`
- Target milestone:

## Problem

Recent workspace scoping fixes aligned most `skilllite evolution` commands with the explicit
`--workspace` target, but `reset`, `disable`, and `explain` still resolve prompt state through
process-global `SKILLLITE_WORKSPACE` / home defaults. A user running `skilllite evolution reset
--force` from a project directory with no `SKILLLITE_WORKSPACE` can delete project evolved skills
while reseeding/removing prompt and log state under `~/.skilllite/chat`, leaving the intended
project chat state untouched and destroying unrelated global evolution state.

## Scope

- In scope:
  - Add a workspace argument to `evolution reset`, `disable`, and `explain`.
  - Route these commands through the same workspace chat root used by status/backlog/run.
  - Preserve the existing legacy `.skills` fallback for evolved skill deletion.
  - Add regression coverage for reset using workspace over process environment/home defaults.
  - Update EN/ZH command documentation for the added workspace flag.
- Out of scope:
  - Changing evolution force/governance policy.
  - Changing desktop UI flows that do not expose reset/disable/explain today.
  - Redesigning nested-workspace project-root discovery.

## Acceptance Criteria

- [x] `skilllite evolution reset --force --workspace W` only resets `W/chat` and `W` skills roots, even when `SKILLLITE_WORKSPACE` points elsewhere.
- [x] `disable` and `explain` can target the same workspace root as other evolution commands via `--workspace`.
- [x] Regression tests cover the destructive reset workspace mismatch.
- [x] EN/ZH docs mention the workspace-scoped reset/disable/explain surface.
- [x] Required Rust and task validation commands pass or any pre-existing blockers are recorded.

## Risks

- Risk: Existing users may rely on process-global `SKILLLITE_WORKSPACE` for these commands.
  - Impact: A command run outside a project without flags still targets the current directory by default, matching other evolution commands rather than the old home default.
  - Mitigation: Keep `--workspace` default as `.` and document the flag; explicit `SKILLLITE_WORKSPACE` users can pass `--workspace "$SKILLLITE_WORKSPACE"`.
- Risk: Reset touches destructive filesystem paths.
  - Impact: A bad path resolver could remove the wrong `_evolved` tree.
  - Mitigation: Reuse existing workspace-root and skills fallback helpers; add a regression test with env and target workspaces.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-commands --features agent` — passed, 42 tests.
  - `cargo test -p skilllite` — passed.
  - `cargo test` — passed.
  - `python3 scripts/validate_tasks.py` — passed, 70 task directories checked.
- Commands to run:
  - `cargo fmt --check` — passed.
  - `cargo clippy --all-targets -- -D warnings` — passed.
- Manual checks:
  - Inspected CLI dispatch to confirm reset/disable/explain pass workspace through.
  - Re-read changed files and task board after edits.

## Regression Scope

- Areas likely affected:
  - `skilllite evolution reset`
  - `skilllite evolution disable`
  - `skilllite evolution explain`
  - Evolution CLI help/docs
- Explicit non-goals:
  - Sandbox behavior
  - LLM provider/runtime configuration
  - Desktop L2 JSON command contracts

## Links

- Source TODO section: N/A
- Related PRs/issues: PR #101 (`TASK-2026-069`) explicitly left `reset/disable/explain` workspace semantics out of scope.
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
