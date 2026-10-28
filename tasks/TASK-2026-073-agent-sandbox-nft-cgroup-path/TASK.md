# TASK Card

## Metadata

- Task ID: `TASK-2026-073`
- Title: Fix Linux proxy nftables cgroup path
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-10-28`
- Target milestone:

## Problem

`skilllite_sandbox::run` installs an nftables `socket cgroupv2` rule using only the leaf cgroup name (`skilllite-<run>`). nftables 1.0.9 resolves that string as a path under `/sys/fs/cgroup`. On a nested hierarchy the leaf is not a child of the root, so `nft -f` rejects the ruleset and every Linux run that sets `https_proxy` fails closed with `proxy enforcement unavailable: nft rejected the allowlist`.

A rule that names the full relative path but uses the wrong level can still install. The output chain policy is accept, so a non-matching level does not drop other TCP or UDP.

## Scope

- In scope:
  - Generate the nftables match from the cgroup path relative to `/sys/fs/cgroup`.
  - Set `level` to the number of components in that path.
  - Refuse paths that cannot be placed in an nftables string.
  - EN/ZH architecture and home-node wording for that match.
- Out of scope:
  - macOS Seatbelt read scope.
  - Changing the output-chain policy to drop (that would affect every host process).
  - Skill `execute()` sandboxing.

## Acceptance Criteria

- [x] A nested cgroup path is quoted in full in the nftables script, and `level` equals the component count.
- [x] A leaf-only match string is not produced for a nested cgroup.
- [x] A cgroup outside `/sys/fs/cgroup`, or a path containing `"`, does not produce a script.
- [x] `cargo test -p skilllite-sandbox --lib agent_run` passes.
- [x] On this host, the generated rule form installs with nftables and drops TCP to a port other than the proxy port.

## Risks

- Risk: A wrong `level` installs and then fails open because the chain policy is accept.
  - Impact: The sandbox could reach any TCP or UDP destination while the operator believes the proxy is pinned.
  - Mitigation: `level` is the component count of the same relative path that nftables resolves. The unit test locks both.

## Validation Plan

- Required tests: `cargo test -p skilllite-sandbox --lib agent_run`, `cargo test -p skilllite-sandbox`, `cargo fmt --check`, `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: install the generated script with `nft -f` and probe an allowed port and a different local port from inside the cgroup

## Regression Scope

- Areas likely affected: Linux `agent-sandbox` runs that set `https_proxy`
- Explicit non-goals: runs without a proxy, macOS Seatbelt, skill `execute()`

## Links

- Source TODO section: `docs/en/HOME-NODE-CONNECT.md` sandbox section
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `docs/en/HOME-NODE-CONNECT.md`, `docs/zh/HOME-NODE-CONNECT.md`
