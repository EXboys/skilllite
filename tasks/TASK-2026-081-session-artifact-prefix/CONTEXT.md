# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-executor/src/transcript.rs` (`list_transcript_files`, used by chat history and `archive_transcript`)
  - `crates/skilllite-executor/src/plan.rs` (`list_plan_files`)
  - `skilllite-assistant/src-tauri/src/skilllite_bridge/transcript.rs` (`list_transcript_paths`)
  - `skilllite-assistant/src-tauri/src/skilllite_bridge/sessions.rs` (plan deletion on session delete)
  - `skilllite-assistant/src-tauri/src/skilllite_bridge/workspace.rs` (default-session plan fallback)
- Current behavior: a filename matched when it started with the session key. `schedule-daily` matched `schedule-daily-brief-2026-11-25.jsonl`.

## Architecture Fit

- Layer boundaries involved: executor session storage, and the desktop bridge which shells out for chat but reads transcript files itself. No new crate dependency.
- Interfaces to preserve: `list_transcript_files` and `list_plan_files` signatures. Dated files remain `{session_key}-YYYY-MM-DD.{jsonl,json}`.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: legacy `{session_key}.jsonl` is still listed once and sorts as the oldest file. Archived names that do not end in `.json` or `.jsonl` stay untouched.

## Design Decisions

- Decision: accept a file only when the stem equals the key, or the stem is `{key}-` plus an ASCII `YYYY-MM-DD` stamp.
  - Rationale: the date is the only suffix the writers append. Checking that stamp rejects longer keys without forbidding hyphens inside the key (`schedule-daily-brief`, `s-{hex}`).
  - Alternatives considered: require the next character after the key to be `-` and then take the rest as opaque.
  - Why rejected: `schedule-daily-` is also a prefix of `schedule-daily-brief-2026-11-25`, so a bare hyphen check still crosses sessions.

## Open Questions

- [x] Desktop and executor predicates stay duplicated because the assistant crate does not link `skilllite-executor`.
- [x] Docs sync is not required. This restores the existing per-session file contract. No command, env var, or architecture change.
