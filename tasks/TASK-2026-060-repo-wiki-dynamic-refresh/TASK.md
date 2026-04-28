# TASK Card

## Metadata

- Task ID: `TASK-2026-060`
- Title: Repo Wiki dynamic refresh
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

Repo Wiki compile is currently manual. Users can ingest or edit raw sources and then query stale articles unless they remember to run `skilllite wiki compile`.

## Scope

- In scope: Add source fingerprint metadata, `wiki status`, automatic compile after ingest, automatic stale refresh before query, and skip flags for explicit control.
- Out of scope: Background file watcher, chat/agent automatic writes, memory integration, SQLite/vector indexes, URL crawling, or changes to skills discovery.

## Acceptance Criteria

- [x] Compiled articles record source fingerprints derived from raw content.
- [x] `skilllite wiki status` reports up-to-date, stale, missing, and uncompiled raw source states without mutating files.
- [x] `wiki ingest` compiles by default and supports `--no-compile`.
- [x] `wiki query` refreshes stale wiki articles before searching and supports `--no-compile`.
- [x] Existing manual `wiki compile` remains available.
- [x] No watcher, memory, SQLite, `.skills`, or `.gitignore` changes are introduced.

## Risks

- Risk: Automatic compile changes Markdown during commands that previously only ingested or queried.
  - Impact: Users may see additional `wiki/` diffs after ingest/query.
  - Mitigation: Limit writes to ingest/query only, add `--no-compile`, and document behavior.

## Validation Plan

- Required tests: Unit tests for stale detection, status output model, ingest/query auto-refresh, and `--no-compile` behavior.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo test -p skilllite`, `python3 scripts/validate_tasks.py`.
- Manual checks: CLI help includes `status` and skip flags; docs describe only shipped dynamic behavior.

## Regression Scope

- Areas likely affected: `skilllite wiki` CLI and command implementation.
- Explicit non-goals: memory, skills, SQLite, background daemon/watchers, network access.

## Links

- Source TODO section: User asked how to guarantee LLM Wiki stays dynamically updated and then requested execution.
- Related PRs/issues:
- Related docs: `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
