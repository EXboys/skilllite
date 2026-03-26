//! `.skilllite/schedule.json` — MVP timed inject for agent chat turns.
//!
//! Non–dry-run execution also requires env `SKILLLITE_SCHEDULE_ENABLED=1` (see `skilllite-commands::schedule::cmd_tick`).
//!
//! See `todo/architecture-companion-schedule-channel.md`.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use chrono::{Local, NaiveDateTime, NaiveTime, TimeZone};
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
    /// High-level objective (optional). Merged into the injected message when non-empty.
    #[serde(default)]
    pub goal: String,
    /// Step-by-step instructions for the agent (optional). Merged when non-empty.
    #[serde(default)]
    pub steps_prompt: String,
    /// Extra context appended after structured blocks when `goal` / `steps_prompt` are used;
    /// otherwise this is the full injected message (backward compatible).
    pub message: String,
    /// Persistent session key; default `schedule-<id>`.
    #[serde(default)]
    pub session_key: Option<String>,
    /// Local wall time `HH` or `HH:MM` (24h). When set (and `once_at` unset), run at most once per
    /// calendar day after this time. Uses the system local timezone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub daily_at: Option<String>,
    /// Additional (or sole) daily wall times; merged with `daily_at`, deduped and sorted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub daily_times: Vec<String>,
    /// One-shot local datetime `YYYY-MM-DDTHH:MM` (no `Z`; interpreted in system local time).
    /// When set, takes precedence over `daily_at` and `interval_seconds` for due-ness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub once_at: Option<String>,
}

impl ScheduleJob {
    /// Text passed to `run_chat` for this job.
    pub fn injected_message(&self) -> String {
        let g = self.goal.trim();
        let s = self.steps_prompt.trim();
        let m = self.message.trim();
        if g.is_empty() && s.is_empty() {
            return self.message.clone();
        }
        let mut blocks: Vec<String> = Vec::new();
        if !g.is_empty() {
            blocks.push(format!("【目标】\n{}", g));
        }
        if !s.is_empty() {
            blocks.push(format!("【执行步骤】\n{}", s));
        }
        if !m.is_empty() {
            blocks.push(m.to_string());
        }
        blocks.join("\n\n")
    }
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

pub fn save_schedule(workspace: &Path, schedule: &ScheduleFile) -> Result<(), String> {
    let dir = workspace.join(".skilllite");
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {}", dir.display(), e))?;
    let p = schedule_path(workspace);
    let s = serde_json::to_string_pretty(schedule).map_err(|e| e.to_string())?;
    std::fs::write(&p, s).map_err(|e| format!("write {}: {}", p.display(), e))
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

fn opt_nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

/// Parse `HH:MM` or `H:MM` (24h). Returns `(hour, minute)`.
pub fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let t = s.trim();
    let (h, m) = t.split_once(':')?;
    let hour: u32 = h.trim().parse().ok()?;
    let minute: u32 = m.trim().parse().ok()?;
    if hour >= 24 || minute >= 60 {
        return None;
    }
    Some((hour, minute))
}

fn naive_local_to_unix(naive: chrono::NaiveDateTime) -> Option<i64> {
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => Some(dt.timestamp()),
        chrono::LocalResult::Ambiguous(earliest, _) => Some(earliest.timestamp()),
        chrono::LocalResult::None => None,
    }
}

/// Start of today's `HH:MM` slot in local time, as Unix seconds.
fn today_at_hhmm_unix(hour: u32, minute: u32, now: i64) -> Option<i64> {
    let now_local = Local.timestamp_opt(now, 0).latest()?;
    let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
    let naive = now_local.date_naive().and_time(time);
    naive_local_to_unix(naive)
}

/// Parse `YYYY-MM-DDTHH:MM` or `YYYY-MM-DD HH:MM` as local naive datetime.
pub fn parse_once_at_local(s: &str) -> Option<NaiveDateTime> {
    let t = s.trim();
    NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M")
        .ok()
        .or_else(|| NaiveDateTime::parse_from_str(t, "%Y-%m-%d %H:%M").ok())
}

fn once_at_unix(s: &str) -> Option<i64> {
    let naive = parse_once_at_local(s)?;
    naive_local_to_unix(naive)
}

/// All distinct `(hour, minute)` wall-clock slots for this job (local time), sorted.
pub fn daily_hhmm_pairs(job: &ScheduleJob) -> Vec<(u32, u32)> {
    let mut set: BTreeSet<(u32, u32)> = BTreeSet::new();
    for s in &job.daily_times {
        if let Some(p) = parse_hhmm(s) {
            set.insert(p);
        }
    }
    if let Some(d) = opt_nonempty(&job.daily_at) {
        if let Some(p) = parse_hhmm(d) {
            set.insert(p);
        }
    }
    set.into_iter().collect()
}

/// Wall-clock jobs (`once_at` or any daily time). Used to bypass global `min_interval_seconds_between_runs`
/// so failed once/daily runs can retry on the next tick without waiting for that gap.
pub fn job_uses_wall_clock_schedule(job: &ScheduleJob) -> bool {
    opt_nonempty(&job.once_at).is_some()
        || !job.daily_times.is_empty()
        || opt_nonempty(&job.daily_at).is_some()
}

