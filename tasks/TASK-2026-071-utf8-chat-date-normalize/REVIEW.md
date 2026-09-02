# Review Report

## Scope Reviewed

- Files/modules:
  - `crates/skilllite-agent/src/extensions/builtin/chat_data.rs`
  - Caller: `execute_builtin_tool` → `chat_history` / `chat_plan` (no `catch_unwind`)
- Commits/changes:
  - `main` still at `6d5c5b9` (docs-only #145 since last merged code change #140). This is a longstanding crash on the builtin tool path, not a new-commit regression.

## Findings

- Critical:
  - `normalize_date` treated any 8-byte string as `YYYYMMDD` and sliced at byte indexes 4/6. Localized dates `2026年9` and `2026-9月` panic (`byte index 6 is not a char boundary`). The agent process exits.
- Major: None opened. Concurrent `schedule tick` duplicate runs remain a design gap (needs overlapping processes; same class as deferred `sessions.json` RMW). Remaining HIGH items map to open drafts #89/#112–#149.
- Minor: None.

## Quality Gates

- Architecture boundary checks: `pass` — change stays in `skilllite-agent` builtin helpers.
- Security invariants: `pass` — no sandbox, auth, or path-containment change.
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass` — tool schema already documents `YYYY-MM-DD` or `YYYYMMDD`; no command/env/docs change.

## Test Evidence

- Commands run:
  - `cargo test -p skilllite-agent --lib extensions::builtin::chat_data::tests` (before fix): 1 passed, 3 failed with the exact UTF-8 panic at `chat_data.rs:124`.
  - `cargo test -p skilllite-agent --lib extensions::builtin::chat_data::tests` (after fix): 4 passed.
  - `cargo test -p skilllite-agent`: 251 passed.
  - `cargo fmt --check`: clean.
  - `cargo clippy --all-targets -- -D warnings`: clean.
  - `cargo test`: all workspace packages `ok`, 0 failed.
  - `python3 scripts/validate_tasks.py`
- Key outputs:
  - Before: `end byte index 6 is not a char boundary; it is inside '年' (bytes 4..7 of string)` and inside `'月' (bytes 5..8)`.
  - After: CJK dates pass through; `20260902` / `2026-09-02` still normalize to `2026-09-02`.

## Decision

- Merge readiness: ready
- Follow-up actions: none for this crash. Do not re-open path-escape / recovered-write / SilentEventSink drafts already on GitHub.
