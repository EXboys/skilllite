# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Fix desktop workspace root mismatches
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-09`
- Target milestone:

## Problem

Recent workspace-scope fixes made desktop chat subprocess writes use the active UI workspace, but several Tauri read/write helpers still resolve chat data from the process-global root when `SKILLLITE_WORKSPACE` is unset. The same mismatch exists in Life Pulse rhythm: due detection checks the active workspace, but `schedule tick` runs without an explicit workspace. Users can see missing chat history, edit prompt files in the wrong root, or run scheduled agent jobs against the wrong project.

## Scope

- In scope:
  - Route desktop chat/session/transcript/memory/log/prompt artifact helpers through the active UI workspace chat root.
  - Pass the active workspace from frontend calls that read or mutate chat-scoped data.
  - Pass `--workspace` and set `current_dir` for Life Pulse rhythm subprocesses.
  - Add focused regression tests for workspace chat root resolution and rhythm command arguments.
- Out of scope:
  - CLI-only evolution maintenance commands such as `reset`, `disable`, `explain`, and `repair-skills`.
  - Broad desktop state-store redesign or multi-workspace session UX changes.

## Acceptance Criteria

- [x] Desktop transcript/session/memory/log/prompt reads use the same `<workspace>/chat` root as chat subprocess writes when a workspace is supplied.
- [x] Desktop session create/rename/delete mutates the active workspace's `sessions.json` and related transcript/plan files.
- [x] Life Pulse rhythm executes `skilllite schedule tick --workspace <workspace>` from the resolved project root.
- [x] Existing no-workspace call paths remain backward compatible with the process-global chat root.
- [x] Regression tests cover the fixed routing behavior.

## Risks

- Risk: Tauri command signature changes could miss a frontend caller.
  - Impact: A panel could continue reading global chat data.
  - Mitigation: Search all invoke callsites and preserve backend defaults for omitted workspace values.
- Risk: Changing rhythm cwd/args could affect relative workspace handling.
  - Impact: Scheduled jobs could fail to start for unusual workspace strings.
  - Mitigation: Reuse the same `find_project_root` contract already used by growth and test generated args.

## Validation Plan

- Required tests:
  - Focused Rust unit tests for `skilllite-assistant` workspace routing.
  - Task artifact validation.
- Commands to run:
  - `cargo fmt --check`
  - `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `npm run build` in `crates/skilllite-assistant`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Inspect edited callsites to confirm every changed Tauri invoke passes `workspace` or uses a backend compatibility default.

## Regression Scope

- Areas likely affected:
  - Desktop assistant chat reload, sessions sidebar, status/detail memory and logs, evolution prompt diffs/manual edits, Life Pulse schedule execution.
- Explicit non-goals:
  - Non-desktop CLI data root policy.
  - Sandbox policy or LLM routing behavior.

## Links

- Source TODO section: N/A - daily critical bug investigation.
- Related PRs/issues: Recent workspace root fixes around TASK-2026-068 and TASK-2026-069.
- Related docs: N/A - this restores intended workspace-scoped behavior and does not add commands, flags, or environment variables.
