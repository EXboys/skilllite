# PRD — Path root scoping and atomic_write staging

## Problem

Workspace-scoped evolution and migration commands still have residual global-root path bugs on `main`:

1. Operators run `skilllite evolution disable <id>` from a project and expect the project's evolved rules to be disabled. Instead the command reads/writes `~/.skilllite/chat/prompts/rules.json` (unless `SKILLLITE_WORKSPACE` is set), leaving project rules active and optionally corrupting the global store.
2. `skilllite claw migrate -w <project>` advertises a project workspace but copies OpenClaw `MEMORY.md` / daily notes into `~/.skilllite/chat/memory`, splitting persona/skills (project) from memory (global) and risking cross-project data leakage.
3. Shared `atomic_write` staging via `with_extension("tmp")` can cross-corrupt concurrent writes to same-stem different extensions.

## Goals

- Align disable/explain with the workspace chat root used by status/run/pending.
- Align OpenClaw migrate memory with `<workspace>/chat/memory`.
- Make `skilllite_fs::atomic_write` staging collision-free by basename.

## Non-Goals

- Re-fix issues already in open PRs #89 / #123–#133.
- Change planner `disabled` field semantics (#124).

## Requirements

- FR-1: Disable/explain MUST accept `--workspace/-w` and use `chat_root_for_workspace`.
- FR-2: Migrate MUST derive `memory_root` from the resolved project root's `chat/memory`.
- FR-3: `atomic_write` MUST stage to a unique temp name that includes the full destination basename.
