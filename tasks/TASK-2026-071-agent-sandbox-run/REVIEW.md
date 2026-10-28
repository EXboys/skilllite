# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run`, `skilllite/src/cli.rs`, `skilllite/src/dispatch/agent_sandbox.rs`
- Commits/changes: uncommitted working tree

## Findings

- Critical: none
- Major: none
- Minor: macOS Seatbelt cannot hide other directories from reads. Writes outside the canonical home and work directory are denied. Linux mount isolation is the path that hides the host home. This host did not execute `bwrap` or nftables.
- Minor: `require_proxy` defaults to false, so a run with no proxy starts and egress is not pinned. Set the flag or `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` to restore the refusal. A provided proxy is still pinned.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check` (exit 0)
  - `cargo clippy --all-targets -- -D warnings` (exit 0, `Finished dev profile ... in 12.98s`)
  - `cargo test` (exit 0)
- Key outputs:
  - `skilllite-sandbox` lib: `test result: ok. 105 passed; 0 failed; 0 ignored`
  - `macos_sandbox_smoke`: `test result: ok. 1 passed; 0 failed`
  - `e2e_minimal`: `test result: ok. 2 passed; 0 failed`
  - `e2e_macos_sandbox`: `test result: ok. 1 passed; 0 failed`
  - `agent_run` cases include redaction with `中文`, directory lock `busy`, ephemeral home removal, bwrap argv without `--unshare-all` / `--unshare-net`, nft `socket cgroupv2 level 3`, `require_proxy` refusing an empty proxy, and a start with no proxy when the switch is off.
  - 2026-10-28 follow-up: `cargo clippy -p skilllite-sandbox -p skilllite --all-targets -- -D warnings` exit 0. `cargo test -p skilllite-sandbox --lib agent_run` → `test result: ok. 24 passed; 0 failed`. `cargo test -p skilllite --test e2e_minimal` → `test result: ok. 2 passed; 0 failed`.

## Decision

- Merge readiness: ready
- Follow-up actions: run the Linux spawn once on a host with `bwrap`, nftables, and cgroup v2 before relying on that path in the private pool.
