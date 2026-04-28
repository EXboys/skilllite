# TASK Card

## Metadata

- Task ID: `TASK-2026-061`
- Title: Conversation wiki suggestion
- Status: `done`
- Priority: `P2`
- Owner: `agent`
- Contributors:
- Created: `2026-04-28`
- Target milestone:

## Problem

Repo Wiki can now refresh from raw sources, but chat and Desktop Assistant do not yet surface a safe prompt to capture lessons after replans or repeated tool failures.

## Scope

- In scope: Add structured wiki-update suggestion signals for conversation outcomes, add a user-confirmed lesson recording entry point, and expose the signal for CLI/Desktop consumers.
- Out of scope: Silent wiki writes, background watcher, full transcript ingestion, SQLite/vector indexes, memory migration, or UI rendering changes beyond structured events/CLI behavior.

## Acceptance Criteria

- [x] Conversation outcomes can produce a `WikiUpdateSuggestion` when replan or consecutive tool failure thresholds are met.
- [x] Suggestions contain structured facts: trigger, replan count, failed tools, error summaries where available, and a proposed lesson.
- [x] User confirmation can record the lesson into `.skilllite/wiki/raw/` and trigger compile using the existing Markdown-only wiki path.
- [x] CLI/Desktop entry layers can observe the suggestion without automatic writes.
- [x] Tests cover threshold triggering and no-suggestion cases.

## Risks

- Risk: Prompting too often could annoy users.
  - Impact: Chat and Desktop Assistant feel noisy.
  - Mitigation: Gate on structured replan/failure thresholds and require explicit confirmation before writing.

## Validation Plan

- Required tests: Unit tests for suggestion trigger logic and lesson recording.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test -p skilllite-agent`, `cargo test -p skilllite-commands`, `cargo test`, `python3 scripts/validate_tasks.py`.
- Manual checks: Confirm docs describe prompt/confirmation behavior only, not silent writes.

## Regression Scope

- Areas likely affected: chat/agent outcome metadata, wiki command recording path, docs.
- Explicit non-goals: memory storage, SQLite, watcher, automatic chat-time writes, `.skills`.

## Links

- Source TODO section: User requested prompt-after-replan/tool-failures and user confirmation before triggering wiki update.
- Related PRs/issues:
- Related docs: `README.md`, `docs/zh/README.md`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
