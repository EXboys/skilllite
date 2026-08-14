# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Reject invalid stdio sandbox levels
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors: Cursor critical bug automation
- Created: `2026-07-19`
- Target milestone: Next patch release

## Problem

The stdio JSON-RPC parser casts an unbounded `u64` sandbox level to `u8`.
Values such as `257` therefore become level `1`, which executes skill code without
isolation instead of rejecting an invalid security setting.

## Scope

- In scope: Validate `sandbox_level` for stdio `run` and `exec` requests before
  conversion, reject non-integer and out-of-range values, and add regression tests.
- Out of scope: Refactoring sandbox policy propagation, changing valid level
  semantics, or adding Python-side duplicate validation.

## Acceptance Criteria

- [x] Stdio `run` and `exec` accept only integer sandbox levels 1, 2, or 3.
- [x] Values that previously truncated to a valid lower level are rejected.
- [x] Missing `sandbox_level` preserves environment/default resolution.
- [x] Focused, crate-wide, formatting, lint, and task validation checks pass.

## Risks

- Risk: Existing clients may rely on invalid values silently falling back.
  - Impact: Those requests will return a validation error.
  - Mitigation: Preserve all documented values and omission semantics; fail closed
    for malformed security configuration.

## Validation Plan

- Required tests: Parser regression tests for `run` and `exec`, plus the workspace
  baseline and `skilllite` integration scope.
- Commands to run: `cargo test -p skilllite stdio_rpc_params`,
  `cargo test -p skilllite`, `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test`, and
  `python3 scripts/validate_tasks.py`.
- Manual checks: Confirm `257` cannot reach `SandboxLevel::Level1` and valid
  values remain unchanged.

## Regression Scope

- Areas likely affected: `skilllite serve --stdio` `run` and `exec` parameter parsing.
- Explicit non-goals: CLI and MCP parsing, sandbox backend behavior, resource-limit
  parsing, and API documentation for already-documented levels.

## Links

- Source TODO section: N/A; found by the scheduled critical bug sweep.
- Related PRs/issues: PR #121.
- Related docs: `spec/security-nonnegotiables.md`,
  `spec/testing-policy.md`.
