# TASK Card

## Metadata

- Task ID: `TASK-2026-074`
- Title: Run Codex in the subscription sandbox
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-10-28`
- Target milestone:

## Problem

`skilllite_sandbox::run` only accepts a `.claude` directory and, when `argv` is empty, always starts Claude Code. A Codex subscription lives in `.codex`, uses a different command, and refreshes `auth.json` during the run. That refresh is lost if the ephemeral copy is deleted.

## Scope

- In scope:
  - `--runtime claude|codex` (default `claude`).
  - Copy `.claude` or `.codex` into the ephemeral home.
  - Codex default command: `codex exec --json --dangerously-bypass-approvals-and-sandbox <prompt>`.
  - After a Codex run, copy a regular `auth.json` back to the subscription directory.
  - Redact `sk-` tokens, covering `sk-ant-` and `sk-proj-`.
  - EN/ZH architecture and home-node docs.
- Out of scope:
  - The subscription pool, user binding, and SSE consumer.
  - Parsing Codex JSONL into the Claude `stream-json` shape.
  - Windows.

## Acceptance Criteria

- [x] Default runtime stays Claude and still refuses a directory without `.claude`.
- [x] `--runtime codex` requires `.codex` and does not require `.claude`.
- [x] Empty `argv` for Codex is `codex exec --json --dangerously-bypass-approvals-and-sandbox <prompt>`.
- [x] A Codex run writes refreshed `auth.json` back and leaves other files in `.codex` unchanged.
- [x] A symlink `auth.json` is not copied back.
- [x] A Claude run does not write the login directory back.
- [x] `sk-proj-` and `sk-ant-` redact, including a line that also contains non-ASCII text.

## Risks

- Risk: Disabling Codex's own sandbox is broader than Codex's default read-only sandbox.
  - Impact: The process can write inside the outer sandbox's writable roots. With no proxy, egress is not pinned.
  - Mitigation: The flag is the documented mode for an externally sandboxed process. bwrap or Seatbelt still limits writes. A set `https_proxy` still pins egress. Claude already runs with `dontAsk` under the same outer limits.
- Risk: Writing `auth.json` back changes the "do not write the subscription directory" rule.
  - Impact: A compromised process could replace that one file.
  - Mitigation: Only a regular file is copied, after the process exits, while the directory lock is held. Symlinks are refused. Other files are not written back.

## Validation Plan

- Required tests: `cargo test -p skilllite-sandbox`, `cargo test -p skilllite --test e2e_minimal`, `cargo fmt --check`, `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`, `cargo clippy -p skilllite --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md after execution
- Manual checks: none on a live `codex` binary. The sandbox tests use `/bin/sh` to stand in for the process.

## Regression Scope

- Areas likely affected: `skilllite agent-sandbox` and `skilllite_sandbox::run`
- Explicit non-goals: pool, binding, SSE event translation, skill `execute()`

## Links

- Source TODO section: `docs/zh/HOME-NODE-CONNECT.md` sandbox section
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `docs/en/HOME-NODE-CONNECT.md`, `docs/zh/HOME-NODE-CONNECT.md`
