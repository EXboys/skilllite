# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `crates/skilllite-sandbox/src/agent_run/plan.rs`
- Current behavior before this change: `resolve_host_binary` kept the symlink path and `bind_canonical_target` mounted only the canonical parent. A one-hop absolute symlink works. The Codex installer layout does not, because `current` is a second symlink.

## Architecture Fit

- Layer boundaries involved: `skilllite_sandbox::run` only. No new crate dependency.
- Interfaces to preserve: `RunRequest::new` still defaults to Claude. CLI flags are unchanged.

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: A command that is already a regular file still runs at that path. `HostLayout` gains `launcher_prefix`, set by `run` after `detect_layout`. The struct is crate-private.

## Design Decisions

- Decision: Store the extra directory on `HostLayout` instead of adding another `spawn_sandboxed` argument.
  - Rationale: The Linux spawn function was already at Clippy's argument limit.
- Decision: Compare launcher and canonical parents with `Path` equality and skip the extra entry when they match.
  - Rationale: A normal `/usr/bin/tool` should not duplicate its directory on `PATH`.

## Open Questions

- [x] None. CI installs bubblewrap so the Linux `run` tests execute there. Unprivileged user namespaces are enabled when the AppArmor sysctl exists.
