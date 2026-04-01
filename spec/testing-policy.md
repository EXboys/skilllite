# Testing Policy (Change-based)

Scope: all code changes. Principle: required tests are selected by change type; partial "convenience-only" test runs are not acceptable.

## Baseline (every code change)

- Required: `cargo fmt --check`
- Required: `cargo clippy --all-targets`
- Required: `cargo test`

## Additional Required Tests by Change Type

- Changes in `skilllite-sandbox` (execution/policy/scanning/runtime deps):
  - Required: `cargo test -p skilllite-sandbox`
  - Required: cover the `skilllite/tests/e2e_minimal.rs` path (or equivalent E2E)
- Changes in `skilllite-agent` (chat loop/tool orchestration/planning/reflection):
  - Required: `cargo test -p skilllite-agent`
  - Recommended: add one minimal regression-focused test
- Changes in CLI/commands/MCP protocol behavior:
  - Required: `cargo test -p skilllite`
  - Required: at least one integration or E2E case that covers behavior change
- Changes in `python-sdk`:
  - Required: `cd python-sdk && ruff check . && ruff format --check . && mypy skilllite && pytest`

## Test Authoring Rules

- Bug fixes must include either a failing test first or a regression test in the same PR.
- New features must include at least 1 happy-path and 1 failure-path test (unit or integration).
- Test names must describe behavior (avoid generic names like `test_fix_xxx`).
- For string formatting/truncation/error-summary logic, add at least one non-ASCII test case (e.g. Chinese or emoji) to guard UTF-8 boundary safety.
- If code has explicit fallback/retry/error branches, include at least one test that exercises that non-happy-path behavior.

## Merge Gate

- [ ] Were all required tests for this change type executed?
- [ ] Were regression tests added or updated?
- [ ] Do local results match CI outcomes?
