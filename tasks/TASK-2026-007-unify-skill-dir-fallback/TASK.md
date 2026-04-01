# TASK Card

## Metadata

- Task ID: `TASK-2026-007`
- Title: Unify skill dir fallback and conflict warning
- Status: `done`
- Priority: `P1`
- Owner: `exboys`
- Contributors:
- Created: `2026-04-01`
- Target milestone:

## Problem

Default `skills` -> `.skills` fallback logic was duplicated across multiple entry points (`init`, `skill common`, `ide`, `mcp`), which risks behavior drift over time.
Also, when `skills/foo` and `.skills/foo` coexist, there was no explicit warning to help users observe naming conflicts.

## Scope

- In scope:
- Add a unified directory resolve/fallback helper in `skilllite-core`.
- Switch `init`, `skill common`, `ide`, and `mcp` to the same helper.
- Add duplicate-name conflict detection and warning output (warning only; no priority changes).
- Out of scope:
- Changing skill loading priority or adding auto-deduplication behavior.
- Introducing new config keys or CLI flags.

## Acceptance Criteria

- [x] `init`, `skill common`, `ide`, and `mcp` no longer keep duplicated fallback logic and all use one helper.
- [x] When `skills/<name>/SKILL.md` and `.skills/<name>/SKILL.md` both exist, affected entry points emit a readable warning.
- [x] Existing compatibility behavior is preserved: if default `skills` does not exist and `.skills` exists, fallback still happens.

## Risks

- Risk:
  - Switching multiple entry points to one helper may slightly change relative/absolute path semantics in IDE config output.
  - Impact:
    - `SKILLLITE_SKILLS_DIR` in IDE integration config may differ from previous behavior.
  - Mitigation:
    - Preserve existing `ide` output semantics (prefer relative path in project mode, absolute path in global mode) and cover with tests.

## Validation Plan

- Required tests:
- `skilllite-core` unit tests for fallback + conflict detection.
- Affected `skilllite` / `skilllite-commands` tests or integration tests.
- Commands to run:
- `cargo fmt --check`
- `cargo clippy --all-targets -D warnings`
- `cargo test`
- `cargo test -p skilllite`
- Manual checks:
- Create coexistence case for `skills/foo` and `.skills/foo` and verify command output includes conflict warning.

## Regression Scope

- Areas likely affected:
- CLI path resolution (`init` / `quickstart` / `reindex` / `skill`).
- IDE integration config generation (Cursor/OpenCode).
- Skill directory selection during MCP startup.
- Explicit non-goals:
- Do not change skill discovery order or loader internal sorting rules.

## Links

- Source TODO section:
- `todo/06-OPTIMIZATION.md` (directory fallback unification recommendation)
- Related PRs/issues:
- Related docs:
