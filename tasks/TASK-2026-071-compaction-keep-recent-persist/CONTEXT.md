# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/chat_session.rs` (`compact_history_inner`, `force_compact`, `read_history`, `summarize_for_memory`)
  - `crates/skilllite-executor/src/transcript.rs` (`TranscriptEntry::Compaction.first_kept_entry_id`)
  - `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`
- Current behavior:
  - Compaction appends a marker with `first_kept_entry_id = ""` and returns `summary + recent` only to the caller.
  - `force_compact` discards that return value.
  - `read_history` loads the last compaction summary plus entries *after* the marker.
  - Cache prune drops all pre-marker entries.

## Architecture Fit

- Layer boundaries involved: agent session persistence; executor transcript types unchanged except using an existing field.
- Interfaces to preserve: `ChatSession::force_compact`, `run_turn` compaction, session-clear marker.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: transcripts compacted before this fix still lose the kept window (marker-only). New compactions persist the window.

## Design Decisions

- Decision: Rewrite kept user/assistant rows after the marker instead of teaching `read_history` to walk backward via `first_kept_entry_id`.
  - Rationale: Matches existing prune + `read_history` "after last compaction" semantics; `force_compact` starts working without a cache rewrite.
  - Alternatives considered: Change prune/read to honor `first_kept_entry_id` pointing at a pre-marker row.
  - Why rejected: Requires changing prune (which currently drains those rows) and is a larger, easier-to-get-wrong behavior change.

## Open Questions

- [x] Should tool-role rows be rewritten? No — default transcript history does not reload tool pairs; rewriting `role=tool` Message rows without `tool_call_id` would risk OpenAI 400s.
