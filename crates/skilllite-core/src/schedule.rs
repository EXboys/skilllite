//! `.skilllite/schedule.json` — MVP timed inject for agent chat turns.
//!
//! Non–dry-run execution also requires env `SKILLLITE_SCHEDULE_ENABLED=1` (see `skilllite-commands::schedule::cmd_tick`).
//!
//! See `todo/architecture-companion-schedule-channel.md`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};

pub const SCHEDULE_FILE: &str = "schedule.json";
pub const SCHEDULE_STATE_FILE: &str = "schedule-state.json";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduleFile {
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub limits: ScheduleLimits,
    #[serde(default)]
    pub jobs: Vec<ScheduleJob>,
}

fn default_true() -> bool {
    true
}

impl Default for ScheduleFile {
    fn default() -> Self {
        Self {
            version: 1,
            enabled: true,
            limits: ScheduleLimits::default(),
            jobs: vec![],
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduleLimits {
    #[serde(default = "default_max_runs")]
    pub max_runs_per_day: u32,
    #[serde(default)]
    pub min_interval_seconds_between_runs: u64,
}

fn default_max_runs() -> u32 {
    8
}

impl Default for ScheduleLimits {
    fn default() -> Self {
        Self {
            max_runs_per_day: default_max_runs(),
            min_interval_seconds_between_runs: 0,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScheduleJob {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Minimum seconds between successful runs of this job.
    pub interval_seconds: u64,
    /// Injected as the user message for one `chat` turn (full agent loop).
    pub message: String,
    /// Persistent session key; default `schedule-<id>`.
    #[serde(default)]
    pub session_key: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct ScheduleState {
    #[serde(default)]
    pub runs_day_date: Option<String>,
    #[serde(default)]
    pub runs_day_count: u32,
    #[serde(default)]
    pub last_any_run_unix: i64,
    #[serde(default)]
    pub jobs: HashMap<String, JobState>,
}

#[derive(Debug, Deserialize, Serialize, Default, Clone)]
pub struct JobState {
    pub last_run_unix: i64,
}

pub fn schedule_path(workspace: &Path) -> PathBuf {
    workspace.join(".skilllite").join(SCHEDULE_FILE)
}

pub fn schedule_state_path(workspace: &Path) -> PathBuf {
    workspace.join(".skilllite").join(SCHEDULE_STATE_FILE)
}

pub fn load_schedule(workspace: &Path) -> Result<Option<ScheduleFile>, String> {
    let p = schedule_path(workspace);
    if !p.exists() {
        return Ok(None);
    }
    let s = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {}", p.display(), e))?;
    serde_json::from_str(&s).map_err(|e| format!("schedule.json: {}", e))
}

pub fn load_state(workspace: &Path) -> ScheduleState {
    let p = schedule_state_path(workspace);
    if !p.exists() {
        return ScheduleState::default();
    }
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_state(workspace: &Path, state: &ScheduleState) -> Result<(), String> {
    let dir = workspace.join(".skilllite");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let p = schedule_state_path(workspace);
    let s = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| e.to_string())
}

/// Reset daily counter when the calendar day changes.
pub fn prepare_state_for_today(state: &mut ScheduleState) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    if state.runs_day_date.as_deref() != Some(&today) {
        state.runs_day_date = Some(today);
        state.runs_day_count = 0;
    }
}

/// Indices into `schedule.jobs` that are due at `now` (UTC unix seconds).
pub fn list_due_job_indices(
    schedule: &ScheduleFile,
    state: &ScheduleState,
    now: i64,
) -> Vec<usize> {
    if !schedule.enabled {
        return vec![];
    }
    if schedule.limits.min_interval_seconds_between_runs > 0
        && state.last_any_run_unix > 0
        && (now - state.last_any_run_unix)
            < schedule.limits.min_interval_seconds_between_runs as i64
    {
        return vec![];
    }
    let mut out = Vec::new();
    for (i, job) in schedule.jobs.iter().enumerate() {
        if !job.enabled {
            continue;
        }
        let last = state
            .jobs
            .get(&job.id)
            .map(|j| j.last_run_unix)
            .unwrap_or(0);
        if last == 0 || (now - last) >= job.interval_seconds as i64 {
            out.push(i);
        }
    }
    out
}

pub fn record_job_run(state: &mut ScheduleState, job_id: &str, now: i64) {
    state.last_any_run_unix = now;
    state.runs_day_count = state.runs_day_count.saturating_add(1);
    state
        .jobs
        .entry(job_id.to_string())
        .or_default()
        .last_run_unix = now;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_due_respects_interval() {
        let schedule = ScheduleFile {
            version: 1,
            enabled: true,
            limits: ScheduleLimits::default(),
            jobs: vec![ScheduleJob {
                id: "a".into(),
                enabled: true,
                interval_seconds: 3600,
                message: "hi".into(),
                session_key: None,
            }],
        };
        let mut state = ScheduleState::default();
        let now = 10_000_i64;
        let due = list_due_job_indices(&schedule, &state, now);
        assert_eq!(due, vec![0]);
        record_job_run(&mut state, "a", now);
        let due2 = list_due_job_indices(&schedule, &state, now + 100);
        assert!(due2.is_empty());
        let due3 = list_due_job_indices(&schedule, &state, now + 3600);
        assert_eq!(due3, vec![0]);
    }

    #[test]
    fn min_interval_global_blocks() {
        let schedule = ScheduleFile {
            version: 1,
            enabled: true,
            limits: ScheduleLimits {
                max_runs_per_day: 8,
                min_interval_seconds_between_runs: 600,
            },
            jobs: vec![ScheduleJob {
                id: "a".into(),
                enabled: true,
                interval_seconds: 1,
                message: "hi".into(),
                session_key: None,
            }],
        };
        let mut state = ScheduleState::default();
        let now = 1_000_000_i64;
        record_job_run(&mut state, "a", now);
        let due = list_due_job_indices(&schedule, &state, now + 60);
        assert!(due.is_empty());
    }
}
