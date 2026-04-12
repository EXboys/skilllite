# Task-Centered Workflow

This directory turns long-form TODO documents into executable task units with lifecycle tracking.

## Scope

`tasks/` tracks **work on this SkillLite repository** (delivery, review, validation). It is **not** where end-user or shipped-application runtime configuration lives. For that distinction, see **Repository scope** in `spec/README.md`.

## Goals

- Replace "one giant TODO file" with small, independently shippable tasks.
- Attach decisions, risks, validation, and regression scope to each task.
- Enable parallel work and interruption recovery across subsystems.

## Contribution Policy

- External/community contributors: lightweight mode is allowed.
  - `Task ID: N/A` is valid for small docs/fix PRs.
  - Creating `tasks/TASK-.../` is optional unless the change is non-trivial.
- Core maintainers: strict mode is recommended.
  - Non-trivial changes should use full task artifacts and board tracking.

## Directory Structure

```text
tasks/
  board.md                        # Global status board
  _templates/
    TASK.md                       # Single-task card template
    PRD.md                        # Product/engineering requirement template
    CONTEXT.md                    # Technical context and constraints
    REVIEW.md                     # Review findings and merge readiness
    STATUS.md                     # Progress journal and checkpoints
  TASK-YYYY-NNN-short-name/
    TASK.md
    PRD.md
    CONTEXT.md
    REVIEW.md
    STATUS.md
```

## Lifecycle

`draft -> ready -> in_progress -> in_review -> done` (or `blocked`, `cancelled`)

Rules:

- Only one owner per task, but multiple contributors are allowed.
- One task should target one mergeable objective.
- A task cannot move to `done` without validation evidence.
- Task artifacts (`TASK.md`, `PRD.md`, `CONTEXT.md`, `STATUS.md`, `REVIEW.md`) should be written in English (see `spec/task-artifact-language.md`).

## Execution Flow

1. Copy templates into a new `TASK-.../` directory.
2. Fill `TASK.md` first (scope, owner, acceptance criteria).
3. Before implementation, draft `PRD.md` and `CONTEXT.md` (or mark `N/A` with reason).
4. Implement and log checkpoints in `STATUS.md`.
5. Record findings and release decision in `REVIEW.md`.
6. Update `tasks/board.md`.

## CI validation (`scripts/validate_tasks.py`)

CI and local checks should pass:

```bash
python3 scripts/validate_tasks.py
```

The script requires, for each `tasks/TASK-*/` folder:

- All of `TASK.md`, `PRD.md`, `CONTEXT.md`, `REVIEW.md`, `STATUS.md`.
- `TASK.md` metadata fields and a backtick-quoted `Status:` in the allowed set.
- `STATUS.md` must contain the headings **`## Timeline`** and **`## Checkpoints`** (copy structure from `tasks/_templates/STATUS.md`; do not replace with a headerless bullet list).
- `REVIEW.md` must contain the substring **`Merge readiness:`** (e.g. `- Merge readiness: ready` under `## Decision` in `tasks/_templates/REVIEW.md`).
- The task directory name must appear in `tasks/board.md`.

Completion note:

- Do not close a task if `PRD.md` / `CONTEXT.md` are stale versus final implementation.

## Mapping from Existing TODO

- Keep strategic analysis in `todo/*.md`.
- Track execution and delivery in `tasks/`.
- If a TODO item becomes actionable, create a `TASK-...` folder and link back to the source TODO section.
