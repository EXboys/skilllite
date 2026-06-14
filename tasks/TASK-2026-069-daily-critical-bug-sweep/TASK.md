# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Daily critical bug sweep
- Status: `done`
- Priority: `P0`
- Owner: `agent`
- Contributors: automation
- Created: `2026-06-14`
- Target milestone: daily critical bug investigation

## Problem

Recent commits can introduce high-severity correctness regressions that escaped review. This task audits recent repository changes for concrete, triggerable bugs that could cause data loss, crashes, security holes, or significant user-facing breakage.

## Scope

- In scope: recent commits on the current branch/base range; behavioral changes with meaningful blast radius; full caller/downstream tracing for suspicious changes.
- Out of scope: style issues, speculative concerns without a concrete trigger, minor UX degradation, broad refactors unrelated to a confirmed critical bug.

## Acceptance Criteria

- [x] Recent commits and their changed files are inspected.
- [x] Suspicious high-impact changes are traced through callers and downstream effects.
- [x] A critical bug is fixed only if a concrete trigger scenario is confirmed; otherwise no PR is opened.
- [x] Findings or "no critical bugs found" summary is posted to Slack.

## Risks

- Risk: false positive report or unnecessary PR.
  - Impact: reviewer churn and potential behavior drift.
  - Mitigation: require a concrete trigger scenario before fixing or opening a PR.
- Risk: shallow diff-only review misses a downstream failure.
  - Impact: critical issue remains undetected.
  - Mitigation: inspect caller chains and execution paths for selected high-blast-radius changes.

## Validation Plan

- Required tests: targeted desktop assistant unit tests for Life Pulse command workspace propagation and growth anchor behavior; root workspace clippy; task artifact validation.
- Commands to run: `cargo fmt --check`, `python3 scripts/validate_tasks.py`, `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml life_pulse --lib`, `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml evolution_ui::growth --lib`, `cargo clippy --all-targets -- -D warnings`.
- Manual checks: traced Life Pulse due checks, subprocess launch arguments, CLI workspace defaults, and A9 periodic anchor semantics.

## Regression Scope

- Areas likely affected: desktop Life Pulse growth and rhythm background subprocesses; desktop A9 periodic growth diagnostics.
- Explicit non-goals: changing manual evolution trigger behavior, schedule file semantics, or unrelated desktop bridge integrations.

## Links

- Source TODO section: N/A - daily automation request.
- Related PRs/issues: N/A at task start.
- Related docs: `spec/verification-integrity.md`, `spec/README.md`.
