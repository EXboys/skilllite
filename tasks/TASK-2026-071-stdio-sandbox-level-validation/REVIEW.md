# Review Report

## Scope Reviewed

- Files/modules: Recent commits after `74f8417`, stdio execution parameter parsing,
  sandbox-level resolution, Python IPC callers, and MCP validation parity.
- Commits/changes: Added shared validation before narrowing conversion and parser
  regression tests for valid, omitted, truncating, and malformed values.

## Findings

- Critical: Stdio `run` and `exec` truncate unbounded sandbox-level integers before
  policy selection. `257` becomes level 1 and disables isolation.
- Major: None.
- Minor: None in the selected fix scope.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass` because the documented 1-to-3 contract is unchanged

## Test Evidence

- Commands run:
  - `cargo test -p skilllite stdio_rpc_params`
  - `cargo test -p skilllite`
  - `cargo test -p skilllite-sandbox`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo clippy --all-targets -- -D warnings -A clippy::question_mark -A clippy::useless_borrows_in_formatting`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
  - Live `skilllite serve --stdio` request with `sandbox_level: 257`
  - `cargo audit` was attempted but is unavailable in this environment.
- Key outputs:
  - Focused parser regression: 3 passed, 0 failed.
  - `skilllite` and sandbox suites passed; sandbox unit suite: 81 passed.
  - Full workspace test groups all passed, including the changed parser tests.
  - Strict Clippy was blocked only by two pre-existing main-branch lints in
    `scan.rs` and `admission.rs`; allowing exactly those categories produced a
    clean workspace Clippy result.
  - Live stdio response returned
    `sandbox_level must be one of [1, 2, 3]` before touching the invalid skill path.
  - Task validation passed for all 71 task directories.
  - The regression is falsifiable: restoring the former cast makes the
    `unwrap_err()` assertions receive `Ok` for `257`.

## Decision

- Merge readiness: ready
- Follow-up actions: The separate CLI/environment policy propagation issue found
  during review remains outside this minimal stdio parsing fix.
