# STATUS

## Current status

`in_progress` — validator fix and tests landed; validation running.

## Timeline

- 2026-08-09: Critical bug sweep on `main@12010e8` reproduced bash-tool injection via bare `&` / `>` / `<`.
- 2026-08-09: Extended `CHAIN_OPERATORS`, added regression tests, updated EN/ZH architecture notes.

## Checkpoints

- [x] Concrete PoC: validator accepted injection forms; `sh -c` created `/tmp/pwned_*`
- [x] Code fix in `bash_validator.rs`
- [x] Unit tests added
- [x] Docs EN/ZH updated
- [ ] `cargo test -p skilllite-sandbox` evidence recorded
- [ ] `cargo fmt --check` / clippy evidence recorded
- [ ] `python3 scripts/validate_tasks.py` evidence recorded
- [ ] PR opened

## Blockers

- None.
