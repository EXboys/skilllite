# TASK Card

## Metadata

- Task ID: `TASK-2026-078`
- Title: Keep the bwrap seccomp descriptor open
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-19`
- Target milestone:

## Problem

Linux skill runs at sandbox level 2 and level 3 pass `--seccomp 3` to bubblewrap. The BPF file is opened with `O_CLOEXEC`. In a normal CLI process the first free descriptor is 3, so that file is descriptor 3. `dup2(3, 3)` does not create a second copy, and the child then closes the source. bubblewrap exits with `Can't read seccomp data: Bad file descriptor` and the skill never starts.

## Scope

- In scope:
  - Stop closing descriptor 3 when it is already the seccomp BPF file.
  - Still move a higher descriptor onto 3, clear `CLOEXEC`, and close the original.
  - Regression tests for both descriptor layouts.
- Out of scope:
  - Changing the seccomp filter contents.
  - The parent-process descriptor leak from `into_raw_fd` (one descriptor per run, released when the CLI exits).
  - macOS Seatbelt and the agent-sandbox bwrap plan.

## Acceptance Criteria

- [x] `skilllite-sandbox run` of `.skills/calculator` at the default sandbox level returns the arithmetic result.
- [x] The same run at sandbox level 2 returns the arithmetic result.
- [x] A unit test fails if descriptor 3 is closed after `dup2(3, 3)`.
- [x] A higher source descriptor is still moved to 3 and closed.
- [x] `cargo test -p skilllite-sandbox` passes.

## Risks

- Risk: Leaving descriptor 3 open with `CLOEXEC` still set would close it at `exec`.
  - Impact: bubblewrap would fail the same way.
  - Mitigation: Clear `CLOEXEC` on descriptor 3 before returning, and assert that in the test.

## Validation Plan

- Required tests: `cargo test -p skilllite-sandbox`, `cargo fmt --check`, `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: `skilllite-sandbox run .skills/calculator` at level 3 and level 2

## Regression Scope

- Areas likely affected: Linux `skilllite run` / `skilllite-sandbox run` at sandbox level 2 and 3
- Explicit non-goals: level 1 (`SKILLLITE_NO_SANDBOX` or sandbox level 1), macOS, agent-sandbox

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: no user-facing contract change
