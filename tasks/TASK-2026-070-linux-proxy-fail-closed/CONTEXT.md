# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-sandbox/src/common.rs`
  - `crates/skilllite-sandbox/src/linux.rs`
  - `crates/skilllite-sandbox/src/security/policy.rs`
- Current behavior:
  - `common::start_network_proxy` returns `None` both when no proxy is needed and when proxy setup fails.
  - Linux bwrap uses `--share-net` for every non-`BlockAll` policy, so direct sockets remain available.
  - Linux firejail only distinguishes `BlockAll`, proxy present, and direct/wildcard access in logging.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` remains the policy enforcement layer below executor/commands.
- Interfaces to preserve: public sandbox execution APIs and `ResolvedNetworkPolicy` semantics.

## Dependency and Compatibility

- New dependencies: None.
- Backward compatibility notes: Skills that request filtered networking now fail closed on Linux; wildcard `*` remains the compatibility path for explicit direct egress.

## Design Decisions

- Decision: Add a small Linux-side policy helper that rejects `ProxyFiltered` before building sandbox network arguments.
  - Rationale: Linux currently has no kernel-enforced path that forces all skill egress through the local proxy; rejecting the policy avoids pretending environment variables are a security boundary.
  - Alternatives considered: Change `start_network_proxy` to return `Result<Option<ProxyManager>>` for all platforms.
  - Why rejected: Broader API churn is unnecessary for the immediate Linux fail-open and would change macOS behavior beyond this fix.

## Open Questions

- [x] Should macOS behavior change? No; current Seatbelt profile already denies network when no proxy ports are available.
- [x] Is docs sync required? No user-facing configuration semantics change; this is enforcement correctness for existing policy.
