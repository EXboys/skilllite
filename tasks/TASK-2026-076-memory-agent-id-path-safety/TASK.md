# TASK Card

## Metadata

- Task ID: `TASK-2026-076`
- Title: Reject path-escaping memory agent IDs
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-31`
- Target milestone:

## Problem

Stdio/executor `memory_write` / `memory_search` join unvalidated `agent_id` into
`memory/{agent_id}.sqlite`. Absolute IDs (e.g. `/tmp/pwn`) replace the memory
root via `Path::join`, creating or opening SQLite files outside the chat data
tree. Relative traversal IDs (e.g. `../../../tmp/pwn`) escape the same way.

## Scope

- In scope:
  - Single-segment validation for memory `agent_id`
  - Fail-closed `index_path` construction used by executor RPC and agent memory indexing
  - Regression tests for absolute / traversal / drive / separator forms
- Out of scope:
  - Open session_key hardening (PR #126)
  - Open evolution entry_point / skill_name PRs
  - Symlink follow bypasses in workspace file tools

## Acceptance Criteria

- [x] Absolute `agent_id` values are rejected before any SQLite open/create
- [x] Traversal / multi-segment / Windows-drive `agent_id` values are rejected
- [x] Valid single-segment IDs (e.g. `default`) still resolve under `chat/memory/`
- [x] Executor RPC `memory_write` / `memory_search` fail closed on bad IDs
- [x] Regression tests cover reject + accept paths

## Risks

- Call sites that previously built paths unconditionally must now handle `Result`
- Legitimate IDs containing `/` would break (none expected; IDs are session-like tokens)

## Validation Plan

- `cargo test -p skilllite-core path_validation`
- `cargo test -p skilllite-executor --lib`
- `cargo test -p skilllite-agent --lib`
- `cargo clippy -p skilllite-core -p skilllite-executor -p skilllite-agent --all-targets -- -D warnings`
- `cargo fmt --check`
- `python3 scripts/validate_tasks.py`

## Regression Scope

- Memory index path construction, stdio memory RPC, agent memory tools/indexing
- Does not address already-open critical PRs (#89, #112–#116, #120–#127)
