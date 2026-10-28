# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/agent_run/plan.rs`, `crates/skilllite-sandbox/src/agent_run/mod.rs`
- Current behavior: `install_linux_filter` creates `skilllite-<run>` under the caller's cgroup and passes that leaf name to `nft_script`. nftables 1.0.9 looks the string up under `/sys/fs/cgroup` and returns `cgroupv2 path fails: No such file or directory`.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` agent-run plan only. Skill `execute()` is not called.
- Interfaces to preserve: `RunRequest`, CLI flags, and the fail-closed error when nftables or the cgroup cannot be used.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: Linux runs that set `https_proxy` and previously failed at `nft -f` can start once the allowlist installs. Egress stays limited to the resolved proxy endpoints. Runs without a proxy are unchanged.

## Design Decisions

- Decision: Put the full relative cgroup path in the `socket cgroupv2` match and set `level` from that path's component count.
  - Rationale: On nftables 1.0.9 the string is a path. The level is still required at match time. Level equal to the component count dropped TCP to ports other than the proxy; level 1 with the same path installed but did not drop (connection refused from a closed port).
  - Alternatives considered: Keep the leaf name and only fix the level. Change the chain policy to `drop`.
  - Why rejected: The leaf name does not resolve, so the rule never installs. Policy `drop` on hook output would discard egress from every process on the host, not only the sandbox.

## Open Questions

- [x] None for this slice. Hosts without nftables or without cgroup delegation still fail closed.
