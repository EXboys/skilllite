# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/linux.rs` (`execute_with_bwrap`).
- Current behavior: `File::open` of `seccomp.bpf` uses the lowest free descriptor. `Command` stdio occupies 0, 1, and 2. The BPF file is therefore descriptor 3 in `skilllite` and `skilllite-sandbox`. The child calls `dup2(3, 3)`, clears `CLOEXEC`, then `close(3)`. bubblewrap's `--seccomp 3` reads a closed descriptor and exits 1.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` only. The CLI calls `skilllite_commands::execute::run_skill`, which calls `execute_with_limits`.
- Interfaces to preserve: `--seccomp 3`, the generated BPF program, and fail-closed behavior when bubblewrap is missing.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: skills that already failed at sandbox start now run. The filter policy is unchanged. `SKILLLITE_NO_SANDBOX=1` and sandbox level 1 still skip this path.

## Design Decisions

- Decision: close the source descriptor only when it is not 3.
  - Rationale: `dup2` of a descriptor onto itself is a no-op and returns success. Closing afterwards drops the filter.
  - Alternatives considered: open the file with a `dup` onto a descriptor above 3 before spawn, then always close the source.
  - Why rejected: the child still has to treat descriptor 3 as special if that reserved slot is already the file. Skipping the close is the smaller change.

## Open Questions

- [x] Does a real CLI hit descriptor 3? Yes. Unfixed `skilllite-sandbox run .skills/calculator` printed the bubblewrap bad-descriptor error on 2026-11-19.
