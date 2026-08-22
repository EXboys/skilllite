# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Reject symlink follow during skill install
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors: `agent`
- Created: `2026-08-22`
- Target milestone: next patch

## Problem

`skilllite add`, skill update-from-source, and OpenClaw import copy skill trees with `std::fs::copy` / `Path::is_dir`, both of which follow symlinks. A skill that contains `passwd-link -> /etc/passwd` (or a directory symlink to `$HOME` / `~/.ssh`) materializes host file contents into `.skills/<name>/`. That is an install-time arbitrary file read and can leak secrets into the installed tree.

## Scope

- In scope:
  - Fail closed on any symlink that would be copied into the installed skill tree
  - Inspect the source tree before deleting an existing destination, so a rejected install cannot wipe a working skill
  - Keep existing exclude-dir behavior (`.venv`, `node_modules`, `.git`, …) including when those names are themselves symlinks
  - Regression tests (unit + CLI e2e) and EN/ZH user-doc note
- Out of scope:
  - Agent workspace symlink containment (open PR #132)
  - ZIP symlink entries (already materialized as regular files containing the path string, not followed)
  - Atomic dest replace / rewrite of `copy_skill` beyond the pre-scan

## Acceptance Criteria

- [ ] `copy_skill` rejects a file symlink to a host secret path and does not write the target bytes into dest
- [ ] `copy_skill` rejects a directory symlink
- [ ] Rejecting a symlink does not delete an already-installed skill at dest
- [ ] Excluded-dir names that are symlinks (e.g. `.venv`) do not fail the install
- [ ] Regular skills without symlinks still copy
- [ ] EN/ZH docs mention the fail-closed symlink rule

## Risks

- Risk: Legitimate skills that ship in-tree symlinks will fail to install
  - Impact: Breaking change for a rare packaging style
  - Mitigation: Fail with a clear path in the error; excluded tooling dirs still skipped
- Risk: Pre-scan and copy filters drift
  - Impact: A symlink could be skipped in the scan but copied later
  - Mitigation: Shared skip helpers; copy path also fail-closed on leftover symlinks

## Validation Plan

- Required tests:
  - Unit tests in `crates/skilllite-commands/src/skill/add/discovery.rs`
  - E2E: `skilllite add` of a local skill that contains a host-file symlink
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo fmt --check`
  - `cargo clippy -p skilllite-commands -p skilllite --all-targets -- -D warnings`
  - `cargo test -p skilllite-commands`
  - `cargo test -p skilllite --test e2e_minimal`
- Manual checks:
  - Re-read modified files after edit

## Regression Scope

- Areas likely affected: `skilllite add`, `update_skill_from_source`, `import-openclaw-skills`
- Explicit non-goals: agent file-ops path containment, zip-slip (already covered)

## Links

- Source TODO section: N/A (critical-bug automation sweep)
- Related PRs/issues: distinct from open path-escape PRs #123–#134 / #132
- Related docs: `README.md`, `docs/zh/README.md`
