# Review Report

## Scope Reviewed

- Files/modules: `crates/skilllite-sandbox/src/macos.rs`, `crates/skilllite-sandbox/src/lib.rs`, `crates/skilllite-sandbox/src/mod.rs`, `crates/skilllite-sandbox/src/security/policy.rs`, `docs/en/ARCHITECTURE.md`, `docs/zh/ARCHITECTURE.md`.
- Commits/changes: Seatbelt profile order only.

## Findings

- Critical: None remaining in the profile order. The previous trailing `(allow default)` was the bug.
- Major: None.
- Minor: A later `(allow file-read* (subpath skill_dir))` can still override an earlier sensitive-read deny for a path inside the skill directory (for example a `.env` shipped in that skill). Home paths such as `~/.ssh` are not under that allow, so those denies stick.

## Quality Gates

- Architecture boundary checks: `pass`
- Security invariants: `pass` (L1/L2/L3 meanings unchanged; L2/L3 macOS isolation is stricter)
- Required tests executed: `pass`
- Docs sync (EN/ZH): `pass`

## Test Evidence

- Commands run:
  - `cargo test -p skilllite-sandbox --lib macos::`
  - `cargo test -p skilllite-sandbox`
  - `SKILLLITE_MAX_PROCESSES=200 cargo test --workspace --exclude skilllite-assistant`
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
- Key outputs:
  - macOS profile tests: 4 passed, including `production_seatbelt_profile_does_not_let_allow_default_reopen_denies` and `production_seatbelt_proxy_denies_network_before_localhost_allow`.
  - `skilllite-sandbox` lib: 85 passed.
  - `skilllite` `e2e_minimal`: 2 passed.
  - Workspace test suites reported `ok` with 0 failed.
  - `cargo clippy -p skilllite-sandbox --all-targets -- -D warnings`: finished, exit 0.
  - Workspace `cargo clippy --all-targets -- -D warnings` still fails on pre-existing `clippy::double_must_use` in `async_trait` traits (`skilllite-core` `LlmProvider`, `skilllite-evolution` `EvolutionLlm`) under rustc 1.99. Not introduced by this change.

## Decision

- Merge readiness: ready
- Follow-up actions: macOS CI smoke still only checks that a tiny Python skill runs. It does not assert a denied write or connect. The new unit tests lock rule order; a future macOS job could attempt an out-of-workspace write.
