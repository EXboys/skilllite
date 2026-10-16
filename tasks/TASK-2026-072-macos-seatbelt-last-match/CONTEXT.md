# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/macos.rs`, `crates/skilllite-sandbox/src/lib.rs`, `crates/skilllite-sandbox/src/security/policy.rs`.
- Current behavior: `execute_with_sandbox` writes the profile from `generate_sandbox_profile_with_proxy` and launches `sandbox-exec -f`.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` only. No crate dependency change.
- Interfaces to preserve: `execute_with_limits` signature, `SandboxLevel` defaults, `ResolvedNetworkPolicy` variants.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: macOS skills that exec extra binaries or open sockets outside an explicit `*` / proxy allow now fail closed. Level 1 is unchanged.

## Design Decisions

- Decision: Put `(allow default)` immediately after `(version 1)`, then emit denies, then the narrow allows that must win (interpreter, work dir, proxy ports, `/private/etc/localtime`).
  - Rationale: Last matching rule wins. Denies after the baseline allow stick. Narrow allows after those denies restore the intended exceptions.
  - Alternatives considered: Delete `(allow default)` and list every mach-lookup/sysctl allow.
  - Why rejected: That list is what `(allow default)` is for, and dropping it would break Python startup for unrelated operations.

## Open Questions

- [x] Resolved: Linux CI compiles `macos` when `cfg(test)` so the order tests run without a Mac.
- [x] Out of scope: live `sandbox-exec` proof. This host is Linux.
