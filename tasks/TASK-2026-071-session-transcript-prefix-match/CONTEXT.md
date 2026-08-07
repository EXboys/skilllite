# CONTEXT

## Technical Context

- Transcript naming: `{session_key}-YYYY-MM-DD.jsonl` (legacy `{session_key}.jsonl`).
- Plan naming: `{session_key}-{date}.jsonl` / `.json`.
- Desktop `list_transcript_paths` already used `format!("{}-", session_key)`; executor lagged and used bare prefix.
- Callers: `ChatSession::read_history_entries_incremental`, `chat_data` history tool, plan listing helpers.

## Constraints

- Do not change on-disk naming conventions.
- Keep UTF-8 path handling unchanged.
- Avoid Tauri/frontend dependency churn beyond the small desktop filter alignment/test.

## Compatibility Notes

- No env/command/docs surface change.
- Behavior becomes stricter only for incorrectly matched sibling-prefix keys (desired).

## Open Questions

- None for this fix. Separate finding: evolution `dedupe_key` UNIQUE blocks re-queue after `executed`.
