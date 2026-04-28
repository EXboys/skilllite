# TASK Card

## Metadata

- Task ID: `TASK-2026-057`
- Title: Project wiki root
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

SkillLite currently has memory and root `.skills/` behavior, but no explicit project-local Repo Wiki root. The product direction is to mirror Qoder-style project wiki assets while keeping existing skills and chat memory/storage unchanged.

## Scope

- In scope: Add project-local `.skilllite/wiki/` as the Repo Wiki root; document that the wiki is plain Markdown and does not use SQLite.
- Out of scope: Moving `chat_root`, changing `memory_write` / `memory_search` storage, changing existing `.skills/` behavior, adding `.gitignore` entries, supporting root `wiki/`, adding unimplemented fallback paths, or migrating existing memory/evolution data.

## Acceptance Criteria

- [x] Project wiki root resolves to `<project>/.skilllite/wiki/` and is documented as Markdown-only.
- [x] Existing root `.skills/` behavior is unchanged.
- [x] Memory and `chat_root` behavior are unchanged.
- [x] EN/ZH docs describe the shipped behavior only.

## Risks

- Risk: Wiki path semantics drift into memory behavior.
  - Impact: Existing chat/evolution state could split unexpectedly.
  - Mitigation: Keep memory/chat_root code paths unchanged in this task.

## Validation Plan

- Required tests: Unit tests for path resolution and `skilllite init` wiki skeleton creation.
- Commands run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: Inspect docs and changed code to verify no project memory path or wiki SQLite behavior was introduced.

## Regression Scope

- Areas likely affected: path resolution, docs.
- Explicit non-goals: memory storage, SQLite-backed wiki indexing, `.gitignore` changes, `chat_root` changes.

## Links

- Source TODO section: User-confirmed design discussion (2026-04-28): Qoder-style project wiki under `.skilllite/wiki/`; existing `.skills/` and memory unchanged.
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`, `README.md`, `docs/zh/README.md`
