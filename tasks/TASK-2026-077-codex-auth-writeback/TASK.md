# TASK Card

## Metadata

- Task ID: `TASK-2026-077`
- Title: Keep a newer host Codex auth.json when the sandbox copy is unchanged
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-02`
- Target milestone:

## Problem

`persist_codex_auth` copied the ephemeral `auth.json` back onto the subscription directory after every Codex sandbox run, including when the sandbox never modified the file. The official Codex CLI refreshes that same file during a normal session. A long `agent-sandbox --runtime codex` run could therefore replace a newer host login with the stale snapshot taken at start and drop the user out of the subscription.

## Scope

- In scope: skip writeback when the sandbox `auth.json` bytes still match the snapshot taken after the copy; take Codex's `auth.lock` when a changed regular file is written back; regression test; EN/ZH docs.
- Out of scope: multi-hop binary symlink execution (TASK-2026-075), unproxied DNS mounts (TASK-2026-076), merging two concurrent token refreshes, and Claude credential writeback.

## Acceptance Criteria

- [x] An unchanged sandbox `auth.json` does not replace a newer host `auth.json` and does not create `auth.lock`.
- [x] A changed regular `auth.json` is still written back, including non-ASCII contents.
- [x] A symlink `auth.json` in the ephemeral home is still refused and does not create the host file.
- [x] EN and ZH architecture and home-node docs describe the unchanged-file behavior.

## Risks

- Risk: a sandbox refresh and a host refresh in the same run can still end with one of the two files winning.
  - Impact: one of the two rotated sessions can be the file that remains.
  - Mitigation: unchanged snapshots no longer clobber the host. A changed file still writes back under `auth.lock`, which is the existing refresh contract. Merging two live refreshes is out of scope.

## Validation Plan

- Required tests: `cargo test -p skilllite-sandbox unchanged_codex_auth_does_not_replace_a_newer_host_file`
- Commands to run: `cargo fmt --check`, `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`, `cargo clippy -p skilllite --all-targets -- -D warnings`, `cargo test -p skilllite-sandbox`, `cargo test -p skilllite --test e2e_minimal`
- Manual checks: confirm the skip path does not create `auth.lock`.

## Regression Scope

- Areas likely affected: `skilllite agent-sandbox --runtime codex` auth writeback.
- Explicit non-goals: proxy nftables, bwrap mounts, Claude login files, and skill `execute()`.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `docs/en/HOME-NODE-CONNECT.md`, `docs/zh/HOME-NODE-CONNECT.md`
