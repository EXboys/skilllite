# Technical Context

## Current State

- Relevant crates/files (post-TASK-2026-043):
  - `crates/skilllite-services/{Cargo.toml,src/lib.rs}` — empty bootstrap crate, only re-exports `BOOTSTRAP_PHASE`.
  - `crates/skilllite-commands/src/skill/common.rs::resolve_skills_dir` — sync wrapper that calls `skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback`, prints conflict warning via `eprintln!`, and returns `effective_path`.
  - `crates/skilllite-commands/src/init.rs::resolve_path_with_legacy_fallback` — same pattern.
  - `crates/skilllite-commands/src/ide.rs::resolve_skills_dir_with_legacy_fallback` (private) — wraps core call without printing; `cmd_cursor` and `cmd_opencode` print the warning at the callsite.
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs::resolve_workspace_skills_root` — wraps core call and silently drops the conflict warning.
  - `deny.toml` — Phase 0 D2 rule for `skilllite-services` with wrappers `skilllite`, `skilllite-commands`, `skilllite-assistant`; until this TASK, all 3 wrappers were unmatched in both graphs.
- Current behavior:
  - CLI prints conflict warning to stderr.
  - Desktop silently drops conflict warning.
  - Both CLI and Desktop fall back to `.skills/` when only the legacy directory exists.

## Architecture Fit

- Layer boundaries involved:
  - Per Phase 0 D2, `skilllite-services` may be depended on only by entry-layer crates (`skilllite`, `skilllite-commands`, `skilllite-assistant`); the deny rule was pre-declared in Phase 0 and is now activated by real consumption.
  - Service depends downward on `skilllite-core` only; no upward dependencies.
- Interfaces to preserve:
  - All CLI subcommand stdout/stderr contracts.
  - All Tauri commands exposed by `skilllite_bridge`.
  - All MCP tool schemas.
  - All Python SDK subprocess/IPC behavior.

## Dependency and Compatibility

- New dependencies:
  - `skilllite-services` adds `serde` (with `derive`), workspace `thiserror`, `skilllite-core` (path), and `tempfile` (dev-only).
  - `skilllite-commands` adds `skilllite-services` (path).
  - `skilllite-assistant/src-tauri` adds `skilllite-services` (path).
  - No new third-party crates outside the workspace.
- Backward compatibility notes:
  - Output text and exit codes unchanged.
  - `cargo deny check bans` `unused-wrapper` warnings reduce (real consumers now match the pre-declared wrappers); the `bans ok` outcome is unchanged.

## Design Decisions

- Decision — Sync `WorkspaceService` (Phase 0 D3 documented exception).
  - Rationale: Every operation is local-filesystem read; no network, no spawn, no long-running blocking. Async-by-default would only force CLI commands into `block_on` boilerplate without any concurrency benefit.
  - Alternatives considered: `async fn` body that synchronously executes filesystem work.
  - Why rejected: Lies to the type system, requires every CLI caller to spin up a tokio runtime per call, and produces no real benefit since downstream callers do not concurrently invoke this service.

- Decision — Adapter `unwrap_or_else` fallback that re-calls `skilllite_core::skill::discovery` on `Err`.
  - Rationale: The service rejects only invariant violations (empty workspace_root, blank skills_dir_arg). Production callers already guarantee non-empty inputs; the fallback exists so a future invariant tightening on the service does not crash CLI commands and instead degrades gracefully to the previous behaviour, with a `tracing::debug!` line to surface the divergence.
  - Alternatives considered: `expect()` / panic on service `Err`.
  - Why rejected: Would convert a service-side validation tightening into a CLI panic, violating `spec/verification-integrity.md` anti-false-positive guidance.

- Decision — Preserve Desktop "silent drop" of conflict warning.
  - Rationale: This refactor TASK explicitly preserves observable behaviour. The Desktop crate does not depend on `tracing`; adding a logging dependency in this TASK would be a scope expansion.
  - Alternatives considered: Add `tracing` dep to Desktop and log at debug level.
  - Why rejected: Adds dependency surface for a side-effect that is irrelevant to current users; future TASK can route `conflicting_skill_names` to a structured assistant UI channel instead.

- Decision — Drive-by fix: convert `init.rs::cwd_is_untrusted_for_relative_skills` from `return cwd == ...` to `cwd == ...` in the `#[cfg(unix)]` and `#[cfg(windows)]` arms.
  - Rationale: This pre-existing `clippy::needless_return` warning was the only blocker to running `cargo clippy --workspace --all-targets -- -D warnings`; fixing it inside this TASK avoids future false-attribution to this PR.
  - Alternatives considered: Leave it for a separate cleanup TASK.
  - Why rejected: Trivially small and inside an already-touched file; would only delay green-light verification.

## Open Questions

- [ ] Should a future TASK route `conflicting_skill_names` to a structured Desktop UI notification (e.g. tray badge or settings warning)? — out of scope here.
- [ ] Should `skilllite-services` adopt `lints.workspace = true` once the workspace defines a `[workspace.lints]` table? — already noted in TASK-2026-043 CONTEXT; still deferred.
