# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/agent_run/plan.rs`, `crates/skilllite-sandbox/src/agent_run/mod.rs`.
- Current behavior: `bwrap_arguments` mounts `/usr`, `/lib`, `/bin`, `/lib64`, CA certs, the binary directory, the ephemeral home, and the work directory. It does not mount name-service files. `run` calls that plan for every Linux subscription run. Skill `execute()` in `linux.rs` already mounts `/etc/resolv.conf`, `/etc/nsswitch.conf`, and `/etc/hosts` because glibc returns `EAI_AGAIN` without them.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` agent-run plan only. Skill `execute()` is not called.
- Interfaces to preserve: `RunRequest`, CLI flags, nftables cgroup match, and Seatbelt profiles stay the same.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: Unproxied Linux runs gain hostname lookup, which is the documented unpinned-egress behavior. Proxied runs keep the same filesystem and allowlist as before.

## Design Decisions

- Decision: Mount the three files only when `proxy` is `None`, and only when `detect_layout` saw each file.
  - Rationale: The no-proxy path is the one that must resolve `api.anthropic.com` or the Codex API host. The proxy path resolves the proxy on the host and forbids other TCP/UDP, including DNS.
  - Alternatives considered: Always mount the files, including on proxied runs.
  - Why rejected: Docs say a proxied sandbox does not resolve DNS. Leaving the files out keeps that true even if a later allowlist mistake would otherwise let port 53 through. `resolv.conf` alone was enough on this host; all three are mounted so a host whose `nsswitch.conf` does not use the glibc default still resolves names, matching the skill sandbox.

## Open Questions

- [x] Does bwrap 0.9.0 follow a symlink `/etc/resolv.conf`? Yes. A bind of a symlink to a regular file presented the target contents at the destination.
