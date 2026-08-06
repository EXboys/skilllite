# TASK-2026-082: Fix evolution disable/explain and OpenClaw migrate wrong roots; harden atomic_write staging

## Metadata

- Task ID: `TASK-2026-082`
- Title: Fix evolution disable/explain and OpenClaw migrate wrong roots; harden atomic_write staging
- Status: `done`
- Priority: `P0`
- Owner: `cursor-cloud`
- Contributors:
- Created: `2026-08-06`
- Target milestone:
- Branches: `cursor/critical-bug-investigation-b649`

## Summary

Three high-confidence critical bugs remain on `main` @ `12010e8` outside open PRs #89 / #123–#133:

1. `evolution disable` / `explain` still use global `paths::chat_root()` with no `--workspace`, so they mutate/read `~/.skilllite/chat` while status/run/pending use `<workspace>/chat`.
2. `claw migrate` / `migrate openclaw` accepts `--workspace` but writes OpenClaw memory into global `paths::chat_root()/memory` instead of `<workspace>/chat/memory`.
3. `skilllite_fs::atomic_write` still stages via `Path::with_extension("tmp")`, so sibling stems (e.g. `examples.json` / `examples.md`) share one temp path (same class as artifact fix in #133).

## Acceptance Criteria

- [x] `evolution disable` and `explain` accept `--workspace/-w` (default `.`) and operate only on that workspace's `chat/prompts` + evolution DB.
- [x] Regression test proves disable with `--workspace A` does not mutate workspace B or global `~/.skilllite/chat`.
- [x] OpenClaw migrate memory destination is `<workspace>/chat/memory` (plan display + apply + reindex).
- [x] Dry-run / apply evidence shows memory under the selected project chat root, not `~/.skilllite/chat`.
- [x] `skilllite_fs::atomic_write` uses basename-preserving unique staging names; unit test covers stem collision.
- [x] EN/ZH command docs updated for new `--workspace` on disable/explain.
- [x] `python3 scripts/validate_tasks.py` passes.

## Non-Goals

- Bugs already fixed in open PRs #89, #123–#133.
- Evolution `reset` / `repair-skills` workspace scoping (covered by open PRs #113 / #120).
- Honor `disabled` in planner typed model (PR #124).

## Risks

- Callers that relied on disable/explain hitting global chat without `SKILLLITE_WORKSPACE` will now default to cwd workspace `./chat` — intentional alignment with status/run.
- Migrate users who expected global memory destination need to use the project chat tree (correct for workspace-scoped agents).

## Validation Plan

- Focused CLI/unit tests for disable workspace isolation + atomic_write staging.
- Migrate dry-run PoC under isolated `HOME`.
- `cargo test` / clippy / fmt / `validate_tasks.py`.

## Regression Scope

- Evolution CLI disable/explain parsing + dispatch.
- OpenClaw migrate memory plan/apply/reindex paths.
- All `skilllite_fs::atomic_write` callers (evolution prompts, checkpoints, judgement).
