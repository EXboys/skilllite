# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-agent/src/types/event_sink.rs` (`SilentEventSink`, `RiskTier`)
  - `crates/skilllite-agent/src/chat_session.rs` (memory flush uses SilentEventSink)
  - `crates/skilllite-agent/src/chat.rs` (`run_single_task` swarm path)
  - `crates/skilllite-agent/src/extensions/builtin/run_command.rs` (sets ConfirmRequired for sensitive/dangerous)
  - `crates/skilllite-agent/src/skills/executor.rs` (L3 / network ConfirmRequired)
- Current behavior: `SilentEventSink` always returns `true` from `on_confirmation_request`.

## Architecture Fit

- Layer boundaries involved: agent EventSink policy only.
- Interfaces to preserve: `EventSink::on_confirmation_request` signature; RiskTier enum.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: silent ConfirmRequired tools stop executing (cancelled). Low still auto-approves.

## Design Decisions

- Decision: Approve only `RiskTier::Low` in SilentEventSink.
  - Rationale: Matches TASK-2026-024 desktop auto-approve policy and RiskTier docs.
  - Alternatives considered:
    - Deny all confirmations in SilentEventSink (would break Low run_command memory-flush helpers).
    - Add a dedicated MemoryFlushEventSink with tool allowlist (better long-term, broader than this fix).
  - Why rejected: allowlist sink is larger scope; deny-all breaks intentional Low auto-approve.

## Open Questions

- [x] Should swarm get a real confirmation channel? Deferred follow-up.
- [x] ChatSession workspace chat-root still deferred (memory tools ignore workspace today).
