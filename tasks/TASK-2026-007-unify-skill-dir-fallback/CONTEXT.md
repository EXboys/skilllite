# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-core/src/skill/discovery.rs`
  - `crates/skilllite-commands/src/init.rs`
  - `crates/skilllite-commands/src/skill/common.rs`
  - `crates/skilllite-commands/src/ide.rs`
  - `skilllite/src/mcp/mod.rs`
- Current behavior:
  - When default `skills` is missing, modules independently decide whether to fallback to `.skills`, with implementation differences.
  - Duplicate names across both directories currently lack explicit warnings.

## Architecture Fit

- Layer boundaries involved:
  - `skilllite-core` provides a pure path-resolution helper (lower layer, no upper-layer deps).
  - `skilllite-commands` and `skilllite` entry layers consume that helper.
- Interfaces to preserve:
  - Preserve existing semantics of the `--skills-dir` argument.
  - Preserve default fallback compatibility behavior.

## Dependency and Compatibility

- New dependencies:
  - None.
- Backward compatibility notes:
  - Keep default behavior: fallback to `.skills` only when default `skills` (including `./skills`) is requested and missing.
  - Warnings are additive observability only and do not change execution results.

## Design Decisions

- Decision:
  - Add a unified resolution result struct and resolver function in `skilllite-core::skill::discovery`.
  - Rationale:
    - Reuse spans both `commands` and `skilllite`, avoiding duplicated implementations.
  - Alternatives considered:
    - Add helper only in `skilllite-commands`, then make `skilllite` depend on `commands`.
  - Why rejected:
    - The helper is a foundational capability; `core` is a better long-term location for reuse and stable layering.

## Open Questions

- [ ] Should warning output be integrated into structured tracing in a follow-up (currently stderr only)?
- [ ] Should configurable priority be introduced in the future (not in this task)?
