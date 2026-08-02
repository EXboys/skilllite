# TASK Card

## Metadata

- Task ID: `TASK-2026-078`
- Title: Reject path-escaping runtime skill entry points
- Status: `done`
- Priority: `P0`
- Owner: `automation`
- Contributors:
- Created: `2026-08-02`
- Target milestone:

## Problem

`skilllite run` / agent skill execution accept `entry_point` from SKILL.md front matter (and LLM `entry_point_override`) when `skill_dir.join(entry_point).is_file()` is true, without requiring the resolved path to stay under the skill directory.

On POSIX, `Path::join` replaces the root for absolute paths, and relative `../` components escape the skill tree. At sandbox level 1 (`execute_unsandboxed`), the interpreter is invoked with that escaped path and `cwd=skill_dir`, so attacker-controlled skill metadata can execute arbitrary host scripts.

`exec_script` already contains the script path via canonicalize + `starts_with(skill_dir)`; `run_skill` does not.

## Scope

- In scope:
  - Shared lexical containment helper for skill-relative entry points
  - Metadata front-matter acceptance
  - `run_skill` override + pre-execution gate
  - Agent skill executor override path
  - Regression tests + PoC verification
- Out of scope:
  - Evolution synthesis write-path hardening (open PR #127)
  - OpenClaw import dest name (separate finding)
  - Unconstrained bash `--cwd` (separate finding)
  - Builtin tool symlink following (separate finding)

## Acceptance Criteria

- [x] Absolute entry points (e.g. `/tmp/pwn.py`) are rejected for runtime run/override
- [x] Traversal entry points (e.g. `../../../tmp/pwn.py`) are rejected
- [x] Windows drive/backslash forms are rejected host-independently
- [x] Legitimate nested relatives like `scripts/main.py` still work
- [x] Confirmed PoC no longer prints `OUTSIDE_EXECUTED` at level 1
- [x] Focused unit/integration tests pass

## Risks

- Risk: Skills that intentionally used absolute entry points break
  - Impact: Fail-closed execution error instead of running outside script
  - Mitigation: Absolute entry points were never a supported/safe contract; `exec_script` already rejects escapes

## Validation Plan

- Required tests:
  - `path_validation` unit tests for accept/reject matrix
  - Metadata parse rejects escaping front-matter entry_point (falls back or leaves empty)
  - Runtime PoC with absolute/relative escape at `SKILLLITE_SANDBOX_LEVEL=1`
- Commands to run:
  - `cargo test -p skilllite-core path_validation`
  - `cargo test -p skilllite-core metadata`
  - `cargo test -p skilllite-commands`
  - `cargo clippy -p skilllite-core -p skilllite-commands -p skilllite-agent --all-targets -- -D warnings`
  - Manual PoC with `skilllite run`
- Manual checks:
  - Relative escape skill no longer executes outside script

## Regression Scope

- Areas likely affected:
  - Skill metadata parsing
  - `skilllite run` / stdio RPC run
  - Agent skill tool execution entry override
- Explicit non-goals:
  - Pending confirm/reject skill_name (#89)
  - Memory/session/artifact path PRs (#123–#129)

## Links

- Source TODO section: critical-bug automation cron
- Related PRs/issues: complements #127 (synthesis writes) with runtime execution gate
- Related docs: N/A (fail-closed validation; no documented absolute entry_point support)
