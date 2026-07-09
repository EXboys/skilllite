# PRD

## Background

Desktop chat writes now run in a child process whose current directory and `SKILLLITE_WORKSPACE` are set to the active UI project root. Several host-side read and mutation helpers still use the Tauri process-global chat root. In packaged desktop launches that root commonly resolves to `~/.skilllite/chat`, while the child writes under `<workspace>/chat`.

## Objective

Ensure desktop host reads/writes and background schedule execution use the same active workspace root as chat and evolution subprocesses. Preserve compatibility for callers that do not pass a workspace.

## Functional Requirements

- FR-1: Chat transcript reload, session listing/CRUD, memory/log reads, recent data, and prompt artifact operations must resolve chat data from `<active workspace>/chat` when a workspace is supplied.
- FR-2: Life Pulse rhythm must invoke `skilllite schedule tick --workspace <active workspace>` and run from the resolved project root.
- FR-3: Existing command calls with omitted workspace must continue to use the previous process-global chat root.

## Non-Functional Requirements

- Security: Avoid wrong-root scheduled agent execution and wrong-root prompt edits.
- Performance: Keep routing changes path-only; do not add extra subprocesses or blocking scans beyond existing project-root lookup.
- Compatibility: Tauri commands accept optional workspace values so older callers remain valid.

## Constraints

- Technical: Use the desktop bridge's existing `find_project_root` semantics to match chat subprocess behavior.
- Timeline: N/A for autonomous execution; scope is limited to focused correctness fixes and tests.

## Success Metrics

- Metric: Workspace-scoped regression tests.
- Baseline: Host read helpers and rhythm subprocess can resolve different roots than the active workspace.
- Target: Tests prove supplied workspaces route to `<workspace>/chat`, while omitted workspaces keep legacy global root behavior.

## Rollout

- Rollout plan: Land as a focused desktop bridge fix.
- Rollback plan: Revert the PR if workspace-scoped desktop reads regress unexpectedly.
