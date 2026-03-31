# Task-Centered Workflow

This directory turns long-form TODO documents into executable task units with lifecycle tracking.

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

## Execution Flow

1. Copy templates into a new `TASK-.../` directory.
2. Fill `TASK.md` first (scope, owner, acceptance criteria).
3. Add design and constraints in `PRD.md` and `CONTEXT.md`.
4. Implement and log checkpoints in `STATUS.md`.
5. Record findings and release decision in `REVIEW.md`.
6. Update `tasks/board.md`.

## Mapping from Existing TODO

- Keep strategic analysis in `todo/*.md`.
- Track execution and delivery in `tasks/`.
- If a TODO item becomes actionable, create a `TASK-...` folder and link back to the source TODO section.
