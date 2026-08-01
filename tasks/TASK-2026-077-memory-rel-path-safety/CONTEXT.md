# Technical Context

## Current State

- Relevant crates/files:
  - `crates/skilllite-executor/src/rpc.rs` (`handle_memory_write`)
  - `crates/skilllite-executor/src/memory.rs` (`reindex_memory_markdown_files`)
  - `crates/skilllite-core/src/path_validation.rs`
  - `crates/skilllite-core/src/error.rs`
- Current behavior: `root.join("memory").join(rel_path)` after
  `rel_path.contains("..") || rel_path.starts_with('/')` only.
- Agent memory tool path uses lexical normalize + `starts_with(memory_dir)` and
  already rejects Windows absolute joins; executor RPC does not.

## Architecture Fit

- Layer boundaries: shared validation in `skilllite-core`, consumption in executor.
- Interfaces to preserve: RPC method names and happy-path payload for valid relative paths.

## Dependency and Compatibility

- New dependencies: none
- Backward compatibility: only invalid/escaping `rel_path` values start failing

## Design Decisions

- Decision: allow nested `/` relatives (memory notes are hierarchical) while rejecting
  Windows separators/drive prefixes at the string layer so Linux CI can cover them.
- Decision: add post-join lexical containment as defense in depth, matching agent memory writes.
