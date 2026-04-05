# CONTEXT

## Code touchpoints

- `crates/skilllite-evolution/src/evolution_memory_rollup.rs` — parse run sections (`## ...`), dedupe per dimension, write `YYYY-MM.rollup.md`, delete rollup when shard absent.
- `crates/skilllite-evolution/src/memory_learner.rs` — call `rebuild_rollups_for_month` after shard append; `build_existing_knowledge_summary` prefers rollup tail; `is_month_shard_stem` filters index months; dimension index gains third column.
- `crates/skilllite-evolution/src/lib.rs` — private `mod evolution_memory_rollup`.
- Docs: `CHANGELOG.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.

## Constraints

- Paths must pass `gatekeeper_l1_path` / `gatekeeper_l3_content`.
- Month keys must match `YYYY-MM` only (stem filter excludes `.rollup.md`).
- FTS (`index_evolution_knowledge`) picks up new `.rollup.md` files automatically; navigational `*.md` index files remain skipped.

## Compatibility

- Extended snapshots already copy `memory/evolution/`; rollups require no snapshot format change.
