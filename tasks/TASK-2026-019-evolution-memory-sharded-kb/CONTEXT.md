# Technical Context

## Current State

- **Relevant crates/files**:
  - `crates/skilllite-evolution/src/memory_learner.rs` — parses LLM JSON, applies gatekeepers, writes shards and index markdown.
  - `crates/skilllite-evolution/src/snapshots.rs` — `create_extended_snapshot` / `restore_extended_snapshot` for prompts, memory tree, skills `_evolved`.
  - `crates/skilllite-agent/src/extensions/memory.rs` — `index_evolution_knowledge` (BM25 via `skilllite_executor::memory::index_file`).
  - `crates/skilllite-evolution/src/lib.rs` — extended snapshot integration test.
- **Current behavior (post-change)**:
  - Writes go to `memory/evolution/<dimension>/<YYYY-MM>.md`; indexes at `memory/evolution/<dimension>.md` and `INDEX.md`.
  - FTS paths remain relative to `memory/` (e.g. `evolution/entities/2026-04.md`).
  - `index_file` replaces chunks per `path`; bulk `DELETE ... WHERE path LIKE 'evolution/%'` clears stale evolution rows before reindex.

## Architecture Fit

- **Layer boundaries**: Evolution crate owns extraction and filesystem layout under chat root; agent crate owns hooking evolution content into the same memory FTS DB used by `memory_search` / `build_memory_context`.
- **Interfaces to preserve**: Changelog event type `memory_knowledge_added` and post-run `index_evolution_knowledge` call sites (`chat_session`, `skilllite-commands`) unchanged in contract.

## Dependency and Compatibility

- **New dependencies**: None.
- **Backward compatibility notes**:
  - Legacy `memory/evolution/knowledge.md` read for dedup only; not required for new writes.
  - Snapshot restore accepts pre-tree snapshots with flat `memory/knowledge.md` in the txn folder.

## Design Decisions

- **Decision**: Monthly shard key `YYYY-MM` (UTC) per dimension, separate files per dimension.
  - **Rationale**: Bounds single-file growth; aligns with existing five-way parsed schema; simple lexicographic sort for “latest month”.
  - **Alternatives considered**: Single rotating file per dimension without months; SQLite-only store.
  - **Why rejected**: User explicitly requested monthly volumes; SQLite would expand scope and tooling surface.

- **Decision**: Exclude navigational markdown from FTS.
  - **Rationale**: Reduces noise in search hits from tables and links in `INDEX.md` / `*.md` dimension indexes.
  - **Alternatives considered**: Index everything.
  - **Why rejected**: Indexes are redundant with shard content for factual retrieval.

- **Decision**: Full `evolution/%` FTS delete before reindex.
  - **Rationale**: Removes rows for deleted or renamed shards without maintaining a tombstone list.
  - **Alternatives considered**: Per-file incremental updates only.
  - **Why rejected**: Orphan paths would remain searchable; delete+reindex is simpler and tree size is small.

## Open Questions

- [ ] Optional one-shot migration from legacy `knowledge.md` body into shards (not in scope for TASK-2026-019).
- [ ] Whether to add `memory_vector` reindex parity for evolution paths (currently BM25-focused in `index_evolution_knowledge`).
