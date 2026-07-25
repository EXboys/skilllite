# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Fix Windows artifact path escape
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-25`
- Target milestone:

## Problem

`LocalDirArtifactStore` and the artifact HTTP API validate keys/run IDs against Unix-oriented rules only. On Windows, values such as `C:\Users\Public\owned.txt` (or `C:/...`) and run IDs like `\Windows\Temp` pass validation, then `Path::join` replaces the store root with an absolute path. That enables arbitrary file read/write outside the artifact directory through `artifact-serve` / `gateway serve --artifact-dir` (token optional) and any local store caller.

## Scope

- In scope:
  - Harden `validate_artifact_key` against Windows absolute/drive/UNC forms and backslashes.
  - Harden `validate_run_id` against `\`, drive prefixes, and other absolute forms.
  - Add a post-join containment check in `LocalDirArtifactStore::artifact_path`.
  - Add regression tests for accepted/rejected forms.
- Out of scope:
  - Symlink-based escapes after a path is inside the root.
  - Redesigning the artifact HTTP API or auth model.
  - Non-local (S3/DB) store backends.

## Acceptance Criteria

- [ ] Windows drive-absolute and rooted keys/run IDs are rejected by validators on all host platforms.
- [ ] `LocalDirArtifactStore` refuses any joined path that does not remain under `<base>/artifacts`.
- [ ] Existing hierarchical keys using `/` continue to work.
- [ ] Regression tests cover drive-letter keys, backslash-rooted run IDs, and happy-path keys.
- [ ] `cargo fmt --check`, focused package tests, and workspace `cargo test` pass.

## Risks

- Risk: Rejecting `\` or `:` breaks unusual but previously accepted keys.
  - Impact: Low; keys are logical artifact names, docs already imply relative POSIX-style paths.
  - Mitigation: Keep `/` hierarchy; document rejection reasons in error strings; cover with tests.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core artifact_store`
  - `cargo test -p skilllite-artifact`
  - `cargo test`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings` (allow known baseline lints if needed)
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm validators reject `C:\...`, `C:/...`, and `\Windows\Temp` without requiring a Windows host.

## Regression Scope

- Areas likely affected:
  - Artifact key/run_id validation and local filesystem store path construction.
  - Artifact HTTP put/get validation (same validators).
- Explicit non-goals:
  - Auth/token policy for artifact-serve.
  - Evolution pending-skill path traversal (tracked separately in open PR #89).

## Links

- Source TODO section: critical-bug automation sweep 2026-07-25
- Related PRs/issues:
- Related docs: `docs/en/ENV_REFERENCE.md` artifact-serve notes (no contract change expected)
