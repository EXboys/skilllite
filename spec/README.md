# Spec Injection Index

This directory defines short, enforceable rules that should be injected by task type.

## Specs

- `verification-integrity.md`: **highest-priority** — anti-hallucination and anti-false-positive rules. Injected for ALL task types unconditionally.
- `architecture-boundaries.md`: crate dependency direction and layering rules.
- `security-nonnegotiables.md`: security invariants for sandbox, policy, and gating changes.
- `rust-conventions.md`: Rust coding conventions — no unwrap in production, crate-level Error/Result, Clippy zero warnings, no raw anyhow in crates.
- `testing-policy.md`: minimum required test set by change type.
- `docs-sync.md`: EN/ZH documentation sync requirements.

## Injection Strategy (Task Type -> Specs)

**Universal (all task types):** `verification-integrity.md` is always injected first.

- `architecture` task:
  - Inject: `verification-integrity.md`, `architecture-boundaries.md`, `rust-conventions.md`, `testing-policy.md`, `docs-sync.md`
- `sandbox` or `security` task:
  - Inject: `verification-integrity.md`, `security-nonnegotiables.md`, `rust-conventions.md`, `testing-policy.md`, `docs-sync.md`
- `agent` / `commands` / `mcp` behavior task:
  - Inject: `verification-integrity.md`, `architecture-boundaries.md`, `rust-conventions.md`, `testing-policy.md`, `docs-sync.md`
- `python-sdk` task:
  - Inject: `verification-integrity.md`, `testing-policy.md`, `docs-sync.md`
- `docs-only` task:
  - Inject: `verification-integrity.md`, `docs-sync.md`
- `mixed/refactor` task:
  - Inject all six specs

## Deterministic Selection Rules

0. **Always** include `verification-integrity.md` — this is the highest-priority spec and applies unconditionally to every task type.
1. If files under `crates/skilllite-sandbox/` or security policy code are touched:
   always include `security-nonnegotiables.md`.
2. If workspace/crate boundaries, dependency direction, or extension wiring change:
   include `architecture-boundaries.md`.
3. For any Rust code change:
   include `rust-conventions.md` and `testing-policy.md`.
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

Lightweight exception:

- For external/community small PRs, `Task ID: N/A` is allowed and `tasks/TASK-.../` can be skipped.
- Even in lightweight mode, injected specs, validation evidence, and regression scope should still be explicit.
