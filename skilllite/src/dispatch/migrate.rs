//! OpenClaw migration command dispatch.

use crate::cli::{ClawAction, Commands, MigrateSource};
use crate::command_registry::CommandRegistry;

fn run_openclaw_migrate(
    workspace: &str,
    openclaw_dir: Option<&str>,
    skills_dir: &str,
    skill_conflict: &str,
    dry_run: bool,
    force: bool,
    scan_offline: bool,
    migrate_secrets: bool,
    no_backup: bool,
    yes: bool,
    overwrite: bool,
) -> crate::Result<()> {
    skilllite_commands::migrate::cmd_claw_migrate_openclaw(
        workspace,
        openclaw_dir,
        skills_dir,
        skill_conflict,
        dry_run,
        force,
        scan_offline,
        migrate_secrets,
        no_backup,
        yes,
        overwrite,
    )
    .map_err(Into::into)
}

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Claw { action } = cmd {
            match action {
                ClawAction::Migrate {
                    workspace,
                    openclaw_dir,
                    skills_dir,
                    skill_conflict,
                    dry_run,
                    force,
                    scan_offline,
                    migrate_secrets,
                    no_backup,
                    yes,
                    overwrite,
                } => Some(run_openclaw_migrate(
                    workspace,
                    openclaw_dir.as_deref(),
                    skills_dir,
                    skill_conflict,
                    *dry_run,
                    *force,
                    *scan_offline,
                    *migrate_secrets,
                    *no_backup,
                    *yes,
                    *overwrite,
                )),
            }
        } else {
            None
        }
    });

    reg.register(|cmd| {
        if let Commands::Migrate { source } = cmd {
            match source {
                MigrateSource::Openclaw {
                    workspace,
                    openclaw_dir,
                    skills_dir,
                    skill_conflict,
                    dry_run,
                    force,
                    scan_offline,
                    migrate_secrets,
                    no_backup,
                    yes,
                    overwrite,
                } => Some(run_openclaw_migrate(
                    workspace,
                    openclaw_dir.as_deref(),
                    skills_dir,
                    skill_conflict,
                    *dry_run,
                    *force,
                    *scan_offline,
                    *migrate_secrets,
                    *no_backup,
                    *yes,
                    *overwrite,
                )),
            }
        } else {
            None
        }
    });
}
