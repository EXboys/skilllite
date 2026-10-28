# TASK Card

## Metadata

- Task ID: TASK-2026-071-agent-sandbox-run
- Title: Sandbox `run` for one subscription directory
- Status: `done`
- Priority: `P1`
- Owner: agent
- Contributors:
- Created: 2026-10-28
- Target milestone:

## Problem

A private pool host needs one isolated process per subscription directory. Skill execution (`execute`) stays offline and short-lived. The new path must not reuse `--unshare-all`, the skill timeout/memory defaults, or the seccomp filter that blocks `AF_UNIX`.

## Scope

- In scope:
  - `skilllite_sandbox::run` copies `.claude` into an ephemeral home, locks the subscription directory, forces `https_proxy`, streams redacted stdout, and deletes the ephemeral home.
  - Linux `bwrap` plan uses user, mount, PID, UTS, and IPC namespaces only.
  - macOS uses a separate Seatbelt profile.
  - CLI `agent-sandbox` on the existing binaries.
  - EN/ZH architecture notes and the draft doc status for this slice.
- Out of scope:
  - Subscription pool, user binding, connect service, SSE, Evotown, team-edition changes.
  - Changes to skill `execute()`.

## Acceptance Criteria

- [x] With `require_proxy` set, a missing `https_proxy` does not start a process. With it unset, a run may start without a proxy. A provided proxy that does not resolve still does not start.
- [x] A second `run` on the same subscription directory fails while the first holds the lock.
- [x] Stdout containing `sk-ant-` is redacted, including when the line also contains non-ASCII text.
- [x] Ephemeral home is removed after the run. The subscription directory and work directory remain.
- [x] Linux argv plan has no `--unshare-all` and no `--unshare-net`.
- [x] Existing skill execution tests still pass.

## Risks

- Risk: Linux destination filtering needs nftables and a private cgroup.
  - Impact: `run` refuses to start when that enforcement cannot be installed.
  - Mitigation: Fail closed. macOS enforces the same allowlist in Seatbelt, which is what local development runs.

## Validation Plan

- Required tests: `cargo test -p skilllite-sandbox`, `cargo test -p skilllite --test e2e_minimal`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: none beyond the tests

## Regression Scope

- Areas likely affected: `skilllite-sandbox` public API and the CLI command enum
- Explicit non-goals: skill `execute()` argv, seccomp, and default sandbox level

## Links

- Source TODO section: `docs/zh/HOME-NODE-CONNECT.md` sandbox section
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
