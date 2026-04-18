# TASK Card

## Metadata

- Task ID: `TASK-2026-033`
- Title: CLI: import OpenClaw-style skills
- Status: `done`
- Priority: `P2`
- Owner: `maintainer`
- Contributors:
- Created: `2026-04-18`
- Target milestone:

## Problem

Users with OpenClaw-style skill layouts had to manually copy trees into SkillLite `skills/`. A single command reduces friction and aligns with Hermes-style multi-root discovery.

## Scope

- In scope: CLI `import-openclaw-skills`; scan common OpenClaw paths; conflict policy; dry-run; reuse admission scan + manifest + deps like `add`.
- Out of scope: Full OpenClaw config/memory migration; GUI; Hermes-specific paths.

## Acceptance Criteria

- [x] Command copies skills from workspace `*/skills`, `*/.agents/skills`, `~/.openclaw/skills`, legacy `~/.clawdbot/skills`, `~/.moltbot/skills`, `~/.agents/skills` into resolved `skills_dir`.
- [x] Duplicate skill **names** across sources use precedence (workspace before `workspace-main`, etc.; user-global paths last).
- [x] `--skill-conflict` supports skip / overwrite / rename; `--dry-run` lists planned installs.
- [x] EN/ZH README list the command; unit tests cover precedence and rename.

## Risks

- Risk: Paths differ across OpenClaw versions.
  - Impact: Some installs may need `--workspace` / `--openclaw-dir`.
  - Mitigation: Document defaults; Hermes-aligned root list.

## Validation Plan

- Required tests: `cargo test -p skilllite-commands import_openclaw`
- Commands to run: `cargo run -p skilllite -- import-openclaw-skills --help`
- Manual checks: `--dry-run` against a fixture tree

## Regression Scope

- Areas likely affected: `skilllite-commands` skill add/discovery visibility (`pub(in crate::skill)`).
- Explicit non-goals: Changing `skilllite add` behavior.

## Links

- Related docs: README.md, docs/zh/README.md
