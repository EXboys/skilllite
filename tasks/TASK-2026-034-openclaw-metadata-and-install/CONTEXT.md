# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-core/src/skill/openclaw_metadata.rs` (new module).
  - `crates/skilllite-core/src/skill/metadata.rs` (call site, new field on
    `SkillMetadata`).
  - `crates/skilllite-core/src/skill/deps.rs` (Priority 2b structured fallback).
  - `crates/skilllite-evolution/src/skill_synth/env_helper.rs` (extra fallback
    when `resolve_packages_sync` returns nothing).
  - `crates/skilllite-agent/src/{prompt,capability_registry,capability_gap_analyzer}.rs`
    and `crates/skilllite-commands/src/execute.rs` (test/builder sites updated
    for the new field).
- Current behavior: OpenClaw `requires.bins` and `requires.env` were merged
  into `compatibility`. `install[]` and other ClawHub fields were ignored.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-core` owns parsing and dependency classification.
  - `skilllite-sandbox` owns environment building; `EnvSpec` consumes
    `metadata.resolved_packages` only.
  - `skilllite-evolution` is the only crate writing `meta.resolved_packages`
    pre-build, so it is the right place to materialise the structured fallback.
- Interfaces to preserve:
  - `SkillMetadata` is widely constructed in tests; new field is `Option<_>`
    so call sites only need an explicit `None` literal.
  - `DependencyInfo` shape unchanged.

## Dependency and Compatibility

- New dependencies: none (uses existing `serde_json`).
- Backward compatibility notes:
  - SKILL.md without `metadata.openclaw.*` returns the same `compatibility`
    string as before (only one extra alias lookup is added).
  - `.skilllite.lock` continues to be the highest-priority package source.

## Design Decisions

- Decision: Encode OpenClaw structured installs into a new struct rather
  than re-serialising into a synthetic `compatibility` line for downstream
  whitelist matching.
  - Rationale: `spec/structured-signal-first.md` — prefer structured signals
    over text reverse-parsing.
  - Alternatives considered: Append `node:openai` style tokens into
    `compatibility` and let the whitelist matcher pick them up.
  - Why rejected: Whitelist matching is keyed off curated lists, so it would
    silently drop arbitrary OpenClaw declarations and conflate inferred
    packages with declared ones.
- Decision: Do **not** auto-install `brew` / `go` kinds.
  - Rationale: `spec/security-nonnegotiables.md` — the SkillLite sandbox
    does not include host package managers, and silently mutating host state
    is not acceptable.
  - Alternatives considered: Shell out to `brew install` / `go install`.
  - Why rejected: Out of scope and outside the sandbox boundary.

## Open Questions

- [ ] Should structured installs still be subject to a (configurable)
      whitelist gate (`--allow-unknown-packages`-style)?
- [ ] Do we need a separate `uv pip` runtime path, or is the existing pip
      path acceptable for OpenClaw `kind: uv` skills?
