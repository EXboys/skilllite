# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/agent_run/plan.rs`, `crates/skilllite-sandbox/src/agent_run/mod.rs`, `.github/workflows/ci.yml`, EN/ZH architecture and home-node docs
- Commits/changes: Linux agent sandbox name-service mounts when `https_proxy` is empty

## Findings

- Critical: none remaining
- Major: none
- Minor: none

## Quality Gates

- Architecture boundary checks: `pass` (agent-run plan only; skill `execute()` is untouched)
- Security invariants: `pass`
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-sandbox`
  - `cargo test -p skilllite --test e2e_minimal`
  - `cargo test -q`
- Key outputs:
  - Unproxied bwrap without the files: `gaierror [Errno -3] Temporary failure in name resolution`
  - With the files: `getaddrinfo("example.com", 443)` returned an address
  - Removing the mounts failed `linux_run_without_proxy_mounts_name_service_files` with `missing /etc/resolv.conf in []`
  - Workspace `cargo test -q` exit 0; workspace clippy exit 0
  - `e2e_minimal`: `2 passed; 0 failed`

## Decision

- Merge readiness: `ready`
- Follow-up actions: none for this bug. Multi-hop agent symlink execution stays in the separate open PR.

## Security policy note

- What changed: an unproxied Linux `run` read-only mounts `/etc/resolv.conf`, `/etc/nsswitch.conf`, and `/etc/hosts` when each file exists. A proxied run does not mount them.
- Default behavior is more permissive only for the unproxied filesystem view (three read-only files). Unproxied egress was already unpinned; hostname lookup is what that egress needs. Proxied nftables tcp/udp drop is unchanged, and DNS files stay out of that plan.
- `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` and other `SKILLLITE_*` semantics are unchanged.
- CI installs bubblewrap so the live mount test runs on Ubuntu.
