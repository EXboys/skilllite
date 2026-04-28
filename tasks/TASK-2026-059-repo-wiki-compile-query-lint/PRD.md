# PRD

## Background

The Repo Wiki MVP can initialize, ingest, query, and lint Markdown, but it does not yet implement the LLM Wiki-style compile layer from immutable `raw/` sources into curated `wiki/` articles. It also needs query depth modes and stronger lint rules.

## Objective

Bring the Repo Wiki closer to LLM Wiki architecture while remaining deterministic, Markdown-only, and local-file based.

## Functional Requirements

- FR-1: Add `skilllite wiki compile` to generate/update article Markdown under `.skilllite/wiki/wiki/` from raw source Markdown.
- FR-2: Add query depth flags: `--quick` reads indexes, default reads articles/lessons/raw, and `--deep` includes all wiki Markdown.
- FR-3: Extend lint to validate canonical article frontmatter and source references.
- FR-4: Keep wiki storage independent from SQLite and memory.

## Non-Functional Requirements

- Security: Do not fetch URLs or ingest automatically; source files are explicit and local.
- Performance: Keep filesystem scans scoped to `.skilllite/wiki/`.
- Compatibility: Preserve existing `wiki` commands and previous skeleton layout.

## Constraints

- Technical: No new crates or dependency direction changes.
- Timeline: Deterministic compile/query/lint only; LLM compile/research is follow-up.

## Success Metrics

- Metric: Generated article frontmatter/source references are machine-checkable.
- Baseline: Raw ingest exists but no compile layer.
- Target: Compile/query/lint tests pass and docs describe the added commands.

## Rollout

- Rollout plan: Additive CLI behavior under `skilllite wiki`.
- Rollback plan: Revert `compile` and query/lint enhancements; raw wiki data remains Markdown.
