# STATUS

## Current Status

`done`

## Timeline

- 2026-07-27: Confirmed skill-add frontmatter name escape and MCP skill_name join escape on `main` @ `12010e8`.
- 2026-07-27: Added shared `validate_skill_dir_name` / `skill_dir_under_root` and enforced call sites.
- 2026-07-27: Validation passed; preparing PR.

## Checkpoints

- [x] Concrete trigger scenarios documented
- [x] Shared validator landed with unit tests
- [x] Call sites enforced
- [x] Validation evidence recorded
- [ ] PR opened

## Blockers

- None.

## Validation Evidence

- `cargo test -p skilllite-core path_validation` → 3 passed
- `cargo test -p skilllite-commands path_traversal` → 3 passed (includes `cmd_add_rejects_frontmatter_name_path_traversal`)
- `cargo test -p skilllite --test cli_mcp --test cli_skill_management` → 14 + 21 passed
- `cargo fmt --check` → clean after fmt
- `cargo clippy -p skilllite-core -p skilllite-commands -p skilllite --all-targets -- -D warnings -A clippy::question_mark -A clippy::useless_borrows_in_formatting` → clean (pre-existing `question_mark` / `useless_borrows_in_formatting` allowed per prior main state)
- `python3 scripts/validate_tasks.py` → Task validation passed (71 task directories checked)
