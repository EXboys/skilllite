# TASK Card

## Metadata

- Task ID: `TASK-2026-068`
- Title: Fix UTF-8 preview truncation crashes
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-06-08`
- Target milestone:

## Problem

Recent UTF-8 truncation fixes covered several write/error paths, but adjacent preview/display
paths still byte-slice arbitrary strings. Multibyte CJK/emoji content can panic the CLI or agent
instead of returning a human-readable status or structured error.

## Scope

- In scope:
  - Make `skilllite evolution status` human output safe when `evolution_log.reason` contains long multibyte text.
  - Make `update_task_plan` invalid string previews safe for multibyte task payloads.
  - Make unexpected embedding response previews safe for multibyte JSON bodies.
  - Add focused non-ASCII regression tests for each fixed behavior.
- Out of scope:
  - Broader refactors of truncation utilities.
  - Lower-severity desktop skills-list error handling issues found during audit.
  - Packaging or UI behavior changes.

## Acceptance Criteria

- [x] Human `evolution status` formatting cannot panic on long CJK/emoji event reasons.
- [x] Agent planning-control errors cannot panic when previewing malformed multibyte task strings.
- [x] Embedding unexpected-response errors cannot panic when previewing multibyte JSON.
- [x] Focused tests cover the above non-ASCII cases.
- [x] Required Rust formatting, linting, tests, and task validation pass.

## Risks

- Risk: Changing truncation unit could slightly alter visible preview length.
  - Impact: Low; previews remain diagnostic-only and preserve existing error semantics.
  - Mitigation: Use existing UTF-8-safe helpers and keep limits close to the original byte caps.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite`
  - `cargo test -p skilllite-commands --features agent shorten_event_reason_preserves_utf8_boundaries`
  - `cargo test`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Build and run a CLI reproduction for `skilllite evolution status` with a long CJK reason: passed, exit `0`.

## Regression Scope

- Areas likely affected:
  - `skilllite-commands` evolution status display.
  - `skilllite-agent` planning-control and embedding error paths.
- Explicit non-goals:
  - No schema, CLI flag, API contract, or docs semantics changes.

## Links

- Source TODO section: N/A
- Related PRs/issues: Recent critical-bug investigations around `TASK-2026-066` and `TASK-2026-067`
- Related docs: N/A
