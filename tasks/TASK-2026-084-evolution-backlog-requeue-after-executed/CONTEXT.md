# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-evolution/src/feedback.rs` — schema `evolution_backlog.dedupe_key TEXT NOT NULL UNIQUE`
  - `crates/skilllite-evolution/src/scope.rs` — `upsert_backlog_proposal`, `enqueue_user_capability_evolution`, `coordinate_proposals*`
  - `crates/skilllite-evolution/src/run.rs` — forced proposal load / recovery → `NoScope` when missing
- Current behavior:
  - INSERT OR IGNORE + UPDATE `WHERE status != 'executed'`
  - After executed history exists, both arms no-op; enqueue returns mint-only ID

## Architecture Fit

- Layer boundaries involved: evolution persistence only; desktop/CLI already pass proposal_id through.
- Interfaces to preserve: `enqueue_user_capability_evolution` signature and backlog column set.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: migrate existing UNIQUE tables; new installs create non-unique `dedupe_key`.

## Design Decisions

- Decision: remove global UNIQUE on `dedupe_key`; keep application soft-dedupe for non-executed rows.
  - Rationale: matches existing UPDATE filter intent and preserves executed history.
  - Alternatives considered:
    - Reset executed row in place (loses acceptance history / timestamps).
    - Encode run generation into dedupe_key (breaks soft-dedupe and existing keys).
  - Why rejected: both destroy governance history or widen blast radius.

- Decision: after upsert, remap selected coordinator proposal_id to latest non-executed DB id.
  - Rationale: soft-dedupe leaves the original proposal_id on the row; status updates by mint id no-op.

## Open Questions

- [x] Should multiple executed historical rows share a dedupe_key? Yes — that is the goal.
- [x] Change docs? No — internal persistence fix; no command/env surface change.
