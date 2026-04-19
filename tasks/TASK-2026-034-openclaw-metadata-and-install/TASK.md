# TASK Card

## Metadata

- Task ID: `TASK-2026-034`
- Title: Structured OpenClaw metadata and install[] support
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-19`
- Target milestone:

## Problem

SKILL.md `metadata` parsing only merged a small slice of OpenClaw / ClawHub
extensions (`metadata.openclaw.requires.bins` and `requires.env`) into
`compatibility`. The full ClawHub field set
(<https://github.com/openclaw/clawhub/blob/main/docs/skill-format.md>) — aliases
`clawdbot` / `clawdis`, `requires.anyBins` / `requires.config`, `primaryEnv`,
`os`, `skillKey`, `always`, and the declarative `install[]` spec — was not
covered. Because OpenClaw `install[]` was ignored, skills that declare their
node/uv dependencies structurally produced empty package lists in SkillLite,
forcing users to duplicate them in `compatibility` text.

## Scope

- In scope:
  - Move OpenClaw merge logic out of `skill::metadata` into a dedicated
    `skill::openclaw_metadata` module.
  - Cover ClawHub aliases (`openclaw`, `clawdbot`, `clawdis`) with deterministic
    selection.
  - Surface `requires.bins` / `anyBins` / `env` / `config`, `primaryEnv`, `os`,
    `skillKey`, `always`, and `install[]` summary in `compatibility`.
  - Expose structured `OpenClawInstalls` on `SkillMetadata` and route `node` /
    `uv` install entries into `deps::detect_dependencies` and
    `evolution::env_helper` so SkillLite installs them via the existing
    npm / pip pipeline.
- Out of scope:
  - Executing `brew` / `go` / unknown install kinds (would require host
    package managers; logged only).
  - A separate `uv pip` runtime (current pip path is reused for `kind: uv`).
  - Rewriting `skilllite-commands` import flow (`import_openclaw.rs`) — its
    behavior already feeds `metadata` parsing.

## Acceptance Criteria

- [x] `metadata.openclaw.requires.bins/env/anyBins/config`, `primaryEnv`, `os`,
      `skillKey`, `always`, and `install[]` are folded into `compatibility`
      output via the new module.
- [x] ClawHub aliases (`openclaw`, `clawdbot`, `clawdis`) are recognized; when
      multiple are present the first one carrying merge-relevant signals wins,
      with `openclaw` preferred over later aliases.
- [x] `SkillMetadata.openclaw_installs` exposes structured node/python/system
      packages parsed from `install[]`.
- [x] `deps::detect_dependencies` returns `node` packages for `kind: node`
      and `python` packages for `kind: uv` when no lock / whitelist match
      exists; `brew` / `go` / unknown kinds are recorded but not installed.
- [x] `evolution::env_helper::ensure_skill_deps_and_env` falls back to the
      structured installs when the existing resolver path produced nothing.
- [x] Unit tests cover: alias selection, install summary, structured
      classification, deps fallback (node/python/brew-only/compatibility wins).
- [x] `cargo test -p skilllite-core`, `cargo test -p skilllite-evolution`,
      and `cargo test -p skilllite-agent` pass.

## Risks

- Risk: Structured installs bypass the SkillLite package whitelist gate.
  - Impact: Skills can declare arbitrary npm/pip packages and have them
    installed without `--allow-unknown-packages`.
  - Mitigation: Logged at `info` level; behavior is intentional because the
    declaration is explicit. Future toggle can re-enable whitelist gating.
- Risk: `kind: brew` / `kind: go` may surprise users expecting auto-install.
  - Impact: Skills that depend on host binaries still need manual install.
  - Mitigation: `tracing::info!` at parse, install summary written into
    `compatibility` text, documented in EN/ZH ARCHITECTURE notes.

## Validation Plan

- Required tests:
  - `cargo test -p skilllite-core`
  - `cargo test -p skilllite-evolution -p skilllite-agent`
- Commands to run:
  - `cargo clippy -p skilllite-core --all-targets -- -D warnings`
  - `cargo clippy -p skilllite-evolution -- -D warnings`
  - `cargo clippy -p skilllite-agent --all-targets -- -D warnings`
- Manual checks:
  - Inspect a fixture SKILL.md with `metadata.openclaw.install: [{kind: node, package: openai}]`
    and confirm `detect_dependencies` returns Node packages.

## Regression Scope

- Areas likely affected:
  - `skilllite-core::skill::metadata` (merge call site, new field).
  - `skilllite-core::skill::deps` (added structured install fallback).
  - `skilllite-evolution::skill_synth::env_helper` (added fallback path).
  - `skilllite-agent` and `skilllite-commands` test fixtures that build
    `SkillMetadata` literally.
- Explicit non-goals:
  - No change to `.skilllite.lock` schema or hashing.
  - No change to `ImportOpenClawSkills` CLI.

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
  - <https://github.com/openclaw/clawhub/blob/main/docs/skill-format.md>
  - `crates/skilllite-core/src/skill/openclaw_metadata.rs`
