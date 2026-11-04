# TASK Card

## Metadata

- Task ID: `TASK-2026-075`
- Title: Execute canonical agent binaries inside the sandbox
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-10-29`
- Target milestone:

## Problem

`skilllite agent-sandbox` resolves a command with `which` and executes that path. The official Codex standalone install is `~/.local/bin/codex` → `~/.codex/packages/standalone/current/codex`, and `current` is another symlink into `releases/<version>/bin/codex`. The sandbox mounts the launcher directory and the final directory, then execs the symlink. The intermediate `current` link is not in the mount namespace, so bwrap returns `No such file or directory` and the run never starts.

## Scope

- In scope:
  - Execute the canonical file.
  - Keep the symlink directory mounted read-only and first on `PATH` when it differs from the canonical directory.
  - Regression tests for the multi-hop layout and for a sibling `node` shebang.
  - EN/ZH architecture and home-node wording.
- Out of scope:
  - Mounting all of `~/.codex`.
  - Changing Codex argv, auth writeback, or proxy enforcement.

## Acceptance Criteria

- [x] A multi-hop symlink with the Codex standalone shape runs inside bwrap and prints its argument.
- [x] A `#!/usr/bin/env node` script still finds `node` beside the original symlink.
- [x] A regular file does not gain a second launcher directory.
- [x] The launcher directory is read-only. No new writable root and no network change.

## Risks

- Risk: Putting the launcher directory on `PATH` could expose extra binaries.
  - Impact: The process could invoke another program that lives next to the symlink.
  - Mitigation: That directory was already the `binary_prefix` mount. It stays read-only. The canonical directory was already mounted by `bind_canonical_target`.

## Validation Plan

- Required tests: `cargo fmt --check`, `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`, `cargo test -p skilllite-sandbox`, `cargo test -p skilllite --test e2e_minimal`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: bwrap repro of the failing symlink path before the fix

## Regression Scope

- Areas likely affected: `skilllite agent-sandbox` command resolution on Linux and macOS
- Explicit non-goals: auth.json writeback, nftables, Seatbelt write roots

## Links

- Source TODO section:
- Related PRs/issues: follows `TASK-2026-074`
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `docs/en/HOME-NODE-CONNECT.md`, `docs/zh/HOME-NODE-CONNECT.md`
