# PRD — Workspace symlink / path containment hardening

## Problem

Several containment checks used lexical `normalize_path` + `starts_with` and therefore followed in-workspace symlinks to outside targets. Separately, Tauri pending skill reads still joined raw `skill_name` even though open PR #89 hardens the CLI/evolution path, and bash `--cwd` / `rewrite_output_paths` could retarget allowlisted relative operations.

## Requirements

- FR-1: Existing path resolution for agent workspace tools must reject symlink targets outside the workspace/output root.
- FR-2: New-file writes under a symlinked directory outside the root must be rejected via nearest-existing-ancestor canonicalize.
- FR-3: Desktop Tauri pending `SKILL.md` reads must require a single-segment skill name.
- FR-4: bash `--cwd` must stay under skill dir, configured workspace, output dir, allowed skills root, or process cwd.
- FR-5: `rewrite_output_paths` must not rewrite parent-traversal or drive-absolute tokens into injected absolute paths.

## Non-requirements

- Do not redesign sandbox policy for bash-tool host execution beyond cwd containment.
- Do not block in-workspace symlinks whose real target remains inside the root.
