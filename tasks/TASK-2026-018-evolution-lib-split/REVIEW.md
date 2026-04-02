# Review

## Summary

Refactor-only change: file moves and module boundaries. Public API preserved through `lib.rs` re-exports.

## Merge readiness

- Ready: Yes.

## Notes

- `scope.rs` remains large (~1160 lines); future work can extract coordinator vs `should_evolve` if desired.
- `EvolutionProfile` is re-exported at crate root (`pub use config::EvolutionProfile`) for parity with the pre-split `lib.rs`.

## Validation evidence

- `cargo test -p skilllite-evolution` — 64 passed, 0 failed.
- `cargo clippy -p skilllite-evolution --all-targets` — finished with no errors.
- `cargo check -p skilllite-agent -p skilllite-commands` — succeeded (pre-existing warnings in `skilllite-commands` only).
