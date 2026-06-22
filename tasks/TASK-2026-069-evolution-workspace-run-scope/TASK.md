# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Evolution workspace run scope
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-19`
- Target milestone:

## Problem

Recent desktop L2 evolution paths and agent-rpc chat can use different workspace/skills roots than the ones used for status, backlog, authorization, and pending-skill review. A user can chat, authorize, or automatically trigger evolution for workspace `W`, but decisions or generated work may land in the default chat DB, process cwd, or `.skills`, leaving generated work in the wrong database or an invisible pending-skill directory.

## Scope

- In scope:
  - Align `evolution run` skill output with the same workspace skill-root fallback used by pending/confirm/status.
  - Ensure desktop `agent-rpc` child processes receive `SKILLLITE_WORKSPACE` matching the resolved UI workspace.
  - Ensure assistant background authorization follow-up invokes `evolution run` with the same `--workspace` used for enqueue.
  - Ensure Life Pulse growth invokes `evolution run` with the active workspace.
  - Align agent in-process A9 skill evolution with the same effective skill root.
  - Add focused regression tests for argument/root selection where feasible without requiring an LLM call.
- Out of scope:
  - Broad evolution governance changes or redesigning force/manual-trigger policy.
  - Changing SQLite schema.
  - Fixing unrelated UI error-suppression paths unless required by the workspace-scope fix.

## Acceptance Criteria

- [x] `cmd_run` writes and reports evolved skills under the effective `skills/` root when a workspace has `skills/`, preserving legacy `.skills` fallback when appropriate.
- [x] Desktop chat decisions are written under the same workspace chat DB that L2 evolution UI reads.
- [x] Background desktop authorization follow-up cannot lose the target proposal because it omits `--workspace`.
- [x] Life Pulse growth cannot run against process cwd when an active workspace is known.
- [x] Regression tests cover the fixed path/argument behavior.
- [x] Required verification commands pass or any environment blockers are recorded with evidence.

## Risks

- Risk: Changing skill-root resolution could affect legacy projects that only have `.skills`.
  - Impact: Existing evolved skills might appear missing if fallback is wrong.
  - Mitigation: Use existing `resolve_skills_dir_with_legacy_fallback` helper and add tests for both `skills` and `.skills`.
- Risk: Desktop subprocess helpers can be hard to test end-to-end.
  - Impact: Argument regressions might slip through if only manually inspected.
  - Mitigation: Extract small pure helper functions for run args where possible and test them directly.

## Validation Plan

- Required tests:
  - Unit tests for evolution skill-root resolution.
  - Unit tests for chat child environment workspace override.
  - Unit tests for assistant background run argument construction.
  - Existing CLI/commands tests covering workspace evolution paths.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-commands`
  - `cargo test -p skilllite`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Inspect changed call sites to confirm all affected subprocess invocations pass `--workspace`.

## Regression Scope

- Areas likely affected:
  - `skilllite evolution run`
  - Desktop `agent-rpc` chat subprocess environment
  - Agent in-process A9 evolution
  - Desktop authorize-capability follow-up run
  - Life Pulse automatic growth run
  - Pending evolved skill list/confirm/reject
- Explicit non-goals:
  - `evolution reset/disable/explain` workspace semantics.
  - LLM model/provider configuration UX changes.

## Links

- Source TODO section: cron critical bug investigation prompt.
- Related PRs/issues: Recent evolution workspace scoping fix `TASK-2026-068`.
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
