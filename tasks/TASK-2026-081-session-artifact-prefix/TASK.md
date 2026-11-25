# TASK Card

## Metadata

- Task ID: `TASK-2026-081`
- Title: Keep session transcript and plan files inside their session key
- Status: `in_review`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-25`
- Target milestone:

## Problem

Transcript and plan filenames are `{session_key}-YYYY-MM-DD.jsonl`. Listing and deletion matched any filename that merely started with the session key. A shorter key such as `schedule-daily` therefore included `schedule-daily-brief-2026-11-25.jsonl`.

That mixes the longer session into chat history, and `clear-session` / desktop delete renames or removes the longer session's files.

## Scope

- In scope:
  - Match a file only when it is `{session_key}.jsonl` / `{session_key}.json`, or `{session_key}-YYYY-MM-DD` with one of those extensions.
  - Apply that match to executor transcript listing, executor plan listing, assistant transcript listing, assistant session plan deletion, and the assistant default-session plan fallback.
  - Regression tests for a shorter key, the longer key, and a non-ASCII key.
- Out of scope:
  - Changing the filename layout.
  - Renaming existing session keys.
  - Archived `*.jsonl.archived.*` files.

## Acceptance Criteria

- [x] `schedule-daily` does not see or select `schedule-daily-brief-YYYY-MM-DD.jsonl`.
- [x] `schedule-daily-brief` still sees its own dated file.
- [x] A non-ASCII key (`日程`) matches its own dated file and not a longer key (`日程表`).
- [x] An empty session key matches nothing.
- [x] Legacy `{session_key}.jsonl` is still included once.

## Risks

- Risk: A session key that itself ends in `-YYYY-MM-DD` could be ambiguous with the date suffix.
  - Impact: A file could be attributed to the shorter key that ends before the date.
  - Mitigation: The on-disk contract is `{key}-YYYY-MM-DD`. Keys are `default`, `s-{millis hex}`, or `schedule-{job id}`. The date stamp is checked only as the entire remainder after one hyphen.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-executor --lib longer_session_key`
  - `cargo test -p skilllite-executor --lib`
  - `cargo clippy -p skilllite-executor --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
- Commands to run: recorded in `STATUS.md` after execution.
- Manual checks: none. The trigger is covered by the unit tests.

## Regression Scope

- Areas likely affected:
  - Chat history load and compaction input (`list_transcript_files`).
  - `clear-session` archive rename.
  - Desktop session preview, reload, and delete.
  - Plan file listing and the desktop recent-plan fallback.
- Explicit non-goals:
  - Sandbox, auth, and artifact store behavior.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: filename contract in `crates/skilllite-executor/src/transcript.rs`
