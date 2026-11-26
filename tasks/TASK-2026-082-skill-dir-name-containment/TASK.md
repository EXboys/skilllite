# TASK Card

## Metadata

- Task ID: `TASK-2026-082`
- Title: Refuse skill directory names that escape the skills root
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-11-26`
- Target milestone:

## Problem

`skilllite add` and `skilllite remove` join the SKILL.md `name` onto the skills directory and may delete that path. A name such as `../../.config` resolves outside the skills directory. On a project at `~/project` with skills in `~/project/skills`, `skilllite remove --force ../../.config` deletes `~/.config`. `skilllite add` of a skill with that name does the same before copying the skill into the emptied directory.

## Scope

- In scope:
  - Reject skill directory names that are not a single path component.
  - Keep install, remove, import, pending-skill confirm/reject, generated skill writes, and MCP skill lookup inside the skills root.
  - Regression tests that keep a sibling `.config/keep.txt` containing `保留`.
- Out of scope:
  - Renaming already installed skills whose folder was created by the old join.
  - Changing sandbox level defaults.

## Acceptance Criteria

- [x] `skilllite remove --force ../../.config` does not delete a directory outside the skills root.
- [x] `skilllite add` of a local skill whose `name` is `../../.config` returns an error and leaves the outside directory unchanged.
- [x] A normal name, including `笔记` and `计算器`, still installs and removes.
- [x] A traversal metadata name still removes only the matching child skill directory.

## Risks

- Risk: A previously installed skill whose only name contains a separator can no longer be addressed by that string.
  - Impact: Delete by the displayed metadata name fails closed instead of deleting an outside directory. The child directory can still be removed when its metadata name matches and the directory is inside the skills root.
  - Mitigation: Metadata search still deletes the in-root child. Unsafe names are never joined.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core --lib skill::dir_name`
  - `cargo test -p skilllite-commands --lib`
  - `cargo test -p skilllite-evolution --lib skill_synth::tests::reject_traversal_name_does_not_delete_outside_pending_dir`
  - `cargo test -p skilllite --test cli_mcp`
- Commands to run:
  - `cargo fmt --all -- --check`
  - `cargo clippy -p skilllite-core -p skilllite-evolution --all-targets -- -D warnings`
  - `cargo clippy -p skilllite-commands -p skilllite --all-targets -- -A clippy::needless_bool -D warnings`
- Manual checks:
  - Before the fix, `cmd_remove("../../.config", ..., true)` printed `Removed skill '.config'` and returned `Ok`.

## Regression Scope

- Areas likely affected:
  - `skilllite add`, `skilllite remove`, OpenClaw import, evolution pending confirm/reject, MCP `get_skill_info` / `run_skill`.
- Explicit non-goals:
  - Artifact temp-file naming, session filename prefixes, sandbox seccomp, and child RSS accounting.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - `README.md`
  - `docs/zh/README.md`
  - `docs/en/GETTING_STARTED.md`
  - `docs/zh/GETTING_STARTED.md`
