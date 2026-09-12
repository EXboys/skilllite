# TASK Card

## Metadata

- Task ID: `TASK-2026-077`
- Title: Harden executor memory rel_path validation
- Status: `done`
- Priority: `P0`
- Owner: `critical-bug-automation`
- Contributors: `critical-bug-automation`
- Created: `2026-08-01`
- Target milestone: critical bug sweep

## Problem

Executor stdio RPC `memory_write` joins caller-controlled `rel_path` under
`<chat_root>/memory/` with only a weak string check (`contains("..")` /
`starts_with('/')`). On Windows, drive-absolute and backslash-rooted values
(e.g. `C:\Temp\pwn.md`, `\Windows\Temp\pwn.md`) pass that check and
`Path::join` replaces the memory root, writing outside the chat tree.

PR #128 fixed `agent_id` escapes and explicitly deferred this `rel_path`
hardening.

## Scope

- In scope:
  - Shared `validate_memory_rel_path` / `memory_file_under_dir` in `skilllite-core`
  - Fail-closed wiring in executor `handle_memory_write` and markdown reindex
  - Regression tests for drive / backslash / traversal / absolute / valid nested paths
- Out of scope:
  - Open PR #128 `agent_id` changes (land separately)
  - Agent builtin `read_file`/`write_file` symlink follow
  - Other open critical PRs (#89, #112–#116, #120–#127)

## Acceptance Criteria

- [x] Windows drive-absolute and backslash-rooted `rel_path` values are rejected on all hosts
- [x] Unix absolute and `..` traversal `rel_path` values remain rejected
- [x] Legitimate nested relatives such as `notes/day.md` still write under `memory/`
- [x] Executor RPC returns a validation error instead of writing outside the memory root
- [x] Targeted tests + fmt/clippy for touched crates pass

## Risks

- Risk: Over-strict rejection of unusual but previously accepted relative paths (e.g. containing `\`)
  - Impact: Hostile/malformed paths fail closed; normal `/`-separated memory notes unchanged
  - Mitigation: Keep nested `/` paths; mirror artifact-key Windows rules; add accept/reject tests

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core path_validation`
  - `cargo test -p skilllite-executor --lib`
- Commands to run:
  - `cargo fmt --all -- --check`
  - `cargo clippy -p skilllite-core -p skilllite-executor --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read wired call sites to confirm validation runs before `create_dir_all` / write

## Regression Scope

- Areas likely affected:
  - Executor `memory_write` RPC
  - Memory markdown reindex path filtering
- Explicit non-goals:
  - Symlink follow in workspace file tools
  - Open session_key / agent_id / artifact PRs

## Links

- Source TODO section: critical bug automation cron
- Related PRs/issues: follow-up to PR #128; same class as PR #123
- Related docs: N/A (fail-closed validation only)
