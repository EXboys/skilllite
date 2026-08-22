# PRD

## Background

Skill install (`copy_skill` / `copy_dir_filtered`) used `fs::copy` and `Path::is_dir`, which follow symlinks. A local, git, or OpenClaw skill tree can therefore pull host files (`/etc/passwd`, `~/.ssh/id_rsa`, project `.env`) into `.skills/`. This is an install-time read of arbitrary host files and can leak secrets if the installed tree is later committed, shared, or executed.

## Objective

Install and import copy only regular files and directories. Any symlink that would be copied is rejected before the destination skill is removed.

## Functional Requirements

- FR-1: `copy_skill` must refuse a source tree that contains a non-excluded symlink and must not write the symlink target's bytes into dest.
- FR-2: The destination directory must be left untouched when the source is rejected.
- FR-3: Exclude-dir names (`.git`, `node_modules`, `.venv`, …) remain skipped even if those names are symlinks.
- FR-4: Skills without symlinks continue to install.

## Non-Functional Requirements

- Security: fail closed; do not follow or recreate host symlinks
- Performance: one extra metadata walk of the skill tree (small)
- Compatibility: skills that intentionally ship symlinks will now fail install (acceptable)

## Constraints

- Technical: stay inside `skilllite-commands` skill copy path; no new dependencies
- Timeline: N/A (automation sweep)

## Success Metrics

- Metric: regression tests fail if symlink follow is reintroduced
- Baseline: `fs::copy` of `ln -s /etc/passwd` produced a regular file with passwd contents
- Target: install errors; dest has no target bytes; existing dest preserved

## Rollout

- Rollout plan: merge the fail-closed copy check
- Rollback plan: revert the discovery.rs change
