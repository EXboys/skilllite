# TASK Card

## Metadata

- Task ID: `TASK-2026-046`
- Title: CLI: import skill ZIP packages
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-22`
- Target milestone:

## Problem

Desktop users need a Windows-safe path to install third-party skills without
depending on external bash installers or Git availability. Today `skilllite add`
accepts git-style sources and local directories, but not a downloaded skill ZIP
bundle such as the package exported by ModelScope skills.

## Scope

- In scope:
  - Extend `skilllite add` so a local `.zip` path can be imported as a skill source.
  - Reuse the existing skill discovery, admission scan, copy, dependency install,
    and manifest update flow after ZIP extraction.
  - Add ZIP extraction safety checks (path traversal rejection) and tests.
  - Document the new local ZIP install path in EN/ZH user docs.
- Out of scope:
  - Remote ZIP URL download support.
  - Desktop UI file picker / assistant-side import affordance.
  - New manifest format or a dedicated `import-zip` command.

## Acceptance Criteria

- [x] `skilllite add ./path/to/skill.zip` treats the ZIP as a local source, extracts it
      to a temporary directory, discovers contained skills, and installs them via the
      same scan + manifest flow used for existing sources.
- [x] ZIP extraction rejects path traversal attempts and tolerates single-root archive
      layouts produced by skill marketplaces.
- [x] Regression tests cover a valid local ZIP add path and a malicious ZIP path
      traversal case; EN/ZH docs mention local ZIP import support.

## Risks

- Risk: ZIP archives vary in root layout (`skill/`, `repo-main/skill/`, multi-skill bundles).
  - Impact: Valid archives could extract successfully but still produce "No skills found".
  - Mitigation: Reuse existing recursive discovery from `skilllite add` after extraction
    instead of assuming a fixed root directory shape.
- Risk: Unsafe ZIP extraction can write files outside the temp directory.
  - Impact: Security regression in the import path.
  - Mitigation: Reject absolute paths, parent traversal, and invalid enclosed names
    during extraction; add a regression test.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite add_local_zip`
  - `cargo test -p skilllite-commands zip_local_source`
- Commands to run:
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Create a minimal skill folder, zip it, and verify `skilllite add` installs it into
    the resolved skills dir without requiring git.

## Regression Scope

- Areas likely affected:
  - `crates/skilllite-commands/src/skill/add/source.rs`
  - `crates/skilllite-commands/src/skill/add/mod.rs`
  - `skilllite` CLI integration tests and add-source parsing behavior
- Explicit non-goals:
  - Changing OpenClaw import behavior.
  - Adding direct remote marketplace integration in this task.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
