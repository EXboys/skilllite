# PRD

## Background

CI runs `cargo audit` with no ignore list. Sixteen advisories fail the build.

## Objective

`cargo audit` exits 0 on the workspace lockfile.

## Functional Requirements

- FR-1: Patched versions are `quinn-proto` >= 0.11.18, `rustls` >= 0.23.46, `rustls-webpki` >= 0.103.16, `webpki-roots` >= 1.0.10, `zip` >= 9.0.0, `zstd-safe` >= 8.2.0, `zstd-sys` >= 2.1.2.
- FR-2: Skill zip import still rejects entries that escape the extraction root.

## Non-Functional Requirements

- Security: do not ignore the advisories.
- Performance: no change intended.
- Compatibility: stay on reqwest 0.12.

## Constraints

- Technical: zip 9.1 requires Rust 1.88. CI uses stable, and this machine has 1.94.1.
- Timeline: unblock main CI.

## Success Metrics

- Metric: `cargo audit` vulnerability count.
- Baseline: 16.
- Target: 0.

## Rollout

- Rollout plan: lockfile and zip call-site update on main.
- Rollback plan: revert the lockfile and the zip dependency pin.
