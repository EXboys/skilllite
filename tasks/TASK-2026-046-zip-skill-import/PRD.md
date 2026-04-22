# PRD

## Background

SkillLite already imports skills from git repositories, ClawHub downloads, and
local directories. For the desktop client, especially on Windows, requiring bash
installers or git clones adds avoidable friction. Several external ecosystems
package skills as ZIP archives, so local ZIP import is the smallest compatible
step that keeps the desktop runtime in control of installation.

## Objective

Allow users and the desktop client to install a downloaded skill ZIP with the
existing `skilllite add` command, while preserving current admission, manifest,
and dependency-install behavior.

## Functional Requirements

- FR-1: `skilllite add <local-zip-path>` must recognize a `.zip` file path as a
  valid local source and extract it into a temporary directory before discovery.
- FR-2: After extraction, the command must reuse the existing discovery / scan /
  copy / manifest pipeline so installed skills behave the same as skills added
  from local folders or git sources.
- FR-3: ZIP extraction must reject unsafe entries that attempt absolute-path or
  parent-directory writes.

## Non-Functional Requirements

- Security: No archive entry may escape the temp extraction root.
- Performance: Extraction should be linear to archive size and avoid copying data
  more than needed.
- Compatibility: Preserve existing `skilllite add` semantics for directories,
  git URLs, and ClawHub sources. Desktop bridge should keep using `add_skill()`
  without API changes.

## Constraints

- Technical: Keep crate boundaries unchanged; the implementation belongs in
  `skilllite-commands` and must not pull desktop-specific logic upward.
- Timeline: Deliver as a bounded backend-only step so desktop UI work can follow
  without blocking on broader marketplace integration.

## Success Metrics

- Metric: Users can install a valid local ZIP package through `skilllite add`.
- Baseline: Local directories work, local ZIP files fail or are treated as
  unsupported/git-like input.
- Target: Valid local ZIP install succeeds; malformed traversal ZIP fails safely.

## Rollout

- Rollout plan: Ship behind the existing `skilllite add` command with docs update;
  desktop can immediately pass local ZIP paths through its current CLI bridge.
- Rollback plan: Revert ZIP-source parsing and extraction helper while leaving the
  rest of `skilllite add` unchanged.
