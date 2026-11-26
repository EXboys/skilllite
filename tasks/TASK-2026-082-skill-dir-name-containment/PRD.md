# PRD

## Background

Skill install uses the SKILL.md `name` as the destination directory. Remove uses the same string. Both call `remove_dir_all` on the joined path. The name is not restricted to one directory component, so a third-party skill or a hand-edited front matter can address a directory outside the skills root.

## Objective

Installing, removing, importing, or rejecting a skill must not delete or write a directory outside the skills root when the skill name contains a path.

## Functional Requirements

- FR-1: A skill directory name must be one normal path component. Reject empty names, `.`, `..`, absolute paths, `/`, `\`, and control characters.
- FR-2: `skilllite add`, `skilllite remove`, OpenClaw import, and evolution pending confirm/reject use that check before joining or deleting.
- FR-3: Remove of a traversal string that matches a child skill's metadata deletes that child only.
- FR-4: Generated skill names and entry points that leave the pending directory are skipped.

## Non-Functional Requirements

- Security: Default behavior becomes stricter. Existing single-component names, including non-ASCII names, keep working.
- Performance: One string check per install or remove.
- Compatibility: Skills whose names are already a single directory name are unchanged.

## Constraints

- Technical: The check lives in `skilllite-core` so commands, evolution, and MCP share it. Core does not depend on upper crates.
- Timeline: Minimal fix on the current main line.

## Success Metrics

- Metric: Outside directory contents survive add and remove of a traversal name.
- Baseline: `cmd_remove("../../.config", skills, true)` returned `Ok` and removed `.config`.
- Target: The same call returns an error, and `keep.txt` still contains `保留`.

## Rollout

- Rollout plan: Ship with the CLI. No new environment variable.
- Rollback plan: Revert the commit. Do not restore a deleted outside directory; that data is already gone if the old command ran.
