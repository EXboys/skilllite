# PRD

## Background

Memory evolution (`evolve_memory`) previously appended all extracted knowledge (entities, relations, episodes, preferences, patterns) into a single `memory/evolution/knowledge.md`. That file grows monotonically, mixes five semantic dimensions in one stream, and is harder to browse, diff, and reason about. Users asked to keep the same chat-root location (`memory/evolution/`) but **shard by dimension and month**, with **per-dimension index files** and a **root index** for navigation, while preserving retrieval (FTS) and rollback (evolution snapshots).

## Objective

- Persist new evolution knowledge in a **structured directory layout** with **monthly shards per dimension** and **maintained markdown indexes**.
- Preserve **backward compatibility** for legacy `knowledge.md` (dedup input) and **legacy snapshot restore** where only `memory/knowledge.md` exists in a txn snapshot.
- Keep **memory search** correct: evolution content remains discoverable via BM25 after writes, without indexing pure navigation files.

## Functional Requirements

- FR-1: For each non-empty dimension after a successful extraction run, append a timestamped section to `memory/evolution/<dimension>/<YYYY-MM>.md` where `<dimension>` is one of `entities`, `relations`, `episodes`, `preferences`, `patterns`.
- FR-2: After writes, regenerate `memory/evolution/<dimension>.md` for each dimension listing available monthly shard files (human-readable index table).
- FR-3: After writes, regenerate `memory/evolution/INDEX.md` summarizing all five dimensions with links to dimension indexes and shard directories, including a **last updated** timestamp.
- FR-4: `existing_knowledge_summary` for the extraction prompt must incorporate (a) a tail of legacy `knowledge.md` if present and (b) tails of the **latest monthly shard per dimension** when present, bounded by the existing character cap.
- FR-5: `index_evolution_knowledge` must reindex evolution markdown used for retrieval: **exclude** `INDEX.md` and the five dimension index files (`entities.md`, …); **include** shards, legacy `knowledge.md`, and other `evolution/*.md` content files (e.g. structured experience topics).
- FR-6: Extended evolution snapshot must backup and restore the **entire** `memory/evolution/` tree; restore must support **legacy** snapshots that only contain `.../memory/knowledge.md` by copying it to `memory/evolution/knowledge.md`.

## Non-Functional Requirements

- **Security**: All writes remain under chat-root paths allowed by evolution L1 gatekeeper; L3 content gate still applies before persistence.
- **Performance**: FTS reindex may delete all `evolution/%` rows then repopulate; acceptable for typical evolution tree size; avoid unbounded full-repo scans.
- **Compatibility**: No requirement to auto-migrate historical body of `knowledge.md` into shards in this task; old file may coexist and remain in dedup summary.

## Constraints

- **Technical**: Rust implementation in `skilllite-evolution` (writer) and `skilllite-agent` (FTS indexer); snapshot helpers in `skilllite-evolution::snapshots`.
- **Timeline**: Delivered as a single cohesive change set with tests and changelog entry.

## Success Metrics

- **Metric**: Layout correctness after a memory evolution run (five dirs + indexes present when applicable).
- **Baseline**: Single-file `knowledge.md` only.
- **Target**: Sharded files + indexes; tests green; clippy clean on touched crates.

## Rollout

- **Rollout plan**: Ship with next release; users gain new layout on next successful `evolve_memory` run; existing `knowledge.md` untouched unless still present.
- **Rollback plan**: Revert code release; on-disk files may remain sharded—operators may manually consolidate if needed (out of scope for automated rollback).
