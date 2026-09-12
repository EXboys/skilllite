# CONTEXT — Technical boundaries

## Injected Specs

- `spec/verification-integrity.md`
- `spec/task-artifact-language.md`
- `spec/architecture-boundaries.md`
- `spec/security-nonnegotiables.md`
- `spec/rust-conventions.md`
- `spec/testing-policy.md`
- `spec/docs-sync.md`

## Task Type

`commands` + `security` (path/root scoping)

## Touched Surfaces

- `crates/skilllite-commands/src/evolution.rs` — disable/explain workspace root
- `skilllite/src/cli.rs` + `skilllite/src/dispatch/mod.rs` — `--workspace` wiring
- `crates/skilllite-commands/src/migrate/openclaw.rs` — memory_root + reindex
- `crates/skilllite-fs/src/read_write.rs` — atomic staging
- `skilllite/tests/cli_evolution_workspace.rs` — disable isolation regression
- EN/ZH command docs for evolution disable/explain

## Compatibility

- Default `--workspace .` matches other evolution subcommands.
- Migrate memory destination changes from global chat to project chat; this matches workspace-scoped agent/chat layout.
