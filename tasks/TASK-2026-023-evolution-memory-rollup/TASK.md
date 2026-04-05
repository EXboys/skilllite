# TASK Card

## Metadata

- Task ID: `TASK-2026-023`
- Title: Evolution memory monthly rollup dedup
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-05`
- Target milestone:

## Problem

Memory evolution appends to monthly shards (`YYYY-MM.md`), producing many duplicate entity/relation lines across runs. Full-file rewrite would hurt auditability; the extractor’s “existing knowledge” tail was too small to prevent repeats.

## Scope

- In scope: Deterministic per-month **`YYYY-MM.rollup.md`** under each evolution dimension; regenerate after each successful memory write; prefer rollup tails in `build_existing_knowledge_summary`; dimension index table links rollups; EN/ZH architecture note + changelog.
- Out of scope: LLM-based compaction; new CLI subcommand; rewriting or deleting historical shard sections.

## Acceptance Criteria

- [x] After memory evolution writes a month shard, rollup files for that month are updated for all five dimensions (or stale rollup removed if shard missing).
- [x] Dedup is deterministic (stable sort keys; entity notes prefer longer / last-equal-length update).
- [x] `build_existing_knowledge_summary` includes latest-month rollup tail when present, before shard tail, within existing cap budget.
- [x] Dimension indexes list rollup links; root `INDEX.md` mentions rollup files.
- [x] Unit tests cover parsing, dedup, invalid month, and filesystem rollup write.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test` pass.

## Risks

- Risk: Parser mismatch if future prompts change bullet formats.
  - Impact: Rollup may drop or mis-merge lines until parser is updated.
  - Mitigation: Tests anchored on current formats; shards remain source of truth.

## Validation Plan

- Required tests: `cargo test -p skilllite-evolution`; full workspace `cargo test`.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: Optional — trigger evolution with memory scope and confirm `*.rollup.md` appears beside `YYYY-MM.md`.

## Regression Scope

- Areas likely affected: `skilllite-evolution` memory learner, evolution FTS file set (extra `.rollup.md` files indexed).
- Explicit non-goals: Changing extraction JSON schema or shard append format.

## Links

- Source TODO section: prior chat recommendation (append + rollup).
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `CHANGELOG.md`.
