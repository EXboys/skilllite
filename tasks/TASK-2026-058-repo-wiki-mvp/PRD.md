# PRD

## Background

The project now has `.skilllite/wiki/` as a Repo Wiki root, but the current implementation only creates a skeleton. The user requested actual LLM Wiki-style command behavior while keeping the wiki Markdown-only and not using SQLite.

## Objective

Provide a deterministic Repo Wiki MVP with local-file ingestion, basic Markdown query, and linting under `.skilllite/wiki/`.

## Functional Requirements

- FR-1: Add a `skilllite wiki` CLI group with `init`, `ingest`, `query`, and `lint` subcommands.
- FR-2: Store ingested local files under `.skilllite/wiki/raw/` as Markdown with YAML frontmatter.
- FR-3: Maintain `_index.md` and `log.md` as Markdown.
- FR-4: Query must read wiki Markdown only and must not use SQLite or memory.
- FR-5: Lint must check required structure and basic frontmatter/index integrity.

## Non-Functional Requirements

- Security: Do not fetch remote URLs or auto-ingest files; local source paths must be explicit.
- Performance: Keep MVP filesystem scanning bounded to `.skilllite/wiki/`.
- Compatibility: Existing `skilllite init`, memory, and skills behavior must continue to work.

## Constraints

- Technical: Implement in `skilllite-commands`; wire from entry CLI without adding new crates.
- Timeline: Ship deterministic Markdown operations before LLM compile/research features.

## Success Metrics

- Metric: CLI commands work with local Markdown files and tests cover success/failure paths.
- Baseline: Only wiki skeleton exists.
- Target: Users can initialize, ingest, query, and lint a project Repo Wiki without SQLite.

## Rollout

- Rollout plan: Additive CLI subcommand group.
- Rollback plan: Remove the `wiki` command wiring/module; existing `.skilllite/wiki/` skeleton remains harmless Markdown.
