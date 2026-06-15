# TASK Card

## Metadata

- Task ID: `TASK-2026-069`
- Title: Fix critical regression crash and workspace scoping bugs
- Status: `in_progress`
- Priority: `P0`
- Owner: `agent`
- Contributors:
- Created: `2026-06-15`
- Target milestone:

## Problem

Recent critical-bug sweeps found high-impact regressions and adjacent escapees:
UTF-8 byte slicing can panic on LLM/embedding error previews, desktop evolution
background runs can target a different workspace DB than the UI path, and high-risk
skill reference/bash documentation can enter prompts without the existing security
notice.

## Scope

- In scope:
  - Replace unsafe UTF-8 byte slicing in embedding and task-planning error previews.
  - Ensure desktop life-pulse and authorized capability background evolution runs pass the intended `--workspace`.
  - Apply the existing `SKILL.md` high-risk security notice to reference docs and bash-tool docs injected into prompts.
  - Add focused regression tests.
- Out of scope:
  - Broad evolution architecture refactors.
  - New security scanners or policy semantics beyond reusing the existing high-risk pattern helper.
  - Changes to CLI flags, environment variable names, or documented command syntax.

## Acceptance Criteria

- [ ] Non-ASCII embedding/task-planner error previews return errors instead of panicking.
- [ ] Desktop-triggered evolution runs use the same explicit workspace as the status/backlog/authorization paths.
- [ ] Prompt injections for high-risk reference docs and bash-tool `SKILL.md` include `SKILL_MD_SECURITY_NOTICE`.
- [ ] Regression tests cover the crash and prompt-security paths.
- [ ] Required validation commands are recorded with real output.

## Risks

- Risk: Desktop process spawning behavior changes.
  - Impact: Background growth or forced-proposal runs might fail if arguments are malformed.
  - Mitigation: Keep the existing command shape and only append `--workspace <workspace>`.
- Risk: Prompt notice placement may duplicate security text.
  - Impact: Slightly longer prompt context for risky skill docs.
  - Mitigation: Apply the same existing notice only when the scanned content has high-severity patterns.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-agent`
  - `cargo test -p skilllite`
  - `python3 scripts/validate_tasks.py`
  - Workspace baseline: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
- Commands to run:
  - Focused package tests first, then workspace checks after commit/push per automation branch rules.
- Manual checks:
  - Re-read modified Rust and task files.

## Regression Scope

- Areas likely affected:
  - `skilllite-agent` LLM error handling, task planning, and prompt construction.
  - Desktop bridge background evolution subprocesses.
  - Task workflow artifacts and board entry.
- Explicit non-goals:
  - Runtime sandbox policy behavior.
  - Python SDK behavior.
  - Evolution database schema changes.

## Links

- Source TODO section: N/A
- Related PRs/issues: Recent critical-bug sweeps around PR #95 and TASK-2026-067.
- Related docs: N/A; this preserves existing CLI/env/security semantics.
