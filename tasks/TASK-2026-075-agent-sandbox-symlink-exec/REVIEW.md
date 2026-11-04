# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `crates/skilllite-sandbox/src/agent_run/plan.rs`, EN/ZH architecture and home-node docs, `.github/workflows/ci.yml`
- Commits/changes: canonical executable path for `agent-sandbox`

## Findings

- Critical: none remaining
- Major: none
- Minor: none

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check` (exit 0)
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` (exit 0)
  - `cargo test -p skilllite-sandbox` (exit 0)
  - `cargo test -p skilllite --test e2e_minimal` (exit 0)
- Key outputs:
  - `skilllite-sandbox` lib: `test result: ok. 119 passed; 0 failed; 0 ignored`
  - Includes `linux_run_executes_a_multihop_symlink` and `linux_run_finds_node_beside_a_symlink_launcher`
  - `e2e_minimal`: `test result: ok. 2 passed; 0 failed`
- Falsifiability: before the change, the same bwrap mounts with the symlink as argv0 failed with `No such file or directory`. `linux_run_executes_a_multihop_symlink` fails if that path is executed again. `linux_run_finds_node_beside_a_symlink_launcher` fails if the launcher directory is dropped from `PATH`.

## Decision

- Merge readiness: `ready`
- Follow-up actions: none

## Security policy note

- What changed: the executed path is the canonical file. The symlink directory stays a read-only mount and the first `PATH` entry when it is a different directory. No new writable root. Proxy and nftables behavior are unchanged.
- Default permissiveness: unchanged. Both directories were already mounted for a symlinked command.
- `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` is unchanged.
