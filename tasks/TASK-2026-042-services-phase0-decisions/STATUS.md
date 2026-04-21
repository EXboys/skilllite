# Status Journal

## Timeline

- 2026-04-20:
  - Progress: TASK created via `bash scripts/new_task.sh`. D1..D5 decisions locked into `todo/multi-entry-service-layer-refactor-plan.md` and into this TASK's `CONTEXT.md` / `PRD.md`. `deny.toml` extended to add `skilllite-assistant` as an allowed wrapper for `skilllite-{agent,sandbox,evolution}` and to pre-declare the `skilllite-services` rule (D2). New CI step added that runs `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`. Both `cargo deny check bans` invocations executed locally and pass with `bans ok` (warnings limited to expected `unused-wrapper` for crates not present in the respective graph). `docs/en|zh/ENTRYPOINTS-AND-DOMAINS.md` and `docs/en|zh/ARCHITECTURE.md` updated to describe Desktop as a first-class entry, list its real direct path deps, and document the new dual `cargo deny` invocations.
  - Blockers: None.
  - Next step: Open PR; on merge, follow up by creating `services-phase1a-workspace` TASK to bootstrap the empty `skilllite-services` crate (workspace member + activates the pre-declared deny rule).

## Checkpoints

- [x] PRD drafted before implementation (or `N/A` recorded)
- [x] Context drafted before implementation (or `N/A` recorded)
- [x] Implementation complete
- [x] Tests passed
- [x] Review complete
- [x] Board updated
