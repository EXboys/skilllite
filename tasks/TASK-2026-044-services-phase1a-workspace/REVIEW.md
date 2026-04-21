# Review Report

## Scope Reviewed

- Files/modules:
  - `crates/skilllite-services/Cargo.toml` (modified — real deps added)
  - `crates/skilllite-services/src/error.rs` (new)
  - `crates/skilllite-services/src/workspace.rs` (new — service + 6 unit tests)
  - `crates/skilllite-services/src/lib.rs` (modified — exports + rustdoc; removed `BOOTSTRAP_PHASE`)
  - `crates/skilllite-commands/Cargo.toml` (modified — added `skilllite-services` dep)
  - `crates/skilllite-commands/src/skill/common.rs` (modified — service migration)
  - `crates/skilllite-commands/src/init.rs` (modified — service migration + drive-by clippy fix)
  - `crates/skilllite-commands/src/ide.rs` (modified — service migration; helper signature updated to `ResolveSkillsDirResponse`)
  - `crates/skilllite-assistant/src-tauri/Cargo.toml` (modified — added `skilllite-services` dep)
  - `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/integrations/shared.rs` (modified — service migration; preserves silent-drop)
  - `tasks/board.md` (modified — TASK status promoted)
- Commits/changes:
  - To be filled when PR is opened.

## Findings

- Critical: None.
- Major: None.
- Minor:
  - `cargo deny check bans` still emits one `unused-wrapper` per graph for the `skilllite-services` rule (root: `skilllite-assistant` is not in the root workspace; Desktop manifest: `skilllite-commands` is not in the Desktop graph). Reduced from 3 unmatched wrappers per graph after Phase 1A bootstrap to 1 each after this migration. Will reduce further when MCP becomes a real consumer.
  - Pre-existing `cargo clippy` warnings unrelated to this TASK (`skip_dep_audit` unused param in `skill/add/admission.rs`, `CLAWHUB_DOWNLOAD_URL` dead constant in `skill/add/source.rs`) remain. They are non-blocking under CI's `cargo clippy --all-targets` (no `-D warnings`); `-D warnings` only fails when run inside their respective files. Out of scope to fix here.

## Quality Gates

- Architecture boundary checks: `pass` (both `cargo deny check bans` invocations exit 0 with the new `skilllite-services` rule actually exercised by real consumers)
- Security invariants: `pass` (no security-relevant change; `forbid(unsafe_code)` retained on `skilllite-services`)
- Required tests executed: `pass` (`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, Desktop equivalents, `cargo fmt --check` workspace + Desktop, both `cargo deny check bans`, Desktop `cargo build`, `python3 scripts/validate_tasks.py`)
- Docs sync (EN/ZH): `pass` (no user-visible behaviour change; entry/architecture docs already updated in TASK-2026-042 to mention the future `skilllite-services` layer; rustdoc on `skilllite-services` is the implementation reference)

## Test Evidence

- Commands run:
  - `cargo test -p skilllite-services`
  - `cargo test --workspace`
  - `cargo clippy -p skilllite-services --all-targets -- -D warnings`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings`
  - `cargo fmt --check`
  - `cargo fmt --check --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `cargo deny check bans`
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans`
  - `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml`
  - `python3 scripts/validate_tasks.py`
- Key outputs:
  - `cargo test -p skilllite-services` → `running 6 tests ... test result: ok. 6 passed; 0 failed; 0 ignored`.
  - `cargo test --workspace` → all suites green; selected counts: `skilllite_agent` 234 passed, `skilllite_executor` 78 passed, `skilllite_evolution` 86 passed, `skilllite_commands` 13 passed, `skilllite_services` 6 passed, `skilllite_assistant` 23 passed, `skilllite_sandbox` 94 passed. No failures across any crate.
  - `cargo clippy --workspace --all-targets -- -D warnings` → succeeds with no warnings; final line `Finished \`dev\` profile`.
  - `cargo clippy --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml --all-targets -- -D warnings` → succeeds with no warnings.
  - `cargo fmt --check` (workspace and Desktop manifest) → empty output.
  - `cargo deny check bans` → ends with `bans ok` (1 `unused-wrapper` per graph for `skilllite-services` rule; see Findings/Minor).
  - `cargo deny --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml check bans` → ends with `bans ok` (same minor warnings).
  - `cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml` → `Finished \`dev\` profile`.
  - `python3 scripts/validate_tasks.py` → `Task validation passed (44 task directories checked).`

## Decision

- Merge readiness: superseded by `tasks/TASK-2026-045-services-rollback-phase1a/`
- Superseded note (2026-04-20):
  - This TASK's `WorkspaceService` extraction was rolled back the same day. Post-implementation comparison showed the four migrated callsites went from ~5 lines each (direct call to `skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback`) to ~10–15 lines each because the new service returned `Result<...>` over an infallible underlying operation, forcing every caller to add fallback boilerplate. Net LOC increased rather than decreased.
  - A subsequent grep-driven review of Phase 1B (RuntimeService) and Phase 2 (EvolutionService) candidates found that the cross-entry duplication these phases were meant to consolidate was smaller than initially estimated, weakening the case for retaining a single `skilllite-services` crate as a foundation.
  - Both this TASK and TASK-2026-043 (bootstrap) are preserved (not deleted) to keep the audit trail.
- Follow-up actions:
  - None. The Phase 0 boundary work (TASK-2026-042) — Desktop manifest deny coverage, EN+ZH doc updates — is unaffected and remains in force.
