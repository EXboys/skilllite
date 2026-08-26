# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Fix session-clear memory index and OpenClaw provider key mapping
- Status: `in_review`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-08-26`
- Target milestone:

## Problem

Two high-severity correctness bugs on `main` @ `6d5c5b9`:

1. Session clear writes the daily memory Markdown correctly, but indexes FTS into `{session_key}.sqlite`. All `memory_search` / `build_memory_context` paths read `default.sqlite`, so cleared-session summaries are unsearchable. Named sessions (desktop extra sessions, `--session`, schedule jobs) hit this on every clear.
2. OpenClaw `--migrate-secrets` maps every `models.providers.*.apiKey` to `OPENAI_API_KEY` (last writer wins). A multi-provider OpenClaw config can overwrite a valid OpenAI key with an Anthropic/Groq/other key.

## Scope

- In scope:
  - Index session-clear memory into the shared `default` FTS DB used by search/context.
  - Map OpenClaw provider ids to the matching allowlisted env keys; do not stomp `OPENAI_API_KEY` with unrelated provider keys.
  - Regression tests for both behaviors.
- Out of scope:
  - Per-session memory isolation redesign.
  - ChatSession / `config.workspace` alignment (#114).
  - Other open HIGH drafts (#89, #112–#148).

## Acceptance Criteria

- [x] Clearing a non-`default` session indexes the daily memory file into `memory/default.sqlite`, not `{session_key}.sqlite`.
- [x] `memory_search` / BM25 can find the cleared-session summary in the shared index.
- [x] OpenClaw provider `apiKey` values write to the matching allowlisted env var (`ANTHROPIC_API_KEY`, `GROQ_API_KEY`, `OPENAI_API_KEY`, …).
- [x] Unknown provider ids do not write into `OPENAI_API_KEY`.
- [x] Targeted tests fail if the guarded behavior is removed.

## Risks

- Risk: Callers relied on per-session sqlite files created by clear.
  - Impact: Orphan `{session_key}.sqlite` files stop receiving new rows (they were never searched).
  - Mitigation: Search never read those files; indexing into `default` restores intended behavior.
- Risk: Custom OpenClaw provider ids expected to land in `OPENAI_API_KEY`.
  - Impact: Those keys are skipped after the fix.
  - Mitigation: Allowlisted `.env` / `openclaw.json` `env` object still merge; unknown providers are fail-closed to avoid credential corruption.

## Validation Plan

- Required tests: Unit tests in `chat_session` and `migrate/openclaw`; `cargo test -p skilllite-agent` and `cargo test -p skilllite-commands` (plus workspace `cargo test` / clippy / fmt).
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test -p skilllite-agent session_clear_memory`
  - `cargo test -p skilllite-commands openclaw_provider`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Re-read patched files.
  - Confirm tests are falsifiable (assert the shared DB / mapped keys).

## Regression Scope

- Areas likely affected: session clear memory FTS; OpenClaw secret merge.
- Explicit non-goals: sandbox, agent file tools, skill install path escapes.

## Links

- Source TODO section: Scheduled critical bug automation, 2026-08-26.
- Related PRs/issues: Open drafts #89 / #112–#148 (not this fix).
- Related docs: `spec/verification-integrity.md`, `spec/task-artifact-language.md`, `spec/rust-conventions.md`, `spec/testing-policy.md`, `spec/docs-sync.md`.
