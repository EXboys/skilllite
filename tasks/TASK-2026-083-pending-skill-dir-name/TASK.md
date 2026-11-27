# TASK Card

## Metadata

- Task ID: `TASK-2026-083`
- Title: Refuse pending skill names that escape the skills root
- Status: `in_review`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-27`
- Target milestone:

## Problem

`skilllite evolution reject <skill_name>` and `skilllite evolution confirm <skill_name>` pass the argument to `reject_pending_skill` and `confirm_pending_skill`. Those functions join the name onto `_evolved/_pending` and then `remove_dir_all` or `rename` the result.

`Path::join` replaces the base when the name is absolute. A relative name with `..` walks out of `_pending`. From a workspace skills directory, `evolution reject /home/user/.config` deletes that directory when it exists, and `evolution confirm` can rename a directory that sits outside `_pending`.

## Scope

- In scope:
  - Accept a pending skill name only when it is one normal path component.
  - Refuse `.`, `..`, absolute paths, and names that contain another component before any filesystem change.
  - Regression tests for absolute and relative escapes, plus a normal non-ASCII name that still confirms.
- Out of scope:
  - `skilllite remove` / `skilllite add` directory-name checks (open PR #184).
  - Reading a pending `SKILL.md` through a traversal name. That path does not delete or rename.
  - Changing where confirmed skills are stored.

## Acceptance Criteria

- [x] `reject_pending_skill` with an absolute directory path returns an error and leaves that directory in place.
- [x] `reject_pending_skill` with a `..` relative path returns an error and leaves the outside directory in place.
- [x] `confirm_pending_skill` with a `..` relative path returns an error and does not rename the outside directory.
- [x] A pending skill whose directory name is `日程` still moves to `_evolved/日程`.
- [x] A normal pending name `notes` is still removed by reject, and a sibling directory is kept.

## Risks

- Risk: A generated skill directory whose name is not a single path component can no longer be confirmed.
  - Impact: The user cannot confirm that skill by passing the raw name.
  - Mitigation: Evolved skill directories are created as one path component. `.` and `..` are not valid skill directory names.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-evolution --lib pending_skill`
- Commands to run: recorded in `STATUS.md` after execution.
- Manual checks: none. The trigger is covered by the unit tests.

## Regression Scope

- Areas likely affected:
  - `skilllite evolution confirm`
  - `skilllite evolution reject`
  - Desktop confirm/reject, which calls the same functions.
- Explicit non-goals:
  - Sandbox, artifact store, and session filename matching.

## Links

- Source TODO section:
- Related PRs/issues: sibling path check in PR #184 (`skilllite add` / `remove`). This task does not modify those commands.
- Related docs:
