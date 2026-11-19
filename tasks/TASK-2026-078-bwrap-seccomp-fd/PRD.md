# PRD

## Background

Default Linux skill execution uses bubblewrap plus a seccomp filter. The filter is passed as an open file descriptor, not a path. On 2026-11-19 a fresh `skilllite-sandbox run .skills/calculator` failed before the skill process started:

`bwrap: Can't read seccomp data: Bad file descriptor`

## Objective

A default Linux sandboxed skill run starts inside bubblewrap with the existing seccomp filter installed.

## Functional Requirements

- FR-1: When the BPF file is already descriptor 3, that descriptor stays open across `exec` and is not close-on-exec.
- FR-2: When the BPF file is a higher descriptor, it is duplicated onto descriptor 3, close-on-exec is cleared, and the original descriptor is closed.
- FR-3: Failure to duplicate or to clear close-on-exec fails the spawn instead of starting bubblewrap with a missing filter.

## Non-Functional Requirements

- Security: The filter bytes and the syscall denylist do not change. The sandbox stays fail-closed when the descriptor cannot be prepared.
- Performance: One `dup2` and one `fcntl` in the child, same as before.
- Compatibility: Level 1 and macOS paths are unchanged.

## Constraints

- Technical: The preparation runs in `pre_exec` after fork. It may only use the descriptor operations already used on this path.
- Timeline: Minimal fix on the current main line.

## Success Metrics

- Metric: calculator skill prints `{"operation":"add",... "result":3.0}` under sandbox level 3 and level 2.
- Baseline: the same command exits 1 with `Can't read seccomp data: Bad file descriptor`.

## Out of Scope

- Rewriting the BPF program.
- Closing the parent's copy of the descriptor before spawn.
