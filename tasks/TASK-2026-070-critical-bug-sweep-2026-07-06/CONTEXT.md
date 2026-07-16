# Technical Context

## Current State

- Relevant crates/files:
  - Initial scope is repository-wide recent commit review.
  - Candidate areas from recent history include evolution workspace scoping, agent/LLM UTF-8 truncation, dependency/security bumps, and sandbox CI coverage.
  - Confirmed fix file: `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/evolution_ui/authorize.rs`.
- Current behavior:
  - Branch `cursor/critical-bug-investigation-4841` started aligned with `main` at merge commit `b57e289`.
  - Desktop authorized capability evolution now passes the captured `proposal_id` to `skilllite evolution run --proposal-id <id>` so `cmd_run` preserves the forced proposal target.

## Architecture Fit

- Layer boundaries involved: Desktop assistant bridge invoking the CLI command layer.
- Interfaces to preserve: CLI behavior, workspace/root resolution semantics, sandbox/security invariants, and task artifact workflow.

## Dependency and Compatibility

- New dependencies: None planned.
- Backward compatibility notes: Existing CLI `--proposal-id` behavior is reused; the environment variable remains set as before for compatibility, but no longer carries the contract alone.

## Design Decisions

- Decision: Start as an investigation-only task and avoid implementation unless the severity/confidence bar is met.
  - Rationale: The automation explicitly expects "no critical bugs found" most days and forbids speculative PRs.
  - Alternatives considered: Proactively patch suspicious code.
  - Why rejected: Suspicion without a concrete trigger does not meet the task's confidence bar.
- Decision: Fix the desktop caller by passing `--proposal-id` instead of changing `cmd_run` environment cleanup semantics.
  - Rationale: `skilllite evolution run` already exposes a typed CLI argument for forced proposal execution, and the desktop caller has the captured proposal id at the spawn site.
  - Alternatives considered: Stop `cmd_run` from clearing `SKILLLITE_EVO_FORCE_PROPOSAL_ID` when the CLI arg is absent.
  - Why rejected: That would broaden the contract to env-only callers and risk stale process environment influencing unrelated manual runs.

## Open Questions

- [x] Which recent behavioral changes have enough blast radius to trace deeply?
- [x] Does any traced path produce a concrete severe failure scenario?
