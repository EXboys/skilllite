# Context

## Technical

- Implementation: `crates/skilllite-commands/src/migrate/openclaw.rs`; CLI in `skilllite/src/cli.rs` (`ClawAction::Migrate`, `MigrateSource::Openclaw`); dispatch `skilllite/src/dispatch/migrate.rs`.
- Skill import helpers re-exported from `skill::import_openclaw` for planning.
- Report directory: `<workspace>/.skilllite/migration/openclaw/<timestamp>/` with `report.json`, optional `pre-migration-backup.zip`, and `archive/`.

## Compatibility

- `import-openclaw-skills` remains available unchanged.
- OpenClaw home resolution prefers existing `~/.openclaw`, then `~/.clawdbot`, then `~/.moltbot`.
