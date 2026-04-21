# Status Journal

## Timeline

- 2026-04-20:
  - Progress: TASK created via `bash scripts/new_task.sh`. Reverted four callsites to pre-Phase-1A form (commands/skill/common.rs, commands/init.rs, commands/ide.rs, assistant/bridge/integrations/shared.rs). Removed `skilllite-services` deps from `skilllite-commands/Cargo.toml` and `skilllite-assistant/src-tauri/Cargo.toml`. Deleted `crates/skilllite-services/` in full. Removed the `skilllite-services` rule from `deny.toml` and updated the header comment to record the rollback. Marked TASK-2026-043 and TASK-2026-044 `superseded` and appended explanation notes in their `REVIEW.md`. Added a "事后回滚" block at the top of `todo/multi-entry-service-layer-refactor-plan.md`. Verified locally: `cargo check --workspace` clean; `cargo test --workspace` all suites green (same as pre-rollback minus the 6 services unit tests); `cargo clippy --workspace --all-targets -- -D warnings` clean (drive-by `init.rs:28` fix retained); `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings` clean; `cargo fmt --check` clean (workspace + Desktop); `cargo deny check bans` (root + Desktop) `bans ok`; `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` succeeds.
  - Blockers: None.
  - Next step: Open PR. No follow-up TASKs planned: the multi-entry service-layer plan in `todo/` is paused indefinitely. Phase 0 boundary work (TASK-2026-042) remains in force.

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
