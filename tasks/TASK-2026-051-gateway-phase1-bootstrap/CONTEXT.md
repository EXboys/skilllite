# Technical Context

## Current State

- Relevant crates/files:
  - `skilllite/src/cli.rs` (CLI command definitions).
  - `skilllite/src/dispatch/artifact.rs` and `skilllite/src/dispatch/channel_serve.rs` (entry-layer bindings).
  - `crates/skilllite-commands/src/channel_serve.rs` (current inbound webhook HTTP host).
  - `crates/skilllite-artifact/src/server.rs` / `serve.rs` (artifact router + standalone serve host).
  - `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md` (current channel/artifact architecture description).
  - `docs/en/ENV_REFERENCE.md`, `docs/zh/ENV_REFERENCE.md` (serve env vars and security wording).
  - `todo/multi-entry-service-layer-refactor-plan.md` and `TASK-2026-045` (prior shared-service extraction rollback context).
- Current behavior:
  - `skilllite channel serve` starts a dedicated Axum server for `/health` and `/webhook/inbound`, gated by `SKILLLITE_CHANNEL_SERVE_ALLOW=1`.
  - `skilllite artifact-serve` starts a separate Axum host for `/v1/runs/{run_id}/artifacts`, gated by `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`.
  - The domain crates are already split sensibly: `skilllite-artifact` provides artifact store/router/server APIs; `skilllite-channel` is outbound-focused, while inbound webhook HTTP still lives in `skilllite-commands`.
  - Assistant currently guides users to `channel serve`, not to a gateway host.

## Architecture Fit

- Layer boundaries involved:
  - Current documented chain: `entry -> commands -> agent -> executor -> sandbox -> core`.
  - This task stays entry-host focused and does not introduce a revived `skilllite-services` layer.
  - The new gateway command should still be wired through entry/commands, not by making lower-layer crates depend on CLI-specific code.
- Interfaces to preserve:
  - Existing CLI flags and behavior for `skilllite channel serve`.
  - Existing CLI flags and behavior for `skilllite artifact-serve`.
  - Existing artifact HTTP API path contract.
  - Existing webhook MVP semantics for `/webhook/inbound`.

## Dependency and Compatibility

- New dependencies:
  - Prefer reusing already-present Axum/tower-http/tokio dependencies in `skilllite-commands`.
  - No new third-party networking stack should be introduced for this task.
- Backward compatibility notes:
  - The gateway command is additive in this phase.
  - Old serve commands remain functional and documented as compatibility paths.
  - Assistant may continue referencing the old path until a later migration task.

## Design Decisions

- Decision: Bootstrap gateway as a unified host command before any deep domain migration.
  - Rationale: Hosting consolidation is the smallest useful step toward a future gateway model and avoids the abstraction mismatch that caused the `skilllite-services` rollback.
  - Alternatives considered:
    - Keep only separate `channel serve` + `artifact-serve`.
    - Create a brand-new shared service layer first.
    - Create a brand-new standalone crate and migrate all HTTP behavior immediately.
  - Why rejected:
    - Keeping only separate hosts does not move the product toward the intended long-term operating model.
    - A fresh shared service layer was already proven too speculative for the current codebase.
    - A full HTTP migration in one task is too large and risks breaking working surfaces.

- Decision: Treat gateway as the host, not as a reason to merge domain crates.
  - Rationale: `artifact` and `channel` represent different capability domains; they should remain independently testable and reusable even if one host process serves them.
  - Alternatives considered:
    - Fold artifact code into gateway.
    - Move all inbound channel logic directly into `skilllite-channel` during the same task.
  - Why rejected:
    - Folding domain code into the host would blur boundaries and reduce reuse.
    - Moving inbound channel logic and adding gateway simultaneously would make the task too broad.

- Decision: Preserve fail-closed defaults on the new gateway host.
  - Rationale: Combining two HTTP surfaces under one process must not weaken the current security stance.
  - Alternatives considered:
    - Auto-bind without a dedicated allow env because gateway is now “the main host”.
    - Allow non-loopback binds without token by default for operator convenience.
  - Why rejected:
    - Both options would violate the repo's current defensive serving pattern and increase accidental exposure risk.

## Open Questions

- [ ]
- [ ] Should the Assistant settings page be migrated directly to `gateway serve` in a follow-up, or should gateway remain a more operator-facing path first?
- [ ] In a later phase, should inbound webhook router code move into a dedicated reusable crate/module, or is shared hosting enough for now?
