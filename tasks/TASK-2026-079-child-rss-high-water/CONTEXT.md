# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/common.rs` (`wait_with_timeout`, `get_children_peak_rss_bytes`). Callers include `spawn_write_and_wait` (Linux bwrap, macOS Seatbelt, unsandboxed execution) and `crates/skilllite-commands/src/execute.rs` bash execution.
- Current behavior: on exit, any `ru_maxrss` above `memory_limit_bytes` discards stdout. The value includes earlier children and, for bwrap, grandchildren that the sandbox waited for.

## Architecture Fit

- Layer boundaries involved: the check stays inside `skilllite-sandbox`. Commands keep calling `wait_with_timeout`.
- Interfaces to preserve: `wait_with_timeout` signature and the `memory_limit` kill reason string.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: a later command is no longer failed because an earlier child peaked higher. A command that sets a new peak above its limit still fails.

## Design Decisions

- Decision: sample `ru_maxrss` immediately before `try_wait`, and reject only when the value after reap is strictly greater and above the limit.
  - Rationale: `try_wait` is what folds this child, and bwrap's grandchildren, into `RUSAGE_CHILDREN`. Sampling before the reap ignores historical peaks without missing a new peak.
  - Alternatives considered: read `/proc/<pid>/status` `VmHWM` for the direct child.
  - Why rejected: bwrap's own high-water mark does not include the sandboxed skill, so that check would stop enforcing the limit on the default Linux sandbox path.

## Open Questions

- [x] Is the false rejection reachable from a default caller? Yes. Agent and CLI skill/bash execution wait for each command in one process, and a helper or an earlier skill above the limit poisons the next wait.
- [x] Should docs change? Yes. State that the limit applies to the current run rather than every earlier child.
