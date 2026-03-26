//! `skilllite schedule tick` — run due jobs from `.skilllite/schedule.json`.

use std::path::Path;

use anyhow::{Context, Result};

/// Process due scheduled jobs: each runs one full agent `chat` turn with `job.message`.
pub fn cmd_tick(workspace: Option<&str>, dry_run: bool) -> Result<()> {
    let ws = workspace.unwrap_or(".");
    let workspace_path = Path::new(ws)
        .canonicalize()
        .with_context(|| format!("workspace path: {}", ws))?;

    skilllite_core::config::load_dotenv_from_dir(&workspace_path);

    let schedule = match skilllite_core::schedule::load_schedule(&workspace_path)
        .map_err(|e| anyhow::anyhow!(e))?
    {
        None => {
            eprintln!(
                "No schedule file. Create {}",
                skilllite_core::schedule::schedule_path(&workspace_path).display()
            );
            return Ok(());
        }
        Some(s) => s,
    };

    if schedule.version != 1 {
        anyhow::bail!(
            "Unsupported schedule.json version {} (only 1 supported)",
            schedule.version
        );
    }

    let mut state = skilllite_core::schedule::load_state(&workspace_path);
    skilllite_core::schedule::prepare_state_for_today(&mut state);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let due_indices = skilllite_core::schedule::list_due_job_indices(&schedule, &state, now);
    if due_indices.is_empty() {
        tracing::debug!("schedule tick: no due jobs");
        return Ok(());
    }

    if dry_run {
        for idx in due_indices {
            if state.runs_day_count >= schedule.limits.max_runs_per_day {
                eprintln!(
                    "schedule: skipped further jobs (would exceed max_runs_per_day={})",
                    schedule.limits.max_runs_per_day
                );
                break;
            }
            let job = &schedule.jobs[idx];
            let session = job
                .session_key
                .clone()
                .unwrap_or_else(|| format!("schedule-{}", job.id));
            eprintln!(
                "schedule: job `{}` → session `{}` (dry-run)",
                job.id, session
            );
        }
        return Ok(());
    }

    if !skilllite_core::config::env_bool(
        skilllite_core::config::env_keys::schedule::SKILLLITE_SCHEDULE_ENABLED,
        &[],
        false,
    ) {
        eprintln!(
            "schedule tick: skipped (no LLM run). Set SKILLLITE_SCHEDULE_ENABLED=1 to execute due jobs, or use --dry-run to preview."
        );
        return Ok(());
    }

    let mut config = skilllite_agent::types::AgentConfig::from_env();
    config.workspace = workspace_path.to_string_lossy().to_string();
    if config.api_key.is_empty() {
        anyhow::bail!("API key required for schedule tick. Set OPENAI_API_KEY.");
    }

    skilllite_core::config::ensure_default_output_dir();

    for idx in due_indices {
        if state.runs_day_count >= schedule.limits.max_runs_per_day {
            tracing::warn!(
                "schedule tick: max_runs_per_day ({}) reached",
                schedule.limits.max_runs_per_day
            );
            break;
        }
        let job = &schedule.jobs[idx];
        let session = job
            .session_key
            .clone()
            .unwrap_or_else(|| format!("schedule-{}", job.id));
        eprintln!("schedule: job `{}` → session `{}`", job.id, session);
        skilllite_agent::chat::run_chat(config.clone(), session, Some(job.message.clone()))
            .with_context(|| format!("schedule job `{}`", job.id))?;
        skilllite_core::schedule::record_job_run(&mut state, &job.id, now);
        skilllite_core::schedule::save_state(&workspace_path, &state)
            .map_err(|e| anyhow::anyhow!(e))?;
    }

    Ok(())
}
