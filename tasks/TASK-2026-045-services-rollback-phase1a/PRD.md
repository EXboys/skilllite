# PRD

## Background

After landing Phase 0 (TASK-2026-042), Phase 1A bootstrap (TASK-2026-043), and Phase 1A real (TASK-2026-044), a self-review under `spec/verification-integrity.md` produced two findings that contradict the original plan's premise:

1. The first real service extraction (`WorkspaceService`) **increased** caller LOC rather than reducing it, because the new service returned `Result<…>` over an underlying infallible function, forcing every caller into `unwrap_or_else` boilerplate.
2. A grep-driven verification of upcoming phases revealed that the cross-entry "duplication" they were meant to consolidate is much smaller than initially estimated. Phase 1B's CLI consumers do not exist; Phase 2's overlap is mostly primitive-call level over an already well-shaped `skilllite-evolution` crate.

Continuing on momentum would be pattern-driven, not evidence-driven. This TASK reverses the over-abstraction while preserving the genuinely independent Phase 0 boundary improvements.

## Objective

Restore the four callsites and the `deny.toml` to their pre-`skilllite-services` shape; delete the empty crate; preserve all Phase 0 boundary work; record the decision in the audit trail so future contributors do not silently re-attempt the same extraction.

## Functional Requirements

- FR-1: Four callsites (commands/skill/common.rs, commands/init.rs, commands/ide.rs, assistant/bridge/integrations/shared.rs) revert to calling `skilllite_core::skill::discovery` directly.
- FR-2: `crates/skilllite-services/` is deleted in full (Cargo.toml + src/{lib,error,workspace}.rs + the empty `src/` and crate dirs).
- FR-3: `Cargo.toml` of `skilllite-commands` and `skilllite-assistant/src-tauri` no longer reference `skilllite-services`.
- FR-4: `deny.toml` no longer contains a `skilllite-services` rule; the header comment is updated to explain the rollback.
- FR-5: TASK-2026-043 and TASK-2026-044 are marked `superseded` with explanatory notes in their `REVIEW.md`.
- FR-6: `todo/multi-entry-service-layer-refactor-plan.md` carries a "事后回滚" block at the top documenting the decision and reasons.
- FR-7: Phase 0 work (TASK-2026-042) remains untouched.

## Non-Functional Requirements

- Security: No security-relevant change.
- Performance: No runtime impact.
- Compatibility: All CLI subcommands, Tauri commands, MCP tool schemas, and Python SDK behaviour unchanged.

## Constraints

- Technical:
  - Must comply with `spec/verification-integrity.md` — the rollback IS the application of the anti-false-positive checklist after a self-review.
  - Must comply with `spec/architecture-boundaries.md` — the Phase 0 deny.toml entries that allow Desktop to consume `skilllite-{agent,sandbox,evolution}` directly must remain in place.
  - Must comply with `spec/task-artifact-language.md` — English task artifacts.
- Timeline: Single PR.

## Success Metrics

- Metric: Net LOC change relative to the state immediately before TASK-2026-043.
  - Baseline: pre-TASK-2026-043 LOC.
  - Target: ±0 (modulo the preserved drive-by clippy fix and rollback audit-trail comments).
- Metric: `cargo test --workspace` outcome.
  - Baseline: pass after TASK-2026-044.
  - Target: pass after this rollback (same suites green).
- Metric: `cargo deny check bans` outcomes.
  - Baseline: 2 invocations both `bans ok` after TASK-2026-044.
  - Target: 2 invocations both `bans ok` after this rollback.

## Rollout

- Rollout plan: Single PR; no version bump; no observable behaviour change.
- Rollback plan: If the rollback itself proves wrong (e.g. external consumer depended on `skilllite-services`), revert this TASK's PR and reinstate TASK-2026-043 / TASK-2026-044 changes. Verified low risk: `skilllite-services` was never published.
