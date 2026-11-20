# TASK Card

## Metadata

- Task ID: `TASK-2026-079`
- Title: Judge sandbox memory limits against the current child
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-20`
- Target milestone:

## Problem

`wait_with_timeout` rejects a finished child when `getrusage(RUSAGE_CHILDREN).ru_maxrss` is above the limit. That field is the high-water mark of every child the process has already waited for, not the child that just exited. After any earlier child peaks above the limit, a later successful skill or bash command is reported as over the memory limit and its stdout is discarded.

## Scope

- In scope:
  - Compare the post-exit high-water mark with the mark from before this child was reaped.
  - Keep rejecting a child that itself raises the mark above its limit.
  - Regression tests for both outcomes.
- Out of scope:
  - Replacing `RUSAGE_CHILDREN` with a per-pid peak for grandchildren inside bwrap.
  - Changing the default 256MB limit or the live RSS poll interval.

## Acceptance Criteria

- [x] A command that exits successfully still returns its stdout when an earlier child in the same process peaked above this command's limit.
- [x] A child that raises the high-water mark above its own limit is still rejected and its stdout is not returned.
- [x] EN and ZH docs state that the limit applies to the current run, not to earlier exited children.

## Risks

- Risk: a fast child can exceed the limit without detection when an earlier child already peaked higher and this child exits before the RSS poll.
  - Impact: memory-limit enforcement has a narrow miss; it does not open the filesystem or network sandbox.
  - Mitigation: the existing RSS poll still kills a process that stays over the limit. A child that sets a new high-water mark above the limit is still rejected after exit.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-sandbox later_command_keeps_stdout_when_an_earlier_child_peaked_higher`
  - `cargo test -p skilllite-sandbox child_that_raises_the_rss_peak_above_the_limit_is_rejected`
  - `cargo test -p skilllite-sandbox`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
- Commands to run:
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm the failing test rejects stdout before the fix and keeps it after the fix.

## Regression Scope

- Areas likely affected:
  - Skill execution, bash execution, and any other caller of `wait_with_timeout`.
- Explicit non-goals:
  - Agent-sandbox nftables, seccomp fd passing, and Codex auth writeback.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
