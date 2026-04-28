# TASK Card

## Metadata

- Task ID: `TASK-2026-059`
- Title: Repo Wiki compile query lint
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

TASK-2026-058 added deterministic Repo Wiki commands, but the implementation still lacks the core LLM Wiki flow where `raw/` sources are compiled into wiki articles, queries support depth modes, and lint verifies source provenance and article schema.

## Scope

- In scope: Add `skilllite wiki compile`; add `wiki query --quick|--deep`; strengthen article frontmatter/source/link linting; keep all behavior Markdown-only.
- Out of scope: URL fetching, multi-agent research, live web search, librarian quality scoring, project memory, SQLite, or changes to `.skills` discovery.

## Acceptance Criteria

- [x] `skilllite wiki compile` creates/updates `wiki/` articles from `raw/` sources with canonical frontmatter (`title`, `category`, `sources`, `created`, `updated`, `tags`, `aliases`, `confidence`, `summary`).
- [x] `skilllite wiki query` supports default standard mode plus `--quick` and `--deep`.
- [x] `wiki lint` checks required article frontmatter, dangling source references, and basic markdown links.
- [x] Existing `init`, `ingest`, `query`, and `lint` tests remain covered, with new compile/query-mode/lint regression tests.
- [x] No SQLite, memory, `chat_root`, `.skills`, or `.gitignore` changes are introduced.

## Risks

- Risk: Deterministic compile is less capable than LLM synthesis.
  - Impact: Generated articles are structured summaries rather than deep synthesis.
  - Mitigation: Label as deterministic MVP and leave LLM-backed compile/research for follow-up.

## Validation Plan

- Required tests: Unit tests for compile output, query modes, source validation, and link lint.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: Confirm docs and CLI help describe only shipped behavior.

## Regression Scope

- Areas likely affected: `skilllite wiki` CLI, `skilllite-commands` wiki module, docs.
- Explicit non-goals: memory, skills discovery, SQLite, network research, LLM calls.

## Links

- Source TODO section: User request to move closer to current LLM Wiki architecture after Repo Wiki MVP.
- Related PRs/issues:
- Related docs: `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
