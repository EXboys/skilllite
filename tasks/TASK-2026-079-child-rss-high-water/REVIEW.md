# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/common.rs`, `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
- Commits/changes: sandbox memory high-water comparison and the two regression tests

## Findings

- Critical: none remaining. The false rejection is covered by `later_command_keeps_stdout_when_an_earlier_child_peaked_higher`.
- Major: none.
- Minor: a child that exceeds its limit but stays under an older high-water mark can still finish if it exits before the RSS poll. The poll still kills a process that remains over the limit, and a new high-water mark above the limit is still rejected.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo test -p skilllite-sandbox --lib later_command_keeps_stdout_when_an_earlier_child_peaked_higher -- --test-threads=1` before the fix
  - `cargo test -p skilllite-sandbox --lib -- --test-threads=1 later_command_keeps_stdout child_that_raises_the_rss_peak` after the fix
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo test -p skilllite-sandbox`
  - `cargo fmt --check`
  - `SKILLLITE_SANDBOX_LEVEL=1 SKILLLITE_AUTO_APPROVE=1 cargo test -p skilllite --test e2e_minimal -- --test-threads=1`
- Key outputs:
  - Before the fix: `assertion left == right failed: stderr=Process rejected: peak memory (57 MB) exceeded limit (4 MB) reason=Some("memory_limit")` with left `-1` and right `0`.
  - After the fix: both memory tests passed, then `cargo test -p skilllite-sandbox` reported `116 passed`.
  - e2e: `e2e_add_local_zip_scan_minimal_skill` and `e2e_add_scan_run_minimal_skill` passed. The test binary looks next to a Cargo layout path, so the built `target/debug/skilllite` was linked into that directory first. Sandbox level 1 was set so this run does not depend on the separate Linux seccomp fd 3 failure.
  - `cargo clippy --workspace --all-targets -- -D warnings` fails in `crates/skilllite-agent/src/extensions/builtin/helpers.rs` on `clippy::needless_bool`. That file is unchanged by this fix. `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` passes.

## Decision

- Merge readiness: `ready`
- Follow-up actions: none for this bug. The Linux seccomp fd 3 startup failure remains the existing unmerged fix and is not part of this change.
