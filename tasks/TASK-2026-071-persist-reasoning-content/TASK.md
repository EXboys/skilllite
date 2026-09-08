# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Persist assistant reasoning_content across transcript reloads
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-09-08`
- Target milestone:

## Problem

DeepSeek thinking-mode APIs require `reasoning_content` to be echoed on subsequent requests when `tools` are present. SkillLite keeps that field in memory during one agent turn, but `append_assistant_message` / `transcript_entry_to_message` drop it on disk. The next `skilllite chat` turn in the same session reloads history without the field and the API returns HTTP 400.

## Scope

- In scope:
  - Persist optional `reasoning_content` on transcript assistant `Message` rows.
  - Reload it into `ChatMessage` so OpenAI-compatible request serialization can echo it.
  - Regression tests for persist/reload and outbound JSON.
- Out of scope:
  - Persisting intermediate tool-call assistant rows (already in-memory for a single turn).
  - Session-key path validation (covered by open PR #126).
  - Desktop UI display of thinking text.

## Acceptance Criteria

- [x] Assistant `reasoning_content` written to today's transcript `Message` row when present.
- [x] `read_history` / `transcript_entry_to_message` restores the field on assistant messages.
- [x] Outbound OpenAI JSON includes `reasoning_content` when the reloaded message has it.
- [x] Legacy transcript rows without the field still load (`None`).

## Risks

- Risk: Transcript schema change breaks older readers.
  - Impact: Desktop/CLI fail to parse history.
  - Mitigation: Optional serde field with default; unknown fields already ignored by tagged enum.
- Risk: Attaching last-assistant reasoning to a synthesized incomplete-task response.
  - Impact: Provider may ignore mismatched pair; still better than omitting the field.
  - Mitigation: Use last assistant message reasoning only; keep content as today.

## Validation Plan

- Required tests: executor serde round-trip; agent history reload; OpenAI payload echo.
- Commands to run:
  - `cargo test -p skilllite-executor --lib`
  - `cargo test -p skilllite-agent`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Trace `run_turn_inner` → persist → `read_history` → `openai_api_message`.

## Regression Scope

- Areas likely affected: chat transcript persist/reload, DeepSeek thinking multi-turn.
- Explicit non-goals: session_key sanitization, tool_call Message persistence, docs/env changes.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-09-08.
- Related PRs/issues: Do not re-open #126 (session_key path escape).
- Related docs: DeepSeek thinking-mode tool-call passback requirement.
