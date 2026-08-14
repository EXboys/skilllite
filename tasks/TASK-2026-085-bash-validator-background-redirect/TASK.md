# TASK Card

## Metadata

- Task ID: TASK-2026-085-bash-validator-background-redirect
- Title: Block bash background and redirect injection
- Status: `done`
- Priority: `P0`
- Owner: automation
- Contributors: automation
- Created: 2026-08-09
- Target milestone: security hotfix

## Problem

`validate_bash_command` blocks `;`, `&&`, `||`, `|`, and process substitution `>(`, but accepts bare `&`, `>`, and `<`. Bash-tool skills then run the accepted string via unsandboxed `sh -c`, so an LLM (or compromised skill args) can background a second host command or redirect I/O to arbitrary paths.

Concrete trigger:

```text
agent-browser open https://example.com & touch /tmp/pwned
agent-browser open x > /tmp/pwned
```

Both are accepted by the validator on `main@12010e8` and execute the side effect even when `agent-browser` is missing.

## Scope

- In scope:
  - Extend `CHAIN_OPERATORS` in `skilllite-sandbox` bash validator
  - Regression tests for `&`, tight `&`, `>`, `<`, `>>`
  - EN/ZH architecture note for the hardened operator set
- Out of scope:
  - Full shell AST parsing / quote-aware validation
  - Sandboxing bash-tool execution itself
  - SilentEventSink auto-approve and chat-root workspace split (tracked separately)

## Acceptance Criteria

- [x] Bare `&`, `>`, and `<` are rejected by `validate_bash_command`
- [x] Existing chain-operator / blocked-prefix tests still pass
- [x] New regression tests cover background and redirect forms
- [x] EN/ZH architecture docs mention the operator set
- [x] `cargo test -p skilllite-sandbox` passes for validator coverage

## Risks

- Risk: Strict substring blocking of `&` / `>` / `<` rejects URLs or args that contain those characters (including quoted query strings).
  - Impact: Some previously-accepted bash-tool commands fail validation.
  - Mitigation: Matches existing substring policy for `;` / `|`; safer fail-closed default for unsandboxed `sh -c`.

## Validation Plan

- Required tests: sandbox bash_validator unit tests (existing + new)
- Commands to run:
  - `cargo test -p skilllite-sandbox bash_validator`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `python3 scripts/validate_tasks.py`
- Manual checks:
  - Confirm pre-fix acceptance / post-fix rejection matrix for `&` / `>` / `<`

## Regression Scope

- Areas likely affected: bash-tool skill execution (`skilllite-agent` / `skilllite-commands` execute paths)
- Explicit non-goals: changing sandbox level defaults; rewriting bash execution to use bwrap

## Links

- Source TODO section: N/A (critical bug automation sweep 2026-08-09)
- Related PRs/issues: open security backlog #89 / #112 / #123–#136 (different themes)
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
