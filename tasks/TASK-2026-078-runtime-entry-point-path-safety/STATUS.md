# STATUS

## Current Status

`done`

## Timeline

- 2026-08-02: Confirmed level-1 relative entry_point escape executes outside script (`OUTSIDE_EXECUTED`).
- 2026-08-02: Added `script_path_under_skill_dir` / `ensure_entry_point_within_skill` in `skilllite-core::path_validation`; wired metadata parse, `run_skill`, and agent executor.
- 2026-08-02: Validation evidence recorded; task marked done.

## Checkpoints

- [x] Bug reproduced with concrete PoC
- [x] Helper + call sites landed
- [x] Tests + PoC verification
- [x] Task artifacts / board finalized

## Validation Evidence

Commands run:

- `cargo test -p skilllite-core path_validation` → 4 passed
- `cargo test -p skilllite-core test_entry_point` → 3 passed (includes escape rejection)
- `cargo test -p skilllite-core` → 91 passed
- `cargo test -p skilllite-commands` → 23 passed
- `cargo clippy -p skilllite-core -p skilllite-agent --all-targets -- -D warnings` → clean
- `cargo fmt --check -p skilllite-core -p skilllite-commands -p skilllite-agent` → clean
- `python3 scripts/validate_tasks.py` → passed (71 task directories)
- Manual PoC at `SKILLLITE_SANDBOX_LEVEL=1`:
  - Escaping front-matter entry points no longer execute outside scripts (`OUTSIDE_EXECUTED` absent)
  - Absolute escape falls back to `scripts/main.py` and prints `{"msg":"inside"}`
  - Escape-only skill (no in-tree scripts) fails closed as prompt-only
  - Legitimate `scripts/main.py` still runs

Notes:

- Workspace `skilllite-commands` Clippy `-D warnings` still blocked by pre-existing `question_mark` / `dead_code` elsewhere (unrelated to this change).
