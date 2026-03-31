# Spec Injection Index

This directory defines short, enforceable rules that should be injected by task type.

## Specs

- `architecture-boundaries.md`: crate dependency direction and layering rules.
- `security-nonnegotiables.md`: security invariants for sandbox, policy, and gating changes.
- `testing-policy.md`: minimum required test set by change type.
- `docs-sync.md`: EN/ZH documentation sync requirements.

## Injection Strategy (Task Type -> Specs)

- `architecture` task:
  - Inject: `architecture-boundaries.md`, `testing-policy.md`, `docs-sync.md`
- `sandbox` or `security` task:
  - Inject: `security-nonnegotiables.md`, `testing-policy.md`, `docs-sync.md`
- `agent` / `commands` / `mcp` behavior task:
  - Inject: `architecture-boundaries.md`, `testing-policy.md`, `docs-sync.md`
- `python-sdk` task:
  - Inject: `testing-policy.md`, `docs-sync.md`
- `docs-only` task:
  - Inject: `docs-sync.md`
- `mixed/refactor` task:
  - Inject all four specs

## Deterministic Selection Rules

1. If files under `crates/skilllite-sandbox/` or security policy code are touched:
   always include `security-nonnegotiables.md`.
2. If workspace/crate boundaries, dependency direction, or extension wiring change:
   include `architecture-boundaries.md`.
3. For any code change:
   include `testing-policy.md`.
4. If user-facing behavior, commands, env vars, architecture docs, or release matrix changes:
   include `docs-sync.md`.
5. If two or more rules match, inject all matched specs (do not down-select to one).

## Prompt Header Template (for agents)

Use this header before implementation work:

```text
[Injected Specs]
- spec/<file-a>.md
- spec/<file-b>.md
...

[Task Type]
<one of: architecture | security | sandbox | agent | commands | mcp | python-sdk | docs-only | mixed>

[Enforcement]
Follow all MUST / MUST NOT / CHECKLIST items in injected specs.
```

## Review Gate

Before marking a task done:

- Confirm injected specs were listed in the working prompt.
- Confirm all relevant checklists were completed.
- Confirm verification commands were executed for the affected scope.
- Confirm task artifacts in `tasks/TASK-.../` are updated (`TASK.md`, `STATUS.md`, `REVIEW.md`) and `tasks/board.md` is in sync.
