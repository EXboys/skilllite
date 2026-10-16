# TASK Card

## Metadata

- Task ID: `TASK-2026-072`
- Title: Fix macOS Seatbelt allow-default override
- Status: `in_review`
- Priority: `P0`
- Owner: `cursor-agent`
- Contributors:
- Created: `2026-10-16`
- Target milestone:

## Problem

macOS Seatbelt is last-match-wins. `generate_sandbox_profile_with_proxy` emitted `(allow default)` after the network, file-write, process-exec, and IPC denies. That last rule re-opened those operations, so a default Level 2/3 skill on macOS could make network connections, write outside the workspace, and exec programs other than the interpreter.

## Scope

- In scope:
  - Reorder the production Seatbelt profile so `(allow default)` is first and later denies stick.
  - Deny `process-exec` before the interpreter allow.
  - Deny `network*` before localhost proxy allows when domain filtering is on.
  - Emit mandatory write denies after the workspace write allow.
  - Keep the test-only legacy profile on the same order.
  - Compile the macOS module under `cfg(test)` so Linux CI runs the order tests.
- Out of scope:
  - Linux bwrap domain-filter fail-open (open PR #112).
  - Changing Level 1 (`SKILLLITE_NO_SANDBOX` / sandbox level 1) behavior.

## Acceptance Criteria

- [x] Production profile contains a single `(allow default)`, and it precedes network, file-write, process, and IPC denies.
- [x] `(deny process-exec)` precedes the interpreter `(allow process-exec ...)`.
- [x] Proxy-filtered profiles deny `network*` before `localhost` proxy allows.
- [x] Mandatory `(deny file-write* (regex ...))` rules come after the work-directory write allow.
- [x] EN and ZH architecture docs describe last-match-wins ordering.

## Risks

- Risk: Skills that previously spawned extra processes or opened direct sockets on macOS will now be blocked.
  - Impact: Those calls fail with a Seatbelt denial. That is the documented whitelist.
  - Mitigation: Interpreter, framework re-exec, work directory, and project-root writes stay allowed. Playwright still allows `process-fork`.

## Validation Plan

- Required tests: Seatbelt rule-order unit tests, `cargo test -p skilllite-sandbox`, workspace `cargo test`, `e2e_minimal`.
- Commands to run: see `STATUS.md`.
- Manual checks: `sandbox-exec` containment was not executed here (Linux host). Order tests lock the profile text that Seatbelt evaluates last-match-wins.

## Regression Scope

- Areas likely affected: macOS Level 2/3 skill execution only.
- Explicit non-goals: Linux and Windows sandbox backends.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs: `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`
