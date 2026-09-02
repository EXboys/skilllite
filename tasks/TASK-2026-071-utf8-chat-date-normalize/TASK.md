# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: UTF-8 safe chat history date normalization
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-09-02`
- Target milestone:

## Problem

`chat_history` and `chat_plan` normalize optional `date` arguments by stripping `-` and, when the remaining string is exactly 8 bytes, slicing at byte indexes 4 and 6. Localized 8-byte dates such as `2026年9` or `2026-9月` land those indexes inside a multibyte code point and panic, crashing the agent/chat process (no `catch_unwind` on the builtin tool path).

## Scope

- In scope: make `normalize_date` treat only 8 ASCII digits as `YYYYMMDD`; add non-ASCII regression tests; keep ISO compact dates working.
- Out of scope: session_key path sanitization (#126), broader date-parser support, UI/docs changes, other UTF-8 panic drafts (#92–#96).

## Acceptance Criteria

- [x] `normalize_date("20260902")` and `normalize_date("2026-09-02")` still produce `2026-09-02`.
- [x] Localized 8-byte dates (`2026年9`, `2026-9月`) do not panic and pass through unchanged.
- [x] `execute_chat_history` / `execute_chat_plan` return `Ok` for those dates instead of aborting the process.
- [x] Focused `skilllite-agent` tests, `cargo fmt`, clippy `-D warnings`, workspace `cargo test`, and `python3 scripts/validate_tasks.py` pass.

## Risks

- Risk: changing which 8-byte strings are rewritten as ISO dates.
  - Impact: a non-digit 8-byte string that previously panicked would now be used as a literal filename fragment and miss.
  - Mitigation: only 8 ASCII digits were a valid `YYYYMMDD` compact form; miss is the correct outcome vs a crash.

## Validation Plan

- Required tests: unit tests in `crates/skilllite-agent/src/extensions/builtin/chat_data.rs`.
- Commands to run:
  - `cargo test -p skilllite-agent normalize_date -- --nocapture`
  - `cargo test -p skilllite-agent`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks: re-read modified files and task artifacts after edits.

## Regression Scope

- Areas likely affected: `chat_history` and `chat_plan` date lookup filenames.
- Explicit non-goals: transcript path sanitization, compaction, session-key joins, evolution status UTF-8 (#96).

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
