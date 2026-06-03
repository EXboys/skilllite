# Technical Context

## Current State

- Relevant crates/files: `crates/skilllite-evolution/src/skill_synth/mod.rs`
  owns pending skill confirm/reject; `crates/skilllite-commands/src/evolution_desktop.rs`
  owns desktop pending skill reads and delegates confirm/reject; `skilllite/src/dispatch/mod.rs`
  exposes CLI dispatch.
- Current behavior: callers provide a `skill_name` string that is joined onto
  `_evolved/_pending` without checking whether it is a single safe path segment.

## Architecture Fit

- Layer boundaries involved: entry CLI -> commands -> evolution library. The
  invariant belongs in `skilllite-evolution` for mutating operations and in
  `skilllite-commands` for the read helper that constructs its own path.
- Interfaces to preserve: command names, JSON output shape, and successful
  operation semantics for safe pending skill directory names.

## Dependency and Compatibility

- New dependencies: none.
- Backward compatibility notes: names containing path separators were never
  valid entries returned by pending skill listing, so rejecting them closes an
  unsafe input path without changing listed pending skill behavior.

## Design Decisions

- Decision: validate identifiers as single normal path components before any
  pending skill filesystem operation.
  - Rationale: this directly addresses absolute-path and `..` traversal while
    keeping the accepted identifier model aligned with `read_dir` listing.
  - Alternatives considered: canonicalize after join and check ancestors.
  - Why rejected: canonicalization requires the path to exist and still benefits
    from segment validation; a single-component invariant is simpler and
    sufficient for pending skill IDs.

## Open Questions

- [x] No open questions for the minimal security fix.
