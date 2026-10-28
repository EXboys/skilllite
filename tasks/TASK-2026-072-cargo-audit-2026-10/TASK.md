# TASK Card

## Metadata

- Task ID: `TASK-2026-072`
- Title: Clear cargo-audit failures on main
- Status: `done`
- Priority: `P1`
- Owner: `agent`
- Contributors:
- Created: `2026-10-28`
- Target milestone:

## Problem

`cargo audit` on main reports 16 vulnerabilities, so CI exits 1. The reported crates are `quinn-proto` 0.11.15, `rustls` 0.23.45, `rustls-webpki` 0.103.15, `webpki-roots` 1.0.6, `zip` 0.6.6, `zstd-safe` 5.0.2, and `zstd-sys` 2.0.16.

## Scope

- In scope:
  - Bump those crates to the versions named in the RustSec advisories.
  - Adjust the zip writer/reader call sites that do not compile against zip 9.
- Out of scope:
  - Yanked-crate warnings (`clipboard-win`, `spin`). CI already allows warnings.
  - A reqwest 0.13 upgrade.

## Acceptance Criteria

- [x] `cargo audit` exits 0.
- [x] `cargo deny check bans` passes.
- [x] Zip path-traversal rejection and `e2e_minimal` still pass.

## Risks

- Risk: zip 9 no longer enables every zip 0.6 default codec.
  - Impact: encrypted, lzma, and xz entries are not read.
  - Mitigation: keep deflate, bzip2, and zstd, which cover the archives this repo writes and the usual skill bundle.

## Validation Plan

- Required tests: `cargo test -p skilllite-commands --lib skill::add::source::tests`, `cargo test -p skilllite --test e2e_minimal`, `cargo clippy -p skilllite-commands -p skilllite --all-targets -- -D warnings`
- Commands to run: recorded in STATUS.md
- Manual checks: none

## Regression Scope

- Areas likely affected: skill zip import and Claw migrate backup zips
- Explicit non-goals: HTTP client major upgrades

## Links

- Source TODO section:
- Related PRs/issues:
- Related docs:
