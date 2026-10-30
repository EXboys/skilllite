# TASK Card

## Metadata

- Task ID: `TASK-2026-076`
- Title: Mount name service files for unproxied agent sandbox
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-10-30`
- Target milestone:

## Problem

On Linux, `skilllite_sandbox::run` shares the host network namespace but does not mount `/etc/resolv.conf`, `/etc/nsswitch.conf`, or `/etc/hosts`. With no `https_proxy` (the CLI default), glibc name lookup returns `EAI_AGAIN` (`-3`), so Claude and Codex cannot resolve their API hosts. A proxied run is unaffected because the proxy URL is already an IP and DNS inside the sandbox is intentionally disabled.

## Scope

- In scope:
  - Read-only mount of the three name-service files on the Linux bwrap plan when no proxy is set and the file exists.
  - Keep those files unmounted when a proxy is set.
  - Regression tests and EN/ZH docs for that split.
- Out of scope:
  - Changing the nftables allowlist, Seatbelt profile, or skill `execute()` mounts.
  - Canonicalizing multi-hop agent symlinks (tracked separately).

## Acceptance Criteria

- [x] An unproxied bwrap plan read-only binds `/etc/resolv.conf`, `/etc/nsswitch.conf`, and `/etc/hosts` when those flags are set.
- [x] A proxied bwrap plan does not bind those paths even when the flags are set.
- [x] `detect_layout` records whether each host file exists.
- [x] A Linux `run` without a proxy can read the files that exist on the host.
- [x] EN and ZH architecture and home-node docs describe the split.

## Risks

- Risk: Mounting resolver config on a proxied run would let the process learn DNS servers and, if the allowlist were wrong, resolve names.
  - Impact: Weakens the documented "do not resolve DNS inside the sandbox" rule.
  - Mitigation: Mount the files only when `proxy` is `None`. The proxied plan test fails if they appear.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-sandbox`
  - `cargo test -p skilllite --test e2e_minimal`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo test -p skilllite-sandbox --lib linux_run_without_proxy_mounts_name_service_files -- --nocapture`
- Manual checks:
  - Host `getaddrinfo("example.com")` inside an agent-style bwrap fails without the files and succeeds with them.

## Regression Scope

- Areas likely affected:
  - Linux `agent-sandbox` with an empty `https_proxy`.
- Explicit non-goals:
  - Proxy nftables matching, macOS Seatbelt reads, skill sandbox mounts.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `docs/en/HOME-NODE-CONNECT.md`, `docs/zh/HOME-NODE-CONNECT.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
