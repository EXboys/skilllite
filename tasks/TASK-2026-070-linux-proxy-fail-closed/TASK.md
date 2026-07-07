# TASK Card

## Metadata

- Task ID: `TASK-2026-070`
- Title: Fail closed when Linux filtered network proxy is unavailable
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-07`
- Target milestone:

## Problem

Linux sandbox execution cannot currently enforce domain-filtered networking with
bwrap/firejail. The local proxy only works for clients that honor proxy
environment variables, while the sandbox still permits direct outbound sockets.
This turns an intended allowlist into unrestricted egress for that skill run.

## Scope

- In scope:
  - Fail closed on Linux when `ResolvedNetworkPolicy::ProxyFiltered` is requested.
  - Preserve existing `BlockAll` and wildcard `AllowAll` behavior.
  - Add focused regression tests for the network decision logic.
- Out of scope:
  - Reworking proxy implementation internals.
  - Changing macOS Seatbelt profile generation.
  - Changing user-facing sandbox configuration semantics.

## Acceptance Criteria

- [ ] Linux bwrap/firejail paths reject filtered-network execution before launching the skill.
- [ ] Linux `BlockAll` still isolates networking and wildcard `AllowAll` remains direct.
- [ ] Regression tests cover filtered-without-proxy, filtered-with-proxy, block-all, and allow-all decisions.

## Risks

- Risk: Treating Linux domain allowlists as unsupported may fail skills that previously ran with unintended direct egress.
  - Impact: Some misconfigured or resource-constrained hosts see a clear sandbox error instead of execution.
  - Mitigation: This is the desired security posture for allowlisted networking; wildcard `*` remains available for explicit direct egress.

## Validation Plan

- Required tests: focused `skilllite-sandbox` unit tests plus workspace Rust checks.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-sandbox`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks: inspect Linux bwrap/firejail command-building branches for fail-closed behavior.

## Regression Scope

- Areas likely affected: Linux sandbox network policy enforcement and proxy-filtered skill execution.
- Explicit non-goals: macOS sandbox behavior, runtime dependency provisioning, and unrelated evolution workspace issues.

## Links

- Source TODO section: N/A - daily critical bug investigation.
- Related PRs/issues: Recent sandbox/CI work around macOS smoke coverage.
- Related docs: `spec/security-nonnegotiables.md`, `spec/testing-policy.md`.
