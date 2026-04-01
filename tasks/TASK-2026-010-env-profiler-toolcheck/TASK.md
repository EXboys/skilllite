# TASK Card

## Metadata

- Task ID: `TASK-2026-010`
- Title: Add safe env profiler tool checks for planning
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors: `Cursor agent`
- Created: `2026-04-01`
- Target milestone:

## Problem

Planning may assume required tools are available, but execution fails later when key local tools (for example `git` or `python`) are missing.

## Scope

- In scope:
  - Add a low-risk environment profiler that checks `git/python/node/npm/cargo`.
  - Collect tool availability and `--version` output using read-only commands only.
  - Inject environment profile summary into planning input.
  - Add unit tests for profile formatting and deterministic coverage.
- Out of scope:
  - No privileged checks (`sudo`, admin calls, deep system scan).
  - No network probing or background monitoring.

## Acceptance Criteria

- [x] Planning prompt includes environment profile summary.
- [x] Tool checks cover `git/python/node/npm/cargo` availability + version.
- [x] Implementation avoids sensitive operations and uses only safe read-only probes.

## Risks

- Risk: Environment checks could be interpreted as intrusive by some endpoints.
  - Impact: Reduced user trust or endpoint alerts.
  - Mitigation: Restrict to fixed allowlist of read-only `--version` probes, no elevated permissions, no deep scans.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - workspace baseline (`fmt`, `clippy`, `test`)
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test`
- Manual checks:
  - Confirm planning input includes the environment profile block and lists missing tools when unavailable.

## Regression Scope

- Areas likely affected:
  - `skilllite-agent` planning input assembly path.
- Explicit non-goals:
  - No changes to sandbox policy model or execution permissions.

## Links

- Source TODO section: `todo/12-SELF-EVOLVING-ENGINE.md` section 15.2 and 15.4 (`env_profiler`)
- Related PRs/issues:
- Related docs:
  - `spec/task-artifact-language.md`
