# PRD

## Background

Scheduled jobs default their session key to `schedule-{job id}`. The checked-in example uses `schedule-daily-brief`. A second job whose id is `daily` (or `daily-brief` plus `daily-brief-monday`) produces a key that is a string prefix of the other key. Chat history, clear, and delete all used that prefix.

## Objective

A session reads, archives, and deletes only the transcript and plan files that belong to that exact key.

## Functional Requirements

- FR-1: `{session_key}.jsonl` and `{session_key}.json` still belong to that session.
- FR-2: `{session_key}-YYYY-MM-DD.jsonl` and `{session_key}-YYYY-MM-DD.json` still belong to that session.
- FR-3: A file whose stem continues past the key before the date stamp does not belong to the shorter key.
- FR-4: An empty session key matches no filename.

## Non-Functional Requirements

- Security: prevents one session operation from deleting another session's transcript or plan.
- Performance: one extra date-shape check per directory entry.
- Compatibility: existing dated and legacy filenames for a single key stay visible.

## Constraints

- Technical: the desktop crate does not depend on `skilllite-executor`, so the filename predicate is duplicated there.
- Timeline: bugfix only. No filename migration.

## Success Metrics

- Metric: unit tests list only the exact session's files when a longer key is present.
- Baseline: before the fix, `starts_with(session_key)` included the longer key's file.

## Out of Scope

- Rewriting historical transcripts onto new names.
- Session-key validation for path separators.
