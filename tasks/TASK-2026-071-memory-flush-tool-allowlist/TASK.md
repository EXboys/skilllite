# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Restrict memory flush to memory tools only
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-08-11`
- Target milestone:

## Problem

Pre-compaction memory flush runs a full `agent_loop` under `SilentEventSink` with
the complete tool registry (builtins, skills, MCP). Non-key-path `write_file`
requires no confirmation, so a flush turn can silently overwrite workspace files.
Even after SilentEventSink denies `ConfirmRequired`, Low-tier `run_command` and
unconfirmed mutating tools remain available.

## Scope

- In scope:
  - Restrict memory-flush agent turns to memory tools only
  - Keep skills/MCP/builtins out of the flush registry
  - Regression tests for the restricted registry
- Out of scope:
  - ChatSession workspace chat-root alignment
  - Concurrent `sessions.json` RMW
  - SilentEventSink ConfirmRequired policy (separate open PR #138)

## Acceptance Criteria

- [ ] Memory flush builds a registry containing only `memory_search` / `memory_write` / `memory_list`
- [ ] Flush path does not register `write_file`, `run_command`, skills, or MCP tools
- [ ] Existing chat/agent turns keep full tool access
- [ ] Focused unit tests pass; agent crate tests pass

## Risks

- Risk: Flush LLM can no longer call helper tools (e.g. `read_file`) while summarizing
  - Impact: Slightly lower flush quality if model expected helpers
  - Mitigation: Prompt already instructs `memory_write` / `NO_REPLY` only; memory_list/search remain

## Validation Plan

- Required tests:
  - Registry unit test for memory-flush policy
  - `cargo test -p skilllite-agent`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-agent --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `python3 scripts/validate_tasks.py`
- Manual checks: N/A

## Regression Scope

- Areas likely affected:
  - Pre-compaction / early memory flush turns
  - Extension registry capability policies
- Explicit non-goals:
  - Swarm `run_single_task` tool surface (still uses SilentEventSink + full tools; #138 covers ConfirmRequired)

## Links

- Source TODO section: critical bug automation sweep 2026-08-11
- Related PRs/issues: follow-up to PR #138 SilentEventSink; deferred "memory-flush tool allowlist"
- Related docs: N/A
