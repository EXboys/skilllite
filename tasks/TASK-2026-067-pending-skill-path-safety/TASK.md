# TASK Card

## Metadata

- Task ID: `TASK-2026-067`
- Title: Constrain pending skill path operations
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: Cursor automation
- Created: `2026-06-03`
- Target milestone: critical bug fix

## Problem

Pending skill operations accept `skill_name` values and join them directly under
`<skills>/_evolved/_pending`. Absolute paths and `..` segments can escape that
directory; `reject_pending_skill` then calls `remove_dir_all` on the escaped
path, and `confirm_pending_skill` can move directories outside the pending
queue. The CLI and desktop bridge expose these operations.

## Scope

- In scope: validate pending skill names as safe single path segments before
  read, confirm, or reject filesystem operations.
- In scope: add regression tests that prove path traversal inputs are rejected
  without deleting or moving out-of-scope directories.
- Out of scope: redesign pending skill storage, UI contract changes, or broader
  evolution policy changes.

## Acceptance Criteria

- [ ] `confirm_pending_skill`, `reject_pending_skill`, and desktop pending skill
      reads reject absolute paths and `..` escapes.
- [ ] Valid pending skill names continue to confirm and reject normally.
- [ ] Regression tests cover rejection behavior and prove out-of-scope
      directories survive.

## Risks

- Risk: Overly strict validation could reject valid skill directories.
  - Impact: Users may be unable to confirm or reject a generated pending skill.
  - Mitigation: Match the safety boundary to existing listing behavior: only
    single directory names are valid pending skill identifiers.

## Validation Plan

- Required tests: focused `skilllite-evolution` unit tests plus workspace Rust
  format, clippy, and test commands required by repository policy.
- Commands to run: `cargo fmt --check`; `cargo test -p skilllite-evolution`;
  `cargo test -p skilllite-commands`; `cargo clippy --all-targets -- -D warnings`;
  `cargo test`; `python3 scripts/validate_tasks.py`.
- Manual checks: inspect the patched filesystem paths and task artifacts after
  edits.

## Regression Scope

- Areas likely affected: evolution pending skill read/confirm/reject paths,
  desktop evolution UI commands, CLI `skilllite evolution confirm|reject`.
- Explicit non-goals: changing generated skill naming rules beyond path safety,
  adding new dependencies, or changing user-facing command names.

## Links

- Source TODO section: N/A; discovered during critical bug automation.
- Related PRs/issues: recent evolution L2 CLI bridge commits.
- Related docs: `spec/security-nonnegotiables.md`,
  `spec/rust-conventions.md`, `spec/testing-policy.md`.
