# REVIEW

## Findings

- Executor transcript listing used bare `starts_with(session_key)` while dated files are `{key}-{date}.jsonl`, so prefix-sibling keys collided.
- Plan listing had the same class of bug.
- Desktop transcript loader was already correct; recent-plan fallback still used bare prefix for hardcoded `default`.

## Decision

- Merge readiness: ready (pending validation evidence in STATUS)

## Follow-ups

- Evolution backlog `dedupe_key` UNIQUE + `INSERT OR IGNORE` permanently drops same-scope proposals after first `executed` status (not fixed here).
