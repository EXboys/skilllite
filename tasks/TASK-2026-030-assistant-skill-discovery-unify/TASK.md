# TASK Card

## Metadata

- Task ID: `TASK-2026-030`
- Title: Unify assistant skill discovery with core
- Status: `done`
- Priority: `P1`
- Owner: `airlu`
- Contributors: `Cursor`
- Created: `2026-04-10`
- Target milestone: `assistant skill UX consistency`

## Problem

`skilllite-assistant` currently maintains its own skill discovery rules for
`.skills`, `skills`, `_evolved`, and `_pending`, while the core workspace
already defines canonical discovery behavior in
`skilllite_core::skill::discovery`. The duplicate logic creates drift risk and
already misses supported locations such as `.agents/skills` and `.claude/skills`.

## Scope

- In scope:
  - Add a core discovery helper for assistant/UI-facing skill instance enumeration.
  - Switch assistant skill listing/open/remove and pending-skill paths to core discovery.
  - Align assistant workspace root detection and bundled-skill seeding with core skill directory resolution.
  - Update desktop assistant documentation for the expanded skill directory support.
- Out of scope:
  - General refactors unrelated to skill discovery.
  - Changes to CLI skill loading semantics beyond extracting shared logic.

## Acceptance Criteria

- [x] Assistant skill list/open/remove flows no longer maintain `.skills` / `skills` / `_evolved` rules independently from core.
- [x] Assistant paths that need the canonical skills root use `skilllite_core::skill::discovery` fallback resolution instead of hardcoded `.skills`.
- [x] Desktop assistant documentation reflects the supported skill directory locations.

## Risks

- Risk:
  - Impact: Skill list/open/remove or pending review actions could stop finding existing skills if the new helper changes precedence incorrectly.
  - Mitigation: Keep behavior deterministic, add focused regression tests, and verify assistant + workspace flows after the refactor.

## Validation Plan

- Required tests: core discovery regression tests; assistant/unit validation for touched Rust crates; desktop frontend build.
- Commands to run: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo test -p skilllite-agent`, `cargo test -p skilllite`, `cargo test --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`, `npm run build`
- Manual checks: confirm assistant docs mention the supported skill directory locations and fallback behavior.

## Regression Scope

- Areas likely affected: assistant skill list/open/remove flows, evolution pending review UI, bundled skill seeding, workspace root resolution around skill folders.
- Explicit non-goals: changing transcript/event behavior or non-skill workspace features.

## Links

- Source TODO section: user-requested follow-up from architecture review
- Related PRs/issues: N/A
- Related docs: `README.md`, `docs/zh/README.md`, `crates/skilllite-assistant/README.md`
