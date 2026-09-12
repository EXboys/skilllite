# STATUS

## Current status

`done` — fix validated; preparing PR.

## Timeline

- 2026-08-09: Critical bug sweep on `main@12010e8` reproduced bash-tool injection via bare `&` / `>` / `<`.
- 2026-08-09: Extended `CHAIN_OPERATORS`, added regression tests, updated EN/ZH architecture notes.
- 2026-08-09: Validation passed (`cargo test -p skilllite-sandbox bash_validator` 25 ok; clippy sandbox clean; `validate_tasks.py` 71 folders).

## Checkpoints

- [x] Concrete PoC: validator accepted injection forms; `sh -c` created `/tmp/pwned_*`
- [x] Code fix in `bash_validator.rs`
- [x] Unit tests added
- [x] Docs EN/ZH updated
- [x] `cargo test -p skilllite-sandbox bash_validator` — 25 passed
- [x] `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings` — clean
- [x] `cargo fmt --check` — clean for changed files / workspace
- [x] `python3 scripts/validate_tasks.py` — 71 task folders passed
- [x] PR opened: https://github.com/EXboys/skilllite/pull/137

## Blockers

- None.

## Validation evidence

```text
$ cargo test -p skilllite-sandbox bash_validator
running 25 tests
...
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 61 filtered out

$ cargo clippy -p skilllite-sandbox --all-targets -- -D warnings
Finished `dev` profile ... 

$ python3 scripts/validate_tasks.py
Task validation passed (71 task directories checked).
```

Pre-fix PoC (host):

```text
ACCEPT agent-browser open ... & touch /tmp/pwned_bg  -> file created
ACCEPT agent-browser open x&touch /tmp/pwned_tight   -> file created
ACCEPT agent-browser open x > /tmp/pwned_redir       -> file created
```
