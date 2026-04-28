# TASK Card

## Metadata

- Task ID: `TASK-2026-058`
- Title: Repo Wiki MVP commands
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

TASK-2026-057 created the project Repo Wiki root and skeleton, but did not implement LLM Wiki operations. Users need command-level workflows to initialize, ingest, query, and lint the Markdown-only repo wiki.

## Scope

- In scope: `skilllite wiki init`, `skilllite wiki ingest <path>`, `skilllite wiki query <question>`, and `skilllite wiki lint` for `.skilllite/wiki/`.
- Out of scope: URL fetching, multi-agent research, LLM compilation, SQLite indexing, memory migration, project memory, or changes to existing `.skills/` discovery.

## Acceptance Criteria

- [x] `skilllite wiki init` creates/repairs the Markdown wiki structure.
- [x] `skilllite wiki ingest <path>` copies a local file into `raw/` as Markdown with frontmatter and updates indexes/logs.
- [x] `skilllite wiki query <question>` answers from wiki Markdown only using index/content scanning; it reports gaps when no matching content exists.
- [x] `skilllite wiki lint` validates required files/directories and frontmatter/index basics without using SQLite.
- [x] Existing memory, `chat_root`, and `.skills/` behavior remain unchanged.

## Risks

- Risk: A regex-only query could feel weaker than a full LLM Wiki.
  - Impact: Users may expect synthesized answers.
  - Mitigation: Scope MVP as deterministic Markdown query; leave LLM compile/research for follow-up.
- Risk: Ingest may copy sensitive files.
  - Impact: Private content could enter a commit-friendly wiki.
  - Mitigation: Local file ingest is explicit; generated frontmatter records source path.

## Validation Plan

- Required tests: Unit tests for wiki init, ingest, query, and lint behavior.
- Commands run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 scripts/validate_tasks.py`, `cargo run -p skilllite -- wiki --help`.
- Manual checks: Inspect CLI docs and code to confirm no SQLite or memory behavior was introduced.

## Regression Scope

- Areas likely affected: CLI command parsing/dispatch, `skilllite-commands` wiki module, docs.
- Explicit non-goals: memory storage/search, skill discovery, `.gitignore`, network fetching, LLM research.

## Links

- Source TODO section: User correction after TASK-2026-057: skeleton alone is not LLM Wiki logic.
- Related PRs/issues:
- Related docs: `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
