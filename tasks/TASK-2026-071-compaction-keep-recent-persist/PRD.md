# PRD

## Background

Conversation compaction is supposed to replace *old* turns with an LLM summary while keeping the most recent `SKILLLITE_COMPACTION_KEEP_RECENT` messages (default 10) for the next LLM call. The implementation only used that window for the in-memory return value of the compacting turn. Persistent history is reconstructed from the last `Compaction` marker forward, so the kept window vanished on the next turn — and immediately after `/compact`.

## Objective

- After automatic or manual compaction, subsequent turns still see the kept recent user/assistant messages plus the summary.
- Documented `SKILLLITE_COMPACTION_KEEP_RECENT` behavior matches runtime history.

## Functional Requirements

- FR-1: When compaction writes a transcript marker, rewrite kept user/assistant messages after that marker.
- FR-2: Set `first_kept_entry_id` to the first rewritten kept message id (empty when nothing is kept).
- FR-3: Session-clear / memory-summary compaction must not rewrite recent turns.
- FR-4: `read_history` continues to use summary + post-marker entries (no change to prune semantics).

## Non-Functional Requirements

- Security: No new path or sandbox surface.
- Performance: At most `keep_recent` extra appends per compaction (default 10).
- Compatibility: Older transcripts with empty `first_kept_entry_id` and no rewritten rows keep today's "summary only" behavior.

## Constraints

- Technical: Minimal change in `skilllite-agent` chat session; no crate graph change.
- Timeline: Same automation run.

## Success Metrics

- Metric: Next-turn history after `/compact` includes kept recent messages.
- Baseline: Only the summary is visible after the marker.
- Target: Summary + kept user/assistant rows.

## Rollout

- Rollout plan: Merge the fix; no migration (append-only transcript).
- Rollback plan: Revert the persist helper; old marker-only behavior returns.
