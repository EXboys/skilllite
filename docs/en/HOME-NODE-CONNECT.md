# Sandbox, connect, subscription pool, and user binding (draft)

**Status:** Sandbox `run` is implemented in this repository (`skilllite_sandbox::run`, command `agent-sandbox`). The pool, user binding, and connect service remain a draft and are not in this repository.

This design does not use the Evotown Doctor WebSocket, `job.assign`, the engine queue, or the `agent-runs` event log.

Four parts:

| Part | Role |
|---|---|
| Subscription pool | Subscription directories already logged in on this computer. One record per subscription |
| User binding | One user maps to one subscription in the pool |
| Connect | The computer dials out. The phone subscribes to this run's SSE on the connect service |
| Sandbox | `skilllite-sandbox`'s `run`. One sandbox per run |

`run` lives in `skilllite-sandbox`. The Agent Doctor desktop and CLI call it. SkillLite does not depend on Agent Doctor. `skilllite-agent`, the evolution engine, `skilllite-commands`, and the `skilllite-assistant` desktop do not take part. Skill `execute()` stays offline, short-timeout, and on its current seccomp profile. It does not call `run`.

## The pool lives in a private repository

The public repositories keep two jobs. SkillLite provides `run`, the sandbox. Agent Doctor stays the personal edition: that person logs in one subscription of their own, their phone reaches their own computer, there is no second subscription slot, and that subscription cannot be bound to someone else.

The subscription pool, user binding, and connect service live in a repository that is not published. It calls `skilllite-sandbox`'s `run`, and it calls Agent Doctor's existing `install` and `doctor` for setup and diagnosis. It does not copy either repository, and it does not add the pool to their installers. A subscription belongs to the person who logged it in.

The Agent Doctor team edition stays the compile-time package it already is. This design does not change that layout and does not include the team edition.

## Subscription pool

The pool is on this computer. One subscription:

| Field | Visible outside | Meaning |
|---|---|---|
| `account_id` | yes | Stable id, such as `work` or `personal` |
| `runtime` | yes | `claude` or `codex` |
| `ready` | yes | The binary exists and the subscription directory contains a login file |
| `busy` | yes | This subscription already has a sandbox |
| Subscription directory | no | `.claude` from `claude auth login`, or `.codex` from `codex login` |
| `https_proxy` | no | Egress proxy for this subscription |

Login is not part of `run`. A person runs the official login once against that directory on the host. The program does not read, store, or forward tokens, and it does not open the macOS keychain. After Codex refreshes its session, the host writes back only the regular file `auth.json`.

One subscription has one sandbox at a time. Different subscriptions run in parallel. Install and diagnosis stay on the existing `install` and `doctor` commands, which are what make `ready` true.

## User binding

A user is bound to one subscription in the pool. A message from that user uses that directory and that proxy.

The personal edition has one binding, for the person at the machine, and does not show a pool. Only the home node keeps several of your own subscriptions. A computer runs one `connect` at a time.

A subscription belongs to the person who logged it in and cannot be rebound to someone else. When that person already has a sandbox running, a later run waits.

## Connect

A home network does not accept inbound connections. The computer dials the connect service. The phone and the computer do not talk directly.

```text
Phone
    │  GET /runs/{run_id}/stream
    ▼
Connect service
    │  one WebSocket: run requests down, stream-json lines back
    ▼
connect on this computer (desktop or CLI)
    │  user → bound subscription → run
    ▼
Sandbox
```

1. The phone gives the connect service one message, with `user_id` and this turn's `run_id`.
2. The connect service sends it down the WebSocket this computer already opened.
3. The computer calls `run` with the subscription bound to that user. stdout comes from `claude -p … --output-format stream-json --verbose`.
4. Each JSON line goes back to the connect service. Tokens that start with `sk-` are replaced with `[redacted]` first.
5. The connect service stores the line by `run_id` and pushes SSE to the phone in order: text, `tool_use`, `tool_result`. At the end it sends `done` with the exit status. SSE does not carry login or the pool.

Proposed line:

```json
{
  "type": "run.stream",
  "run_id": "…",
  "user_id": "…",
  "account_id": "work",
  "line": { "type": "assistant", "message": {} }
}
```

