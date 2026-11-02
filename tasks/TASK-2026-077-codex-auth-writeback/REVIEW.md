# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `skilllite/src/cli.rs`, EN/ZH architecture and home-node docs, `CHANGELOG.md`
- Commits/changes: Codex `auth.json` writeback skips an unchanged snapshot

## Findings

- Critical: none
- Major: none
- Minor: two concurrent refreshes in one run still leave one file. That conflict is recorded as out of scope in `CONTEXT.md`.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check -p skilllite-sandbox -p skilllite` (exit 0)
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` (exit 0)
  - `cargo clippy -p skilllite --all-targets -- -D warnings` (exit 0)
  - `cargo test -p skilllite-sandbox -- --test-threads=8` (exit 0)
  - `cargo test -p skilllite --test e2e_minimal -- --test-threads=8` (exit 0)
- Key outputs:
  - Removing the snapshot compare made `unchanged_codex_auth_does_not_replace_a_newer_host_file` fail with `left: "old-中文"` and `right: "newer-中文"`.
  - Restoring the compare: `codex_auth` tests passed (`2 passed`).
  - `skilllite-sandbox` lib: `test result: ok. 115 passed; 0 failed; 0 ignored; 0 measured`.
  - `e2e_minimal`: `test result: ok. 2 passed; 0 failed`.
- `cargo audit` is not installed in this environment.

## Decision

- Merge readiness: `ready`
- Follow-up actions: none for this writeback. Open PRs #177 and #178 stay separate.

## Security policy note

- What changed: writeback of Codex `auth.json` now happens only when the sandbox bytes differ from the startup snapshot. A real write still replaces one regular file and holds `auth.lock` for that rename.
- Default behavior is stricter. Network, mount, and Claude paths are unchanged.
- No `SKILLLITE_*` variable changed.