/// Whether a job should run: `now` and `last` are Unix seconds; `last` is last successful run.
/// Failed runs do not advance `last`, so once/daily jobs remain due until a successful completion.
pub fn job_is_due(job: &ScheduleJob, last_run_unix: i64, now: i64) -> bool {
    if let Some(once_s) = opt_nonempty(&job.once_at) {
        let Some(deadline) = once_at_unix(once_s) else {
            return false;
        };
        return now >= deadline && last_run_unix < deadline;
    }
    let daily_slots = daily_hhmm_pairs(job);
    if !daily_slots.is_empty() {
        for (h, m) in daily_slots {
            let Some(today_fire) = today_at_hhmm_unix(h, m, now) else {
                continue;
            };
            if now >= today_fire && last_run_unix < today_fire {
                return true;
            }
        }
        return false;
    }
    last_run_unix == 0 || (now - last_run_unix) >= job.interval_seconds as i64
}

/// Indices into `schedule.jobs` that are due at `now` (Unix epoch seconds).
/// Wall-clock schedules (`daily_at`, `once_at`) use the system local timezone.
pub fn list_due_job_indices(
    schedule: &ScheduleFile,
    state: &ScheduleState,
    now: i64,
) -> Vec<usize> {
    if !schedule.enabled {
        return vec![];
    }
    let global_min_blocks = schedule.limits.min_interval_seconds_between_runs > 0
        && state.last_any_run_unix > 0
        && (now - state.last_any_run_unix)
            < schedule.limits.min_interval_seconds_between_runs as i64;

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
        if !job_is_due(job, last, now) {
            continue;
        }
        if global_min_blocks && !job_uses_wall_clock_schedule(job) {
            continue;
        }
        out.push(i);
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
                goal: String::new(),
                steps_prompt: String::new(),
                message: "hi".into(),
                session_key: None,
                daily_at: None,
                daily_times: vec![],
                once_at: None,
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
                goal: String::new(),
                steps_prompt: String::new(),
                message: "hi".into(),
                session_key: None,
                daily_at: None,
                daily_times: vec![],
                once_at: None,
            }],
        };
        let mut state = ScheduleState::default();
        let now = 1_000_000_i64;
        record_job_run(&mut state, "a", now);
        let due = list_due_job_indices(&schedule, &state, now + 60);
        assert!(due.is_empty());
    }

    #[test]
    fn injected_message_merges_goal_and_steps() {
        let job = ScheduleJob {
            id: "x".into(),
            enabled: true,
            interval_seconds: 60,
            goal: "Summarize inbox".into(),
            steps_prompt: "1. Read\n2. Reply".into(),
            message: "Use plain text.".into(),
            session_key: None,
            daily_at: None,
            daily_times: vec![],
            once_at: None,
        };
        let t = job.injected_message();
        assert!(t.contains("【目标】"));
        assert!(t.contains("Summarize inbox"));
        assert!(t.contains("【执行步骤】"));
        assert!(t.contains("Use plain text."));
    }

    #[test]
    fn injected_message_legacy_message_only() {
        let job = ScheduleJob {
            id: "x".into(),
            enabled: true,
            interval_seconds: 60,
            goal: String::new(),
            steps_prompt: String::new(),
            message: "legacy only".into(),
            session_key: None,
            daily_at: None,
            daily_times: vec![],
            once_at: None,
        };
        assert_eq!(job.injected_message(), "legacy only");
    }

    #[test]
    fn parse_hhmm_ok() {
        assert_eq!(parse_hhmm("9:30"), Some((9, 30)));
        assert_eq!(parse_hhmm("09:05"), Some((9, 5)));
        assert!(parse_hhmm("24:00").is_none());
    }

    #[test]
    fn job_once_at_due_then_not() {
        let deadline = super::once_at_unix("2020-06-01T08:00").expect("valid");
        let job = ScheduleJob {
            id: "once".into(),
            enabled: true,
            interval_seconds: 3600,
            goal: String::new(),
            steps_prompt: String::new(),
            message: "m".into(),
            session_key: None,
            daily_at: None,
            daily_times: vec![],
            once_at: Some("2020-06-01T08:00".into()),
        };
        assert!(!job_is_due(&job, 0, deadline - 1));
        assert!(job_is_due(&job, 0, deadline));
        assert!(!job_is_due(&job, deadline + 10, deadline + 20));
    }

    #[test]
    fn daily_hhmm_pairs_merges_and_sorts() {
        let job = ScheduleJob {
            id: "d".into(),
            enabled: true,
            interval_seconds: 60,
            goal: String::new(),
            steps_prompt: String::new(),
            message: "m".into(),
            session_key: None,
            daily_at: Some("10:00".into()),
            daily_times: vec!["15:30".into(), "9:00".into(), "15:30".into()],
            once_at: None,
        };
        assert_eq!(daily_hhmm_pairs(&job), vec![(9, 0), (10, 0), (15, 30)]);
    }

    #[test]
    fn once_bypasses_global_min_interval() {
        let schedule = ScheduleFile {
            version: 1,
            enabled: true,
            limits: ScheduleLimits {
                max_runs_per_day: 8,
                min_interval_seconds_between_runs: 600,
            },
            jobs: vec![ScheduleJob {
                id: "o".into(),
                enabled: true,
                interval_seconds: 3600,
                goal: String::new(),
                steps_prompt: String::new(),
                message: "m".into(),
                session_key: None,
                daily_at: None,
                daily_times: vec![],
                once_at: Some("2020-01-01T00:00".into()),
            }],
        };
        let state = ScheduleState {
            last_any_run_unix: 1_700_000_000,
            ..Default::default()
        };
        let now = state.last_any_run_unix + 30;
        let due = list_due_job_indices(&schedule, &state, now);
        assert_eq!(due, vec![0]);
    }
}
