# TASK Card

## Metadata

- Task ID: `TASK-2026-051`
- Title: Gateway bootstrap and host unification phase 1
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors:
- Created: `2026-04-24`
- Target milestone:

## Problem

SkillLite currently exposes inbound messaging (`skilllite channel serve`) and artifact HTTP (`skilllite artifact-serve`) as separate long-running entry points. This is acceptable for MVP, but it does not match the intended future operating model for multi-platform, cross-session, or edge-style deployments where one always-on gateway should host multiple HTTP surfaces with shared lifecycle, tracing, and security posture.

The repository also already carries evidence that a generic shared service layer can be over-abstracted too early (`TASK-2026-045`). This task therefore needs a bounded, host-focused step that introduces a unified gateway entry without forcing an immediate cross-crate rewrite.

## Scope

- In scope:
  - Add a new `skilllite gateway serve` command as the first unified HTTP host entry.
  - Mount a common health endpoint plus existing inbound webhook behavior under the gateway host.
  - Allow the gateway host to optionally mount artifact HTTP routes backed by a local artifact directory.
  - Keep `skilllite channel serve` and `skilllite artifact-serve` working as compatibility paths in this phase.
  - Update architecture and environment docs in both EN and ZH to describe the new gateway host model and compatibility story.
  - Add regression coverage for the new CLI surface and gateway-specific guardrails.
- Out of scope:
  - Migrating the Assistant settings UI from `channel serve` to `gateway serve`.
  - Replacing the existing `channel serve` or `artifact-serve` commands.
  - Implementing multi-platform session routing, multi-tenant routing, or WebSocket control-plane behavior.
  - Creating a new shared `skilllite-services` layer or reviving the rolled-back services plan.
  - Converting Python SDK or MCP to consume the gateway.

## Acceptance Criteria

- [x] `skilllite gateway serve` exists and can host `GET /health` plus `POST /webhook/inbound` behind a single process with explicit bind gating.
- [x] The gateway can optionally expose artifact HTTP routes from a configured local artifact directory without breaking the standalone `artifact-serve` path.
- [x] Existing `skilllite channel serve` and `skilllite artifact-serve` remain available in this phase and their documented security posture stays fail-closed by default.
- [x] EN/ZH architecture and environment docs describe the new gateway entry, its relationship to channel/artifact, and the compatibility status of old commands.
- [x] Required task/code validation passes, including task validation, formatting/lints/tests, and at least one integration test covering the new CLI surface.

## Risks

- Risk: Gateway bootstrap becomes an accidental “big-bang” migration.
  - Impact: Existing channel/artifact users break, or the change repeats the complexity that caused the services rollback.
  - Mitigation: Keep the new gateway additive; preserve old commands and explicitly mark deeper unification as future follow-up work.
- Risk: Security policy becomes more permissive while merging hosts.
  - Impact: A new combined HTTP host could accidentally weaken loopback/auth rules.
  - Mitigation: Reuse the current fail-closed posture: explicit allow env for binding, non-loopback token requirement unless an explicit insecure override is set, and tests for refusal paths.
- Risk: Docs drift and users misunderstand whether they should use gateway or old commands.
  - Impact: Operator confusion and stale Assistant/user guidance.
  - Mitigation: Update EN/ZH docs together and describe this phase as “new preferred host, old commands retained for compatibility.”

## Validation Plan

- Required tests:
  - `python3 scripts/validate_tasks.py`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `cargo test -p skilllite`
- Manual checks:
  - Re-read CLI help/output and confirm `gateway` appears as a distinct command surface.
  - Re-read EN/ZH docs to confirm gateway/channel/artifact terminology is consistent.
  - Re-read `tasks/board.md` after status updates.

## Regression Scope

- Areas likely affected:
  - `skilllite/src/cli.rs`
  - `skilllite/src/dispatch/**`
  - `crates/skilllite-commands/src/**`
  - `skilllite/Cargo.toml`
  - `docs/en/ARCHITECTURE.md`
  - `docs/zh/ARCHITECTURE.md`
  - `docs/en/ENV_REFERENCE.md`
  - `docs/zh/ENV_REFERENCE.md`
  - `tasks/TASK-2026-051-gateway-phase1-bootstrap/*`
- Explicit non-goals:
  - No Assistant settings migration in this task.
  - No Python SDK integration changes.
  - No platform-specific channel adapters beyond the current inbound webhook MVP.
  - No websocket/dashboard/control-plane work.

## Links

- Source TODO section:
  - Follow-up to the gateway/channel/artifact direction discussed in April 2026 architecture review chats.
- Related PRs/issues:
- Related docs:
  - `todo/multi-entry-service-layer-refactor-plan.md`
  - `spec/architecture-boundaries.md`
  - `spec/security-nonnegotiables.md`
  - `spec/docs-sync.md`
  - `spec/testing-policy.md`
