# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run/plan.rs`, `crates/skilllite-sandbox/src/agent_run/mod.rs`, EN/ZH architecture and home-node docs
- Commits/changes: Linux nftables cgroup match for `agent-sandbox` proxy enforcement

## Findings

- Critical: none remaining. The leaf-name match was the defect under review.
- Major: none
- Minor: none

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --all -- --check` (exit 0)
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` (exit 0)
  - `cargo test -p skilllite-sandbox` (exit 0)
  - `cargo test -p skilllite --test e2e_minimal` (exit 0)
  - Live nftables probe on nftables 1.0.9: full relative path at the component-count level
- Key outputs:
  - `skilllite-sandbox` lib: `test result: ok. 105 passed; 0 failed; 0 ignored`
  - `e2e_minimal`: `test result: ok. 2 passed; 0 failed`
  - Probe: `proxy_port OPEN` and `other_port TimeoutError` for `socket cgroupv2 level 5 "<nested>/skilllite-run-1"`
- Falsifiability: `nft_script_names_the_nested_cgroup_path` requires the quoted path `user.slice/user-1000.slice/user@1000.service/skilllite-run-1` at level 4 and rejects `level 4 "skilllite-run-1"`. A leaf-only match fails that assertion.
- `cargo audit` was not run; it is not installed in this environment.

## Decision

- Merge readiness: `ready`
- Follow-up actions: none for this path. Hosts without nftables or without permission to create a child cgroup still fail closed.

## Security policy note

- The output chain policy stays `accept`. Only the sandbox cgroup is matched. A shorter level was not used: on this host, level 1 with the full path installed and did not drop a closed local port (`ConnectionRefusedError`).
- Runs that omit `https_proxy` are unchanged. `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` is unchanged.
- Compared with the previous script, a proxied Linux run can start. Egress from that cgroup is limited to the resolved proxy. That is the documented allowlist, not a wider network policy.
