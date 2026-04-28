# PRD

## Background

The user wants SkillLite to adopt a Qoder-like project asset model for Repo Wiki while keeping existing root `.skills/` and memory behavior unchanged. This keeps shared project knowledge separate from long-term chat memory.

## Objective

Define and implement a project-local root for Repo Wiki without changing skills or memory storage.

## Functional Requirements

- FR-1: Resolve the project Repo Wiki root as `<project>/.skilllite/wiki/`.
- FR-2: Treat the Repo Wiki as plain Markdown source-of-truth; do not create or require SQLite for wiki.
- FR-3: Preserve existing root `.skills/` behavior exactly.
- FR-4: Preserve existing memory and `chat_root` behavior exactly.

## Non-Functional Requirements

- Security: Do not add `.gitignore` or persistence policy for unimplemented caches; do not move private memory into the project tree.
- Performance: Path resolution must be simple filesystem path construction/discovery only.
- Compatibility: Existing `.skills` / configured skill paths and global memory must continue to work unchanged.

## Constraints

- Technical: Follow existing crate dependency direction; path primitives belong in lower layers.
- Timeline: Keep the first implementation narrow and avoid migration logic.

## Success Metrics

- Metric: Project wiki root is represented in code and docs.
- Baseline: No explicit project Repo Wiki root.
- Target: Tests and docs prove the new wiki root while skills and memory behavior remain unchanged.

## Rollout

- Rollout plan: Ship as additive behavior; users can create `.skilllite/wiki/` in a project.
- Rollback plan: Revert the additive path resolution and docs; no data migration is required.
