# TASK Card

## Metadata

- Task ID: `TASK-2026-071`
- Title: Extract desktop Assistant into a standalone project
- Status: `in_progress`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-09-25`
- Target milestone:

## Problem

User request (ZH): 「把桌面的模块独立出来到一个新的项目」.

English interpretation: move SkillLite Assistant (`crates/skilllite-assistant`) out of the engine crate tree into a standalone desktop project (P4 of the Assistant split plan). The engine repo should treat Desktop as an optional GUI that talks to a released `skilllite` binary, not as a crate under `crates/`.

## Scope

- In scope:
  - Relocate the Tauri/React app from `crates/skilllite-assistant/` to top-level `skilllite-assistant/`
  - Leave a stub at the old path for one release cycle
  - Decouple prebuild / debug-binary lookup from a hard-coded monorepo layout
  - Update engine CI, deny comments, root npm scripts, EN/ZH architecture and start-path docs
- Out of scope:
  - Creating a separate GitHub repository (this environment cannot create remotes)
  - Rewriting remaining L3 file helpers into new CLI commands
  - Changing agent-rpc or CLI `--json` contracts

## Acceptance Criteria

- [ ] Desktop sources live at `skilllite-assistant/` (own README, npm, Tauri manifest)
- [ ] `crates/skilllite-assistant/README.md` is a stub pointing at the new project
- [ ] Prebuild can use `SKILLLITE_ENGINE_ROOT`, a detected engine checkout, or an already-installed `skilllite` binary
- [ ] Debug binary lookup no longer assumes `crates/skilllite-assistant/src-tauri`
- [ ] EN/ZH docs, CI, deny comments, and root `package.json` use the new path
- [ ] Assistant Rust tests and frontend unit tests pass; engine `cargo test` still passes
- [ ] `python3 scripts/validate_tasks.py` passes

## Risks

- Risk: Contributors keep using `crates/skilllite-assistant` after the move
  - Impact: Broken local scripts and stale docs
  - Mitigation: Stub README + EN/ZH path updates + root npm prefixes
- Risk: Standalone checkout cannot `cargo install --path skilllite`
  - Impact: Desktop prebuild fails
  - Mitigation: Fall back to PATH / `~/.skilllite/bin`; optional engine-root env
- Risk: Debug `target/debug/skilllite` path changes after the move
  - Impact: `tauri dev` picks the wrong binary
  - Mitigation: Walk for engine `skilllite/Cargo.toml` or `SKILLLITE_ENGINE_ROOT`

## Validation Plan

- Required tests:
  - `cargo test --manifest-path skilllite-assistant/src-tauri/Cargo.toml`
  - `cd skilllite-assistant && npm run test:llm-fallback`
  - `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` (engine workspace)
- Commands to run:
  - `python3 scripts/validate_tasks.py`
  - `cargo deny check bans`
  - `cargo deny --manifest-path skilllite-assistant/src-tauri/Cargo.toml check bans` (if cargo-deny is installed)
- Manual checks:
  - Re-read stub, new README, and docs for leftover `crates/skilllite-assistant` user-facing paths

## Regression Scope

- Areas likely affected:
  - Desktop prebuild, release-desktop workflow paths, deny CI job, contributor start paths
- Explicit non-goals:
  - Engine CLI/MCP behavior, Python SDK, sandbox policy

## Links

- Source TODO section: user request to extract desktop into a new project
- Related PRs/issues:
- Related docs: `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`, `docs/zh/ASSISTANT-SPLIT-ARCHITECTURE.md`
