# TASK Card

## Metadata

- Task ID: `TASK-2026-079`
- Title: Reject path-escaping OpenClaw import skill names
- Status: `in_progress`
- Priority: `P0`
- Owner: `critical-bug-automation`
- Contributors:
- Created: `2026-08-03`
- Target milestone:

## Problem

`skilllite import-openclaw-skills` (and `claw migrate` skill import) trusts SKILL.md frontmatter `name` as the destination directory under the skills root. Because Rust `Path::join` replaces the root when given an absolute second component, and joins `../` relatives above the skills root, a malicious OpenClaw skill can write outside the intended `skills/` tree during import.

Concrete trigger:

1. Place a skill under `~/.openclaw/skills/evil-src/` with frontmatter `name: ../escaped-skill` (or an absolute path).
2. Run `skilllite import-openclaw-skills --force --scan-offline`.
3. Pre-fix: `copy_skill` installs to `<skills_root>/../escaped-skill` (or the absolute path), escaping the skills root.

## Scope

- In scope:
  - Shared `validate_skill_dir_name` / `skill_dir_under_root` helpers in `skilllite-core::path_validation`
  - Fail-closed validation in `cmd_import_openclaw_skills` before planning/install joins
  - Regression tests for relative and absolute escape names
- Out of scope:
  - `skilllite add` / MCP / CLI lookup validation (covered by open PR #125)
  - Pending evolution confirm/reject (open PR #89)
  - Broader OpenClaw migrate UX changes

## Acceptance Criteria

- [x] Unsafe frontmatter names (`../x`, absolute, multi-segment, drive prefixes) are skipped and never joined under the skills root
- [x] Safe skills in the same import batch still install
- [x] Unit/regression tests cover relative and absolute escape attempts
- [x] `cargo test -p skilllite-core path_validation` and `cargo test -p skilllite-commands import_` pass

## Risks

- Risk: Duplicate helper with open PR #125
  - Impact: Merge conflict on `path_validation.rs` / `PathValidationError`
  - Mitigation: Keep helper API identical to #125 so rebase is mechanical; OpenClaw call-site is unique to this PR

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core path_validation`
  - `cargo test -p skilllite-commands import_`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-core -p skilllite-commands --all-targets -- -D warnings` (with known main allow categories if needed)
- Commands to run: above
- Manual checks: N/A

## Regression Scope

- Areas likely affected:
  - `import-openclaw-skills` destination selection
  - `claw migrate` skill import (delegates to the same command)
- Explicit non-goals:
  - Changing conflict rename policy semantics for valid names
  - Validating source OpenClaw directory layout beyond dest name safety

## Links

- Source TODO section: critical bug automation sweep 2026-08-03
- Related PRs/issues: open PR #125 (add/MCP/CLI skill name safety)
- Related docs: N/A
