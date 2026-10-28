# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `crates/skilllite-sandbox/src/agent_run/redact.rs`, `skilllite/src/cli.rs`, `skilllite/src/dispatch/agent_sandbox.rs`, EN/ZH architecture and home-node docs
- Commits/changes: Codex runtime for `agent-sandbox`

## Findings

- Critical: none
- Major: none
- Minor: none

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt -p skilllite-sandbox -p skilllite -- --check` (exit 0)
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` (exit 0)
  - `cargo clippy -p skilllite --all-targets -- -D warnings` (exit 0)
  - `cargo test -p skilllite-sandbox` (exit 0)
  - `cargo test -p skilllite --test e2e_minimal` (exit 0)
- Key outputs:
  - `skilllite-sandbox` lib: `test result: ok. 109 passed; 0 failed; 0 ignored`
  - macOS sandbox smoke: `test result: ok. 1 passed; 0 failed`
  - `e2e_minimal`: `test result: ok. 2 passed; 0 failed`
- Falsifiability: `codex_run_writes_auth_json_back_and_leaves_other_files` fails if `auth.json` stays `old` or if `config.toml` becomes `changed`. `claude_run_does_not_write_the_login_directory_back` fails if the original marker becomes `changed`. `codex_auth_writeback_replaces_only_a_regular_file` fails if a symlink to `/etc/passwd` is copied.

## Decision

- Merge readiness: `ready`
- Follow-up actions: the connect service still needs a Codex JSONL reader. That is not this repository.

## Security policy note

- What changed: Codex runs with `--dangerously-bypass-approvals-and-sandbox` so its own sandbox does not start inside bwrap or Seatbelt. The outer profile still limits writes to the ephemeral home and the work directory. A provided proxy is still the only egress. With no proxy, egress stays unpinned, the same as a Claude run.
- `auth.json` is the one subscription file written back, and only when it is a regular file. Claude login files are still not written back.
- `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` is unchanged. The new flag is `--runtime`, default `claude`.
