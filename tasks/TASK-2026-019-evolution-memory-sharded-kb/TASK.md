# TASK Card

## Metadata

- Task ID: `TASK-2026-019`
- Title: Shard evolution knowledge by category and month
- Status: `done`
- Priority: `P2`
- Owner: `TBD`
- Contributors:
- Created: `2026-04-03`
- Target milestone:

## Problem

Single-file `memory/evolution/knowledge.md` grew without structure; five knowledge types were merged into one stream, which is hard to browse and scales poorly.

## Scope

- In scope: Write evolution memory knowledge under `memory/evolution/` by dimension (`entities/`, `relations/`, `episodes/`, `preferences/`, `patterns/`) with monthly shards (`YYYY-MM.md`); per-dimension index markdown; root `INDEX.md`; dedup summary from legacy file + latest shard tails; FTS reindex of evolution content (excluding navigational md); extended snapshot/restore for whole `memory/evolution/` with legacy snapshot path compatibility.
- Out of scope: Moving canonical store to workspace `knowledge/`; semantic/vector reindex for evolution (unchanged: BM25 path only here).

## Acceptance Criteria

- [x] New writes use monthly shards per dimension and refresh `entities.md` … `patterns.md` plus `INDEX.md`.
- [x] Legacy `knowledge.md` still contributes to `existing_knowledge_summary` when present.
- [x] `index_evolution_knowledge` clears stale `evolution/*` FTS rows and indexes shard/content files only (skips `INDEX.md` and dimension index files).
- [x] Extended snapshot copies/restores `memory/evolution/` tree; restore supports old `snap/.../memory/knowledge.md` layout.
- [x] `cargo test -p skilllite-evolution -p skilllite-agent` and `cargo clippy -p skilllite-evolution -p skilllite-agent -- -D warnings` pass.

## Risks

- Risk: Users rely on old snapshot path only.
  - Impact: Restore might miss memory if snapshot predates tree layout and used flat file only — mitigated by legacy branch.
- Risk: FTS delete `evolution/%` briefly empties evolution hits until reindex completes.
  - Impact: Narrow window during `index_evolution_knowledge`; acceptable.

## Validation Plan

- Required tests: `skilllite-evolution` extended snapshot test updated for sharded layout; existing agent tests.
- Commands to run: `cargo test -p skilllite-evolution -p skilllite-agent`; `cargo clippy -p skilllite-evolution -p skilllite-agent -- -D warnings`.
- Manual checks: Run one memory evolution cycle locally and inspect `~/.skilllite/chat/memory/evolution/` layout (optional).

## Regression Scope

- Areas likely affected: `evolve_memory`, evolution snapshots/restore, memory FTS indexing, changelog UX strings.
- Explicit non-goals: Prompt/schema redesign for new categories; automatic migration of old `knowledge.md` body into shards.

## Links

- Source requirement: interactive product/design discussion (2026-04-03) — sharded evolution knowledge under `memory/evolution/`, monthly volumes, per-dimension and root indexes; English interpretation recorded in `PRD.md`.
- Strategic backlog (related, not source of this task): `todo/06-OPTIMIZATION.md` (evolution crate size / modularization).
- Related PRs/issues: (none filed)
- Related docs: `CHANGELOG.md` [Unreleased]; implementation in `memory_learner.rs`, `snapshots.rs`, `extensions/memory.rs`.
