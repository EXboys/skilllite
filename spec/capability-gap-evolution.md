# Capability gap: scripts and evolution (lightweight)

When working **in this repository** and built-in tools, MCP, or existing `scripts/` are **not enough** to finish the job, prefer closing the gap with a **small, bounded script** (Python or Node — match what the repo already uses for the same kind of work) instead of repeating fragile manual steps.

## MUST

- Put throwaway or task-local outputs under **`tasks/TASK-.../artifacts/`** when a task folder exists; otherwise use **`scratch/<YYYYMMDD>-<short-slug>/`** and treat it as **temporary** (delete or fold into the PR/task before merge when possible).
- Keep scripts **out of the repository root** unless they are already part of an established layout (prefer `scripts/`, `tasks/TASK-.../artifacts/`, or `scratch/` as above).
- Record **what was run** and **where outputs live** in task evidence (`TASK.md` / `STATUS.md`) when a `tasks/TASK-.../` exists.
- **MUST NOT** commit secrets, tokens, or long-lived credentials into scripts or artifacts.

## SHOULD (evolution)

- Prefer **one short script** with clear inputs/outputs over several overlapping snippets.
- If the **same gap** appears **three or more times**, or a script is clearly **long-lived**: promote it into **`scripts/`** (repo-wide) or into a **Skill** / documented command path so the next session does not reinvent it.
- Add a **`--dry-run`** (or equivalent) when the script mutates files or touches non-artifact paths.

## MUST NOT (scope)

- This file does **not** redefine product runtime or end-user configuration; see **Repository scope** in `spec/README.md`.