After a reconnect, a sandbox that is still running keeps sending lines for the same `run_id`. The connect service does not use the Evotown job queue.

## Sandbox

Each run calls `skilllite_sandbox::run` (CLI: `skilllite agent-sandbox`). The subscription directory, working directory, prompt, and `https_proxy` come from the caller. The connect service does not send the proxy, and the directory is not part of the published status. `https_proxy` may be omitted. With `--require-proxy`, or `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY=1`, a missing proxy refuses to start. When a proxy is set, macOS requires a loopback address and Seatbelt records it as `localhost:<port>`. On Linux the nftables match names that cgroup by its path under `/sys/fs/cgroup`, not by the leaf name alone. The run does not start when that allowlist cannot be attached.

1. Create an ephemeral home. `--runtime claude` (the default) copies `.claude`. `--runtime codex` copies `.codex`. The original directory is not mounted. A Claude run does not write it back. A Codex run writes back only the refreshed `auth.json`.
2. Mount the working directory in place at `/work` (on macOS, use it as the working directory). Results stay in that directory. One working directory has one `run` at a time. The only tree deleted at the end is the ephemeral home and the subscription copy inside it.
3. Take a file lock on the subscription directory, matching `busy` on the pool.
4. When `https_proxy` is set, resolve it on the host. Inside the sandbox, allow only that IP and port. Refuse every other TCP and UDP destination, including the LAN, the public internet, and every local port except the proxy port. Do not resolve DNS inside the sandbox. Local sockets stay allowed. If that proxy does not resolve, do not start the run. A run may start with no proxy, and egress is not pinned. `--require-proxy` (or `SKILLLITE_AGENT_SANDBOX_REQUIRE_PROXY=1`) refuses to start when the proxy is missing.
5. Resolve the command to an absolute path on the host, then pass that path into the sandbox. The binary's directory is added to the sandbox `PATH`. When no other command is supplied, run:

```text
claude -p "<prompt>" --output-format stream-json --verbose --permission-mode dontAsk
codex exec --json --dangerously-bypass-approvals-and-sandbox "<prompt>"
```

The Codex command turns off Codex's own sandbox. The outer bwrap or Seatbelt profile still limits writes. When a proxy is set, egress still goes only to that proxy.

6. On exit, cancel, or error: kill the process group, write Codex `auth.json` back, delete the ephemeral home, release the lock, and set `busy` back to false.

Linux uses a `bwrap` profile built for `run`: user, mount, PID, UTS, and IPC namespaces; hostname set to this run id; no network namespace and no dedicated NIC; read-only `/usr`, `/lib`, `/bin`, and `/lib64` when those paths exist; ephemeral home at `/home/agent`; `/tmp` on tmpfs; no host home, Skilllite config directory, or container sockets; non-root. The proxy allow rule is attached to this process's user or cgroup.

macOS uses a Seatbelt profile specific to `run`, via `sandbox-exec`. `HOME` is the ephemeral home. Reads stay broad because a narrow path list prevents dyld from starting the process. Writes are limited to the ephemeral home and the work directory. The proxy allow rule is in that profile. There is no PID namespace, so Activity Monitor can still show another subscription's process. Subscription directories and egress IPs stay per subscription.

## Out of scope

Evotown dispatch and its event log, a dedicated NIC, timezone changes from the proxy IP, locale mirroring, an OpenAI-compatible wrapper around a subscription, Windows, managing the pool inside the desktop app, and an SSE listener on the desktop or the CLI. Claude and Codex share the same `bwrap` and Seatbelt assembly.

## Acceptance (after implementation)

- After a user is bound to a `ready` subscription, a phone receives text and tool calls in order from the connect service SSE, then `done`.
- A phone message reaches the computer through the connect service. The computer does not open a port on the home network.
- Two different subscriptions running at once cannot read each other's ephemeral homes. On Linux, neither `ps` lists the other.
- After a run, the ephemeral home is gone. The subscription directory and the working directory remain, and that subscription's `busy` is false.
- With `--require-proxy`, a run without a proxy does not start. When a proxy is set, a connection to any other address fails.
- Desktop `connect` and CLI `connect` do not run on the same computer at the same time.
- Existing SkillLite skill-execution tests still pass.
