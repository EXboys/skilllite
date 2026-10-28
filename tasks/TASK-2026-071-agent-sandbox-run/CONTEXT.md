# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-sandbox/src/linux.rs` (`execute_with_bwrap` uses `--unshare-all`), `macos.rs` (skill Seatbelt), `skilllite/src/cli.rs` (`run` is skill/agent execution).
- Current behavior: sandbox entry points execute skills. They do not copy a subscription directory or force an external HTTPS proxy.

## Architecture Fit

- Layer boundaries involved: `skilllite-sandbox` stays below executor and agent. The CLI calls the sandbox crate directly.
- Interfaces to preserve: `execute` / `execute_with_bwrap` and skill seccomp.

## Dependency and Compatibility

- New dependencies: none. Unix locking uses `libc`, already a target dependency.
- Backward compatibility notes: new CLI variant `agent-sandbox`. Existing `run` flags are unchanged.

## Design Decisions

- Decision: separate module `agent_run`, not a branch inside `execute_with_bwrap`.
  - Rationale: skill isolation and this path have opposite network and timeout policies.
  - Alternatives considered: reuse `execute_with_bwrap` and delete `--unshare-all` at the call site.
  - Why rejected: that would risk the skill path.
- Decision: Linux destination filter is an nftables script scoped to a private cgroup, applied before spawn.
  - Rationale: filtering the host uid would drop the operator's own traffic.
  - Alternatives considered: environment variables only.
  - Why rejected: the process can unset `HTTPS_PROXY` and connect directly.
- Decision: CLI name `agent-sandbox`.
  - Rationale: `skilllite run` is already skill execution.
- Decision: macOS Seatbelt allows broad reads and write access only to the canonical home and work directories. A loopback proxy is written as `localhost:<port>`. A non-loopback proxy is rejected.
  - Rationale: this OS rejects literal IP hosts in `sandbox-exec`, and a narrow `file-read*` subpath list aborts `/bin/echo` before `main`.
  - Alternatives considered: `(remote tcp "127.0.0.1:port")`, and `(allow file-read* (subpath "/usr") ...)` only.
  - Why rejected: both fail on darwin 24. `*:443` would allow the public internet on that port.
- Decision: nft `socket cgroupv2 level` uses the component count under `/sys/fs/cgroup`, not a fixed level 1.
  - Rationale: the private cgroup is created under the current process cgroup, which is not a direct child of the v2 root.
- Decision: missing-proxy refusal is `RunRequest.require_proxy`, default false. CLI `--require-proxy` and `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY` set it.
  - Rationale: the caller turns the gate on when a subscription must egress only through a proxy.
  - Impact: the default is more permissive than the first cut. A provided proxy is still resolved and pinned. Skill `execute()` is unchanged.

## Open Questions

- [x] Pool and SSE stay out of this repository.
- [x] Team edition layout is unchanged.
