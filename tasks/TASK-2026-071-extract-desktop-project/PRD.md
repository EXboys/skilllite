# PRD

## Background

SkillLite Assistant is already a thin Tauri client over `skilllite` (L1 `agent-rpc`, L2 CLI `--json`). It still lives under `crates/`, which implies it is an engine crate. Split architecture P2 (no path deps on `skilllite-{agent,sandbox,evolution}`) is done. P4 is to extract the desktop tree into its own project.

## Objective

- Desktop is a first-class standalone project at `skilllite-assistant/` that can later be published as its own repository.
- Engine documentation and CI treat Desktop as an optional GUI distribution, not a `crates/*` member.

## Functional Requirements

- FR-1: All Assistant source, frontend, and Tauri config live under `skilllite-assistant/`.
- FR-2: Old path `crates/skilllite-assistant/` contains only a pointer stub.
- FR-3: Prebuild installs or copies a `skilllite` binary without requiring the assistant tree to sit two levels below the engine root.
- FR-4: Debug resolution of `target/debug/skilllite` works when the assistant is a sibling of `skilllite/` (monorepo) or when `SKILLLITE_ENGINE_ROOT` is set.
- FR-5: User-facing EN/ZH docs describe the new layout.

## Non-Functional Requirements

- Security: No new in-process engine crate links; deny policy stays subprocess-only (D1′).
- Performance: Unchanged chat spawn model.
- Compatibility: Existing installers and `min_skilllite_version` behavior remain; only source layout and prebuild discovery change.

## Constraints

- Technical: Cannot create `github.com/EXboys/skilllite-assistant` from this environment. Extract stays in-tree as a standalone project ready for subtree split.
- Timeline: N/A (agent execution).

## Success Metrics

- Metric: User-facing docs and CI paths point at `skilllite-assistant/`
- Baseline: Desktop lives in `crates/skilllite-assistant/`
- Target: Desktop project root is `skilllite-assistant/`; engine workspace excludes that directory

## Rollout

- Rollout plan: Land the move in this repo; keep a stub for one cycle; optional later `git subtree split` to a new remote.
- Rollback plan: Revert the move commit; restore previous paths.
