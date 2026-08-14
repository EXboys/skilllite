# TASK Card

## Metadata

- Task ID: `TASK-2026-073`
- Title: Reject path-escaping skill directory names
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-07-27`
- Target milestone:

## Problem

Skill directory names from SKILL.md frontmatter and MCP/CLI `skill_name` arguments are joined onto skills roots without single-segment validation. A malicious `name: ../...` (or caller-supplied `../...`) can write, read, execute, or delete outside the skills directory.

## Scope

- In scope:
  - Shared single-segment skill directory name validator in `skilllite-core`.
  - Enforce it on `skilllite add` install destinations, `update_skill_from_source`, `find_skill`/`remove`, and MCP `get_skill_info` / `run_skill`.
  - Targeted regression tests for traversal rejection.
- Out of scope:
  - Pending evolution confirm/reject (covered by open PR #89).
  - Broader skill identity redesign or rename UX.
  - Docs rewrites beyond security-relevant behavior notes if needed.

## Acceptance Criteria

- [x] Traversal / absolute / multi-segment skill names are rejected before filesystem join.
- [x] `skilllite add` does not install outside the skills root when frontmatter `name` contains `../`.
- [x] MCP `get_skill_info` / `run_skill` reject escaping `skill_name` values.
- [x] CLI show/remove paths that use `find_skill`/remove reject escaping names.
- [x] Regression tests cover the critical write/read escape cases.
- [x] Validation commands pass for the touched crates.

## Risks

- Risk: Over-strict name rejection breaks unusual but legitimate skill names.
  - Impact: Install/show/run failures for edge-case names.
  - Mitigation: Allow any single normal path component (unicode OK); only reject separators, `.`, `..`, empty, and absolute forms.
- Risk: Missing a call site that still joins raw names.
  - Impact: Residual escape path.
  - Mitigation: Cover add destination, MCP handlers, find/remove, and update-from-source in one change.

## Validation Plan

- Required tests:
  - Unit tests for `validate_skill_dir_name`.
  - Command/MCP-focused tests proving traversal names do not touch escape targets.
- Commands to run:
  - `cargo test -p skilllite-core path_validation`
  - `cargo test -p skilllite-commands path_traversal`
  - `cargo test -p skilllite --test cli_mcp --test cli_skill_management`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-core -p skilllite-commands -p skilllite --all-targets -- -D warnings -A clippy::question_mark -A clippy::useless_borrows_in_formatting`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Trace add/MCP/remove call chains for remaining unsanitized joins.

## Regression Scope

- Areas likely affected:
  - Skill install destination selection
  - MCP skill lookup/execution
  - CLI show/remove by name
- Explicit non-goals:
  - Pending evolution path ops (PR #89)
  - Network sandbox policy changes

## Links

- Source TODO section: Scheduled critical bug automation, 2026-07-27.
- Related PRs/issues: Open PR #89 (pending skill names); prior notes from automation memory on skill-add/MCP escapes.
- Related docs: `spec/security-nonnegotiables.md`, `spec/verification-integrity.md`.
