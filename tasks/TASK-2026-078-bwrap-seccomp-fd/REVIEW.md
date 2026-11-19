# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/linux.rs`
- Commits/changes: keep bwrap seccomp descriptor 3 open when `dup2` is a no-op

## Findings

- Critical: none
- Major: none
- Minor: the parent still leaks the raw BPF descriptor until process exit. That is one descriptor per run and is outside this fix.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check -p skilllite-sandbox`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo test -p skilllite-sandbox`
  - `SKILLLITE_MAX_PROCESSES=200 SKILLLITE_AUTO_APPROVE=1 ./target/debug/skilllite-sandbox run .skills/calculator '{"operation":"add","a":1,"b":2}'`
  - `SKILLLITE_MAX_PROCESSES=200 SKILLLITE_SANDBOX_LEVEL=2 ./target/debug/skilllite-sandbox run .skills/calculator '{"operation":"add","a":1,"b":2}'`
- Key outputs:
  - Unfixed binary: `bwrap: Can't read seccomp data: Bad file descriptor` (exit 1)
  - Fixed binary, level 3 and level 2: `{"operation": "add", "a": 1.0, "b": 2.0, "result": 3.0}` (exit 0)
  - With the close restored, `seccomp_fd_stays_open_when_the_filter_is_already_fd_3` panicked: `seccomp fd check failed with code 2`
  - `cargo test -p skilllite-sandbox`: 116 passed

## Decision

- Merge readiness: `ready`
- Follow-up actions: none
