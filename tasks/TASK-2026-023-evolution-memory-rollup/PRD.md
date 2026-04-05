# PRD

## Goal

Provide human-readable, deduplicated views of evolution memory shards per month without replacing append-only audit logs.

## Requirements

1. **Rollup artifact**: For each dimension (`entities`, `relations`, `episodes`, `preferences`, `patterns`), maintain `memory/evolution/<dim>/YYYY-MM.rollup.md` derived from `YYYY-MM.md`.
2. **Trigger**: Rebuild rollups automatically after a successful memory evolution write for the active month.
3. **Dedup**: Merge lines that represent the same key (e.g. entity name + type case-insensitive); prefer richer text where applicable.
4. **Extraction context**: Feed rollup content into the existing-knowledge summary so the LLM sees consolidated recent knowledge first.
5. **Discoverability**: Dimension indexes and root evolution index mention rollup files.

## Non-goals

- Replacing shards with rollups as the only storage.
- Cross-month consolidation in this iteration.

## Success criteria

Rollups reduce visible repetition for operators; duplicate extraction rate should drop when rollup tails are present (not formally measured in CI).
