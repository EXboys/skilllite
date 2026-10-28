# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/agent_run/mod.rs`, `crates/skilllite-sandbox/src/agent_run/redact.rs`, `skilllite/src/cli.rs`, `skilllite/src/dispatch/agent_sandbox.rs`
- Current behavior: One outer sandbox runs a command with `HOME` set to an ephemeral directory. The login directory is copied in. The original directory is not mounted.

## Architecture Fit

- Layer boundaries involved: `skilllite` CLI calls `skilllite_sandbox::run`. No new crate dependency.
- Interfaces to preserve: `RunRequest::new` still defaults to Claude, `require_proxy: false`, and an empty `argv`.

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility notes: A new public enum `AgentRuntime` and a `runtime` field on `RunRequest`. Struct update syntax keeps compiling. The CLI flag defaults to `claude`.

## Design Decisions

- Decision: Codex default argv includes `--dangerously-bypass-approvals-and-sandbox`.
  - Rationale: Codex documents that flag for a process that is already inside an external sandbox. Nested bwrap or Seatbelt does not start.
  - Alternatives considered: `--sandbox workspace-write`.
  - Why rejected: That keeps Codex's own sandbox, which fails inside the outer sandbox and would also fight the write roots this run already enforces.
- Decision: Persist only `.codex/auth.json`.
  - Rationale: Codex rotates the ChatGPT session in that file. Dropping the ephemeral copy invalidates the next run. Other files are not session credentials that must survive.
  - Alternatives considered: Mount the original `.codex` writable.
  - Why rejected: The sandbox process would then be able to change every file in the subscription directory.

## Open Questions

- [x] None for this slice. The connect service still has to learn Codex JSONL before a phone can render it like Claude stream-json.
