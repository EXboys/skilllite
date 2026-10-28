# Technical Context

## Current State

- Relevant crates/files: `Cargo.lock`, `skilllite/Cargo.toml`, `crates/skilllite-commands/Cargo.toml`, zip call sites in `skill/add/source.rs`, `migrate/openclaw.rs`, and `skilllite/tests/e2e_minimal.rs`.
- Current behavior: `cargo audit` fails on the versions listed in TASK.md.

## Architecture Fit

- Layer boundaries involved: none. This is a dependency bump inside existing crates.
- Interfaces to preserve: zip extraction still rejects path traversal before writing a file.

## Dependency and Compatibility

- New dependencies: zip 9 pulls `zlib-rs`, `zopfli`, and `typed-path`. `bzip2` is now `libbz2-rs-sys` instead of `bzip2-sys`.
- Backward compatibility notes: `file.name()` in zip 9 returns `Result`. Encrypted, lzma, and xz entries are no longer decoded.

## Design Decisions

- Decision: bump zip to 9.1 with `deflate`, `bzip2`, and `zstd`, and leave reqwest on 0.12.
  - Rationale: the zip advisories are patched only in 9.x. reqwest 0.12.28 already accepts quinn 0.11.12 and rustls 0.23.46.
  - Alternatives considered: ignore the advisories, or upgrade reqwest to 0.13.
  - Why rejected: ignores would leave CI policy weaker, and reqwest 0.13 is a larger API move than this audit failure needs.

## Open Questions

- [x] Yanked `clipboard-win` and `spin` stay as allowed warnings.
