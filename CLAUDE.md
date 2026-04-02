# Claude Project Instructions

Use the same rule semantics as Cursor, but follow Claude's file conventions.

## Source of Truth

- **Verification integrity (highest priority)**: `spec/verification-integrity.md`
- Task artifact language policy: `spec/task-artifact-language.md`
- Spec routing and enforcement: `spec/README.md`
- Architecture constraints: `spec/architecture-boundaries.md`
- Rust coding conventions: `spec/rust-conventions.md`
- Security constraints: `spec/security-nonnegotiables.md`
- Testing policy: `spec/testing-policy.md`
- Docs sync policy: `spec/docs-sync.md`
- Structured-signal-first policy: `spec/structured-signal-first.md`
- Task workflow: `tasks/README.md`
- Task board: `tasks/board.md`

Do not duplicate rule content in this file. Follow the referenced files directly.

## Contribution Modes

- External/community contribution (lightweight):
  - `Task ID: N/A` is acceptable for small fixes/docs/issues.
  - Creating `tasks/TASK-.../` is optional.
  - Validation evidence and regression notes are still required in PR.
- Core/maintainer contribution (strict):
  - Use full `tasks/TASK-.../` lifecycle for non-trivial work.

## Required Workflow

1. Determine task type and inject specs using `spec/README.md`.
2. For non-trivial work, create/reuse a task folder in `tasks/TASK-.../`.
3. Before implementation starts, ensure baseline task artifacts are ready:
   - `TASK.md` scoped with acceptance criteria
   - `PRD.md` drafted (or explicitly `N/A` with reason)
   - `CONTEXT.md` drafted (or explicitly `N/A` with reason)
4. Keep task artifacts updated:
   - `TASK.md`
   - `PRD.md`
   - `CONTEXT.md`
   - `STATUS.md`
   - `REVIEW.md`
   - Keep task artifacts in English per `spec/task-artifact-language.md`
5. Keep `tasks/board.md` in sync with real status.
6. For any Rust code change, enforce `spec/rust-conventions.md` and `spec/testing-policy.md`.
7. For behavior/command/env/doc/release changes, enforce `spec/docs-sync.md`.

## Task Creation

Prefer the standard task bootstrap script:

```bash
bash scripts/new_task.sh "<slug>" "<Title>" [P0|P1|P2] [owner]
```

## Completion Gate

Before marking work complete:

- `spec/verification-integrity.md` checklist is completed (anti-hallucination, anti-false-positive),
- injected specs are listed and followed,
- `PRD.md` and `CONTEXT.md` are updated to final implementation state (or explicit `N/A`),
- validation evidence exists (commands actually run, outputs inspected — not just model assertions),
- regression scope is reviewed,
- task artifacts are updated,
- `tasks/board.md` is updated AND re-read to confirm the status change landed.
