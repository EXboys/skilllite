# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Block grep_files from leaking sensitive files
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: `agent`
- Created: `2026-08-19`
- Target milestone: next merge

## Problem

`read_file` hard-blocks `.env`, `.key`, `.pem`, and `.git/config`, and redacts secret-like keys in other files. `grep_files` used the same workspace resolver but skipped both checks, so a normal agent tool call could return `API_KEY=...` from `.env` (or from a regular file) into the model context.

## Scope

- In scope:
  - Reject `grep_files` when the target path itself is a sensitive file
  - Skip sensitive files during directory walks so they cannot consume the match budget
  - Redact secret-like keys in remaining match lines (same helper as `read_file`)
  - Regression tests for the leak and the redaction path
  - EN/ZH architecture note that `grep_files` shares the sensitive-file policy
- Out of scope:
  - Dotenv variant suffix hardening (`#143`)
  - Symlink canonicalize (`#132`)
  - Recovered `write_file` clobber (`#144` / `#146`)
  - Preview-server symlink follow
  - ChatSession workspace-root alignment (`#114`)

## Acceptance Criteria

- [ ] Direct `grep_files` on `.env` / `.key` / `.pem` returns a blocked error and does not include file contents
- [ ] Workspace-root grep does not return lines from `.env` even when they match the pattern
- [ ] Sensitive keys in ordinary files are redacted in grep output
- [ ] Existing grep happy-path tests still pass
- [ ] `python3 scripts/validate_tasks.py` passes

## Risks

- Risk: Skipping files during walk could hide an expected match in a non-secret file whose name happens to match a suffix (for example `notes.key`).
  - Impact: Agent cannot grep that filename.
  - Mitigation: Reuse the existing `is_sensitive_read_path` policy already applied to `read_file`; those files were already unreadable.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent`, `cargo test`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
- Commands to run:
  - `cargo test -p skilllite-agent grep_files -- --nocapture`
  - `python3 scripts/validate_tasks.py`
- Manual checks: none; trigger is a unit-testable tool call.

## Regression Scope

- Areas likely affected:
  - Agent `grep_files` builtin
  - `skilllite-fs::grep_directory` skip-file callback
- Explicit non-goals:
  - Desktop IDE IO
  - Sandbox env inheritance
  - Preview HTTP serving

## Links

- Source TODO section: daily high-severity automation 2026-08-19
- Related PRs/issues: `#143` (dotenv variants, does not cover grep)
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
