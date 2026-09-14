# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Persist kept recent messages after compaction
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-09-14`
- Target milestone:

## Problem

`compact_history_inner` and `/compact` (`force_compact`) summarize old turns and append a `Compaction` transcript marker, but they never rewrite the `SKILLLITE_COMPACTION_KEEP_RECENT` window after that marker. `read_history` only loads the summary plus entries after the last compaction, so the next turn drops the recent messages that were supposed to be kept (and `/compact` drops them immediately). `first_kept_entry_id` is always written as an empty string and is unused.

## Scope

- In scope:
  - Persist kept user/assistant messages after the compaction marker.
  - Populate `first_kept_entry_id` with the first rewritten kept message id.
  - Regression tests for persist + `read_history` reconstruction, including non-ASCII content.
  - Clarify ENV_REFERENCE (EN/ZH) so `SKILLLITE_COMPACTION_KEEP_RECENT` matches the persisted behavior.
- Out of scope:
  - Reloading tool_call / tool_result rows from transcript.
  - `--resume` tool-pair splitting.
  - Atomicity of memory markdown writes.
  - Claude consecutive-user merge (#156) and other open sweep drafts.

## Acceptance Criteria

- [x] After compaction, the next `read_history` call includes the compaction summary plus the kept recent user/assistant messages.
- [x] `/compact` / `force_compact` persist the kept window (not only an in-memory return value).
- [x] Session-clear compaction still writes a marker without rewriting recent turns.
- [x] Regression tests fail if kept messages are not rewritten after the marker.
- [x] Validation commands are actually run and recorded.

## Risks

- Risk: Rewriting kept messages duplicates rows that still exist before the marker.
  - Impact: Transcript files grow; UI/history tools that ignore compaction could show duplicates.
  - Mitigation: `read_history` and cache prune already ignore pre-marker rows; `chat_history` readers that walk the raw file already see pre-marker messages today.
- Risk: False positive — treating documented compaction as data loss.
  - Impact: Unnecessary PR.
  - Mitigation: Docs already promise KEEP_RECENT survives compaction; `first_kept_entry_id` exists but is unused; `/compact` has a concrete trigger.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent` compaction persist / history reconstruction tests; workspace `cargo test`.
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read modified files after edit.
  - Confirm session-clear path still appends compaction only.

## Regression Scope

- Areas likely affected: chat session compaction, `/compact`, pre-request context-budget compaction, next-turn `read_history`.
- Explicit non-goals: tool-row reload, Claude message coalescing, memory markdown atomic writes.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-09-14.
- Related PRs/issues: Distinct from #135 (transcript prefix) and #156 (Claude consecutive user).
- Related docs: `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md`.
