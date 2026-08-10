# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Deny ConfirmRequired in SilentEventSink
- Status: `in_progress`
- Priority: `P0`
- Owner: `critical-bug-automation`
- Contributors:
- Created: `2026-08-10`
- Target milestone: critical-bug-sweep

## Problem

`SilentEventSink::on_confirmation_request` always returns `true`, including for
`RiskTier::ConfirmRequired`. That sink is used by pre-compaction memory flush
(`ChatSession::run_memory_flush_turn`) and swarm local execution
(`chat::run_single_task`). Sensitive `run_command` paths, L3 skill execution,
and network-skill confirmations can therefore run with no user approval.

This contradicts TASK-2026-024 (`RiskTier::ConfirmRequired` must not be
auto-approved) and `spec/security-nonnegotiables.md` (do not enable
auto-approval of dangerous operations by default).

## Scope

- In scope:
  - Change `SilentEventSink` to approve only `RiskTier::Low`.
  - Add unit regression coverage for Low vs ConfirmRequired.
  - Update tests that incorrectly relied on SilentEventSink approving ConfirmRequired.
- Out of scope:
  - ChatSession workspace chat-root split (deferred; needs memory/tool root alignment).
  - Broader swarm confirmation UX / RPC channel for remote nodes.
  - Docs rewrite beyond noting behavior if required by docs-sync.

## Acceptance Criteria

- [ ] `SilentEventSink` returns `true` only for `RiskTier::Low`.
- [ ] `SilentEventSink` returns `false` for `RiskTier::ConfirmRequired`.
- [ ] Regression unit tests cover both tiers.
- [ ] Existing tests that intended explicit approval use an approving sink, not SilentEventSink.
- [ ] `cargo test -p skilllite-agent` and required gates pass.
- [ ] Task artifacts + board updated; `python3 scripts/validate_tasks.py` passes.

## Risks

- Risk: Swarm / memory-flush turns that previously auto-ran ConfirmRequired tools now cancel.
  - Impact: Some background turns may report cancellation instead of executing high-risk ops.
  - Mitigation: Correct safety behavior; Low-risk confirmations still auto-approve for silent ops.

## Validation Plan

- Required tests: `cargo test -p skilllite-agent`
- Commands to run: `cargo fmt --check`, `cargo clippy -p skilllite-agent --all-targets -- -D warnings`, `cargo test -p skilllite-agent`, `python3 scripts/validate_tasks.py`
- Manual checks: N/A (unit-testable confirmation gate)

## Regression Scope

- Areas likely affected: `SilentEventSink`, `run_command` tests using SilentEventSink, memory flush / swarm silent paths.
- Explicit non-goals: No agent-loop or sandbox policy redesign.

## Links

- Source TODO section: critical-bug-investigation automation cron
- Related PRs/issues: TASK-2026-024, open PR backlog #112–#137
- Related docs: `docs/en/ENV_REFERENCE.md` (desktop auto-approve low only)
