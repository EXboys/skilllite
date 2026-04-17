//! 进化反馈库、生长调度、待审核技能与 `run_evolution` / 后台 `evolution run` 触发（桌面 UI 数据源）。

use serde::Serialize;
use std::collections::HashMap;

use crate::skilllite_bridge::chat::{merge_dotenv_with_chat_overrides, ChatConfigOverrides};
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};

use super::shared::{existing_workspace_skills_root, resolve_workspace_skills_root};

fn workspace_env_lookup(workspace: &str, key: &str) -> Option<String> {
    load_dotenv_for_child(workspace)
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .or_else(|| std::env::var(key).ok())
}

fn is_evolution_runtime_env_key(k: &str) -> bool {
    use skilllite_core::config::env_keys::evolution as evo;
    k == evo::SKILLLITE_EVOLUTION
        || k == evo::SKILLLITE_MAX_EVOLUTIONS_PER_DAY
        || k.starts_with("SKILLLITE_EVO")
        || k.starts_with("SKILLLITE_EVOLUTION_")
}

/// 将合并后的子进程环境变量中、与进化运行时相关的键同步到当前进程，供 `run_evolution` 内 `from_env()` 读取。
struct EvolutionRunEnvGuard {
    prev: Vec<(String, Option<String>)>,
}

impl EvolutionRunEnvGuard {
    fn push_from_merged(m: &std::collections::HashMap<String, String>) -> Self {
        let mut prev = Vec::new();
        for (k, v) in m.iter() {
            if !is_evolution_runtime_env_key(k) {
                continue;
            }
            prev.push((k.clone(), std::env::var(k).ok()));
            std::env::set_var(k, v);
        }
        Self { prev }
    }
}

impl Drop for EvolutionRunEnvGuard {
    fn drop(&mut self) {
        for (k, ov) in self.prev.drain(..) {
            match ov {
                Some(val) => std::env::set_var(&k, val),
                None => std::env::remove_var(&k),
            }
        }
    }
}

fn effective_evolution_interval_secs(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> u64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(n) = c.evolution_interval_secs.filter(|&n| n > 0) {
            return n;
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

fn effective_evolution_decision_threshold(
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
) -> i64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(n) = c.evolution_decision_threshold.filter(|&n| n > 0) {
            return i64::from(n);
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

fn effective_evo_profile_key(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> &'static str {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(ref p) = c.evo_profile {
            let t = p.trim();
            if t == "demo" {
                return "demo";
            }
            if t == "conservative" {
                return "conservative";
            }
        }
    }
    match workspace_env_lookup(workspace, evo_env::SKILLLITE_EVO_PROFILE).as_deref() {
        Some("demo") => "demo",
        Some("conservative") => "conservative",
        _ => "default",
    }
}

fn effective_evo_cooldown_hours(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> f64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(h) = c.evo_cooldown_hours.filter(|h| h.is_finite() && *h >= 0.0) {
            return h;
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVO_COOLDOWN_HOURS)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| skilllite_evolution::EvolutionThresholds::default().cooldown_hours)
}

/// Merged A9 growth config: workspace `.env` + optional UI overrides for interval / raw threshold.
fn growth_schedule_merged_for_workspace(
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
) -> skilllite_evolution::growth_schedule::GrowthScheduleConfig {
    let mut c = skilllite_evolution::growth_schedule::GrowthScheduleConfig::from_env();
    c.interval_secs = effective_evolution_interval_secs(workspace, cfg);
    c.raw_unprocessed_threshold = effective_evolution_decision_threshold(workspace, cfg);
    c
}

fn evolution_mode_from_workspace(workspace: &str) -> skilllite_evolution::EvolutionMode {
    use skilllite_core::config::env_keys::evolution as evo_env;
    match workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION).as_deref() {
        None | Some("1") | Some("true") | Some("") => skilllite_evolution::EvolutionMode::All,
        Some("0") | Some("false") => skilllite_evolution::EvolutionMode::Disabled,
        Some("prompts") => skilllite_evolution::EvolutionMode::PromptsOnly,
        Some("memory") => skilllite_evolution::EvolutionMode::MemoryOnly,
        Some("skills") => skilllite_evolution::EvolutionMode::SkillsOnly,
        _ => skilllite_evolution::EvolutionMode::All,
    }
}

/// Desktop **Life Pulse** only: whether to spawn `skilllite evolution run`.
///
/// A9: periodic interval **or** weighted recent signals **or** raw unprocessed count **or** long-interval sweep.
/// When **only** the periodic arm is due and no proposals would be built, returns `false` (no subprocess,
/// no `evolution_run_outcome` spam). Signal/sweep arms still spawn even if proposals end up empty.
/// Merged workspace `.env` + UI overrides are applied for the preflight check so thresholds match the child.
///
/// `last_periodic_spawn_unix`: last time the **periodic** arm fired; updated when the periodic
/// condition is met. Initialized lazily on first check so the first periodic window starts then.
pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &std::sync::Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
) -> bool {
    let mode = evolution_mode_from_workspace(workspace);
    if mode.is_disabled() {
        return false;
    }
    let schedule_cfg = growth_schedule_merged_for_workspace(workspace, cfg);
    let chat_root = skilllite_core::paths::chat_root();
    let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut g = last_periodic_spawn_unix
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let outcome =
        skilllite_evolution::growth_schedule::growth_due(&conn, now, &mut g, &schedule_cfg)
            .unwrap_or_default();

    if !outcome.due {
        return false;
    }
    if outcome.periodic_only {
        let dotenv = load_dotenv_for_child(workspace);
        let merged_vec = merge_dotenv_with_chat_overrides(dotenv, cfg);
        let merged_map: HashMap<String, String> = merged_vec.into_iter().collect();
        let _guard = EvolutionRunEnvGuard::push_from_merged(&merged_map);
        return skilllite_evolution::would_have_evolution_proposals(
            &conn,
            skilllite_evolution::EvolutionMode::from_env(),
            false,
        )
        .unwrap_or(true);
    }
    true
}

fn evolution_mode_labels(
    mode: &skilllite_evolution::EvolutionMode,
) -> (&'static str, &'static str) {
    match mode {
        skilllite_evolution::EvolutionMode::All => ("all", "全部启用"),
        skilllite_evolution::EvolutionMode::PromptsOnly => ("prompts", "仅 Prompts"),
        skilllite_evolution::EvolutionMode::MemoryOnly => ("memory", "仅 Memory"),
        skilllite_evolution::EvolutionMode::SkillsOnly => ("skills", "仅 Skills"),
        skilllite_evolution::EvolutionMode::Disabled => ("disabled", "已禁用"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionLogEntryDto {
    pub ts: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    /// `evolution_log.version`（常为本轮 txn，如 `evo_YYYYMMDD_HHMMSS`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatusPayload {
    pub mode_key: String,
    pub mode_label: String,
    pub interval_secs: u64,
    pub decision_threshold: i64,
    /// Weighted sum over the latest `signal_window` meaningful unprocessed decisions (A9).
    pub weighted_signal_sum: i64,
    /// Env `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN` (after workspace merge for interval/threshold only).
    pub weighted_trigger_min: i64,
    pub signal_window: i64,
    /// `default` / `demo` / `conservative`（应用内覆盖优先于工作区 `.env`）。
    pub evo_profile_key: String,
    pub evo_cooldown_hours: f64,
    pub unprocessed_decisions: i64,
    pub last_run_ts: Option<String>,
    /// Latest material `evolution_run` timestamp (passive cooldown / A9 sweep clock).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_material_run_ts: Option<String>,
    pub judgement_label: Option<String>,
    pub judgement_reason: Option<String>,
    pub recent_events: Vec<EvolutionLogEntryDto>,
    pub pending_skill_count: usize,
    /// Read-only A9 arm breakdown (Life Pulse periodic anchor when available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a9: Option<skilllite_evolution::GrowthDueDiagnostics>,
    /// Passive gates + per-arm open state (same thresholds as `should_evolve_impl`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passive: Option<skilllite_evolution::PassiveScheduleDiagnostics>,
    /// Whether `build_evolution_proposals` would be non-empty for this workspace mode.
    pub would_have_evolution_proposals: bool,
    /// When [`Self::would_have_evolution_proposals`] is false: stable engine reason (English).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_proposals_reason: Option<String>,
    pub db_error: Option<String>,
}

/// Evolution feedback DB + schedule hints for the assistant UI.
///
/// `periodic_anchor_unix`: last time the desktop **periodic** arm advanced (`LifePulseState`); pass
/// `None` when unavailable (e.g. first launch — periodic elapsed is reported from “now”).
pub fn load_evolution_status(
    workspace: &str,
    cfg: Option<ChatConfigOverrides>,
    periodic_anchor_unix: Option<i64>,
) -> EvolutionStatusPayload {
    let mode = evolution_mode_from_workspace(workspace);
    let (mode_key, mode_label) = evolution_mode_labels(&mode);

    let schedule_cfg = growth_schedule_merged_for_workspace(workspace, cfg.as_ref());
    let interval_secs = schedule_cfg.interval_secs;
    let decision_threshold = schedule_cfg.raw_unprocessed_threshold;
    let weighted_trigger_min = schedule_cfg.weighted_min;
    let signal_window = schedule_cfg.signal_window;
    let evo_profile_key = effective_evo_profile_key(workspace, cfg.as_ref()).to_string();
    let evo_cooldown_hours = effective_evo_cooldown_hours(workspace, cfg.as_ref());

    let chat_root = skilllite_core::paths::chat_root();
    let mut pending_skill_count = 0;
    if let Some(skills_root) = existing_workspace_skills_root(workspace) {
        pending_skill_count =
            skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root).len();
    }

    let mut db_error = None;
    let mut unprocessed_decisions = 0i64;
    let mut weighted_signal_sum = 0i64;
    let mut recent_events = Vec::new();
    let mut last_run_ts = None;
    let mut last_material_run_ts = None;
    let mut judgement_label = None;
    let mut judgement_reason = None;
    let mut a9 = None;
    let mut passive = None;
    let mut would_have_evolution_proposals = false;
    let mut empty_proposals_reason: Option<String> = None;

    match skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        Ok(conn) => {
            if let Ok(c) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) {
                unprocessed_decisions = c;
            }
            if let Ok(w) = skilllite_evolution::growth_schedule::weighted_unprocessed_signal_sum(
                &conn,
                signal_window,
            ) {
                weighted_signal_sum = w;
            }
            if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn)
            {
                judgement_label = Some(summary.judgement.label_zh().to_string());
                judgement_reason = Some(summary.reason);
            }
            // Last UI "attempt" timestamp: material run or no-output run (constants are fixed literals).
            let last_attempt_sql = format!(
                "SELECT ts FROM evolution_log WHERE type IN ('{}', '{}') ORDER BY ts DESC LIMIT 1",
                skilllite_evolution::feedback::EVOLUTION_LOG_TYPE_RUN_MATERIAL,
                skilllite_evolution::feedback::EVOLUTION_LOG_TYPE_RUN_NOOP,
            );
            if let Ok(mut stmt) = conn.prepare(&last_attempt_sql) {
                if let Ok(mut rows) = stmt.query([]) {
                    if let Ok(Some(row)) = rows.next() {
                        last_run_ts = row.get(0).ok();
                    }
                }
            }
            let last_mat_sql = format!(
                "SELECT ts FROM evolution_log WHERE type = '{}' ORDER BY ts DESC LIMIT 1",
                skilllite_evolution::feedback::EVOLUTION_LOG_TYPE_RUN_MATERIAL,
            );
            if let Ok(mut stmt) = conn.prepare(&last_mat_sql) {
                if let Ok(mut rows) = stmt.query([]) {
                    if let Ok(Some(row)) = rows.next() {
                        last_material_run_ts = row.get(0).ok();
                    }
                }
            }

            let now_unix = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            a9 = skilllite_evolution::inspect_growth_due(
                &conn,
                now_unix,
                periodic_anchor_unix,
                &schedule_cfg,
            )
            .ok();
            passive = skilllite_evolution::passive_schedule_diagnostics(&conn, &mode).ok();
            would_have_evolution_proposals =
                skilllite_evolution::would_have_evolution_proposals(&conn, mode.clone(), false)
                    .unwrap_or(false);
            if !would_have_evolution_proposals {
                empty_proposals_reason =
                    skilllite_evolution::describe_empty_evolution_proposals(&conn, &mode, false)
                        .ok()
                        .map(str::to_string);
            }

            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts, type, target_id, reason, version FROM evolution_log ORDER BY ts DESC LIMIT 25",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let version: Option<String> = row.get(4)?;
                    let txn_id = version
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string());
                    Ok(EvolutionLogEntryDto {
                        ts: row.get(0)?,
                        event_type: row.get(1)?,
                        target_id: row.get::<_, Option<String>>(2)?,
                        reason: row.get::<_, Option<String>>(3)?,
                        txn_id,
                    })
                }) {
                    recent_events.extend(rows.flatten());
                }
            }
        }
        Err(e) => {
            db_error = Some(format!("无法打开进化数据库: {}", e));
        }
    }

    EvolutionStatusPayload {
        mode_key: mode_key.to_string(),
        mode_label: mode_label.to_string(),
        interval_secs,
        decision_threshold,
        weighted_signal_sum,
        weighted_trigger_min,
        signal_window,
        evo_profile_key,
        evo_cooldown_hours,
        unprocessed_decisions,
        last_run_ts,
        last_material_run_ts,
        judgement_label,
        judgement_reason,
        recent_events,
        pending_skill_count,
        a9,
        passive,
        would_have_evolution_proposals,
        empty_proposals_reason,
        db_error,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingSkillDto {
    pub name: String,
    pub needs_review: bool,
    pub preview: String,
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

pub fn list_evolution_pending_skills(workspace: &str) -> Vec<PendingSkillDto> {
    let Some(skills_root) = existing_workspace_skills_root(workspace) else {
        return Vec::new();
    };
    skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root)
        .into_iter()
        .map(|(name, needs_review)| {
            let path = skills_root
                .join("_evolved")
                .join("_pending")
                .join(&name)
                .join("SKILL.md");
            let preview = std::fs::read_to_string(&path)
                .map(|s| truncate_utf8(&s, 4000))
                .unwrap_or_default();
            PendingSkillDto {
                name,
                needs_review,
                preview,
            }
        })
        .collect()
}

pub fn read_evolution_pending_skill_md(
    workspace: &str,
    skill_name: &str,
) -> Result<String, String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    let path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name)
        .join("SKILL.md");
    if !path.is_file() {
        return Err(format!("未找到待审核技能: {}", skill_name));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn evolution_confirm_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())?;
    let chat_root = skilllite_core::paths::chat_root();
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &chat_root,
            "skill_confirmed",
            skill_name,
            "user confirmed (assistant)",
            "",
        );
    }
    Ok(())
}

pub fn evolution_reject_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())
}

pub fn authorize_capability_evolution(
    workspace: &str,
    tool_name: &str,
    outcome: &str,
    summary: &str,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    let proposal_id =
        skilllite_evolution::enqueue_user_capability_evolution(&conn, tool_name, outcome, summary)
            .map_err(|e| e.to_string())?;
    let _ = skilllite_evolution::log_evolution_event(
        &conn,
        &chat_root,
        "capability_evolution_authorized",
        tool_name,
        &format!("outcome={}, proposal_id={}", outcome, proposal_id),
        workspace,
    );
    // User-authorized proposals should be acted on promptly:
    // enqueue first (auditable), then trigger one immediate evolution run in background.
    let workspace_owned = workspace.to_string();
    let proposal_id_owned = proposal_id.clone();
    let skilllite_path_owned = skilllite_path.to_path_buf();
    let chat_root_for_log = chat_root.clone();
    std::thread::spawn(move || {
        let root = find_project_root(&workspace_owned);
        let mut cmd = std::process::Command::new(&skilllite_path_owned);
        crate::windows_spawn::hide_child_console(&mut cmd);
        cmd.arg("evolution")
            .arg("run")
            .arg("--json")
            .current_dir(&root)
            // Do NOT set SKILLLITE_WORKSPACE here: `agent-rpc` chat stores decisions under
            // `skilllite_core::paths::chat_root()` (default `~/.skilllite/chat` when workspace
            // env is unset). Forcing project root makes evolution open `<project>/chat`, a
            // different `feedback.sqlite` — empty decisions → "进化队列为空" and backlog mismatch.
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in load_dotenv_for_child(&workspace_owned) {
            cmd.env(k, v);
        }
        // Must force this backlog row: generic `evolution run` only considers Passive/Active
        // proposals from build_evolution_proposals, not queued user-authorized capability rows.
        cmd.env("SKILLLITE_EVO_FORCE_PROPOSAL_ID", &proposal_id_owned);
        let output = cmd.output();
        let status_note = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let mut merged = stdout.trim().to_string();
                if !stderr.trim().is_empty() {
                    if !merged.is_empty() {
                        merged.push_str(" | ");
                    }
                    merged.push_str(stderr.trim());
                }
                if merged.is_empty() {
                    format!("trigger_exit={}", out.status)
                } else {
                    let mut clipped = merged;
                    if clipped.len() > 280 {
                        clipped.truncate(280);
                        clipped.push('…');
                    }
                    clipped
                }
            }
            Err(e) => format!("trigger_failed: {}", e),
        };
        if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root_for_log) {
            let _ = skilllite_evolution::log_evolution_event(
                &conn,
                &chat_root_for_log,
                "capability_evolution_trigger_run",
                &proposal_id_owned,
                &status_note,
                &workspace_owned,
            );
        }
    });
    Ok(proposal_id)
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionProposalStatusDto {
    pub proposal_id: String,
    pub status: String,
    pub acceptance_status: String,
    pub updated_at: String,
    pub note: Option<String>,
}

pub fn get_evolution_proposal_status(
    _workspace: &str,
    proposal_id: &str,
) -> Result<EvolutionProposalStatusDto, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT proposal_id, status, acceptance_status, updated_at, note
         FROM evolution_backlog
         WHERE proposal_id = ?1
         LIMIT 1",
        [proposal_id],
        |row| {
            Ok(EvolutionProposalStatusDto {
                proposal_id: row.get(0)?,
                status: row.get(1)?,
                acceptance_status: row.get(2)?,
                updated_at: row.get(3)?,
                note: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionBacklogRowDto {
    pub proposal_id: String,
    pub source: String,
    pub risk_level: String,
    pub status: String,
    pub acceptance_status: String,
    pub roi_score: f64,
    pub updated_at: String,
    pub note: String,
}

pub fn load_evolution_backlog(
    _workspace: &str,
    limit: usize,
) -> Result<Vec<EvolutionBacklogRowDto>, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    let limit = limit.clamp(1, 200);
    let mut stmt = conn
        .prepare(
            "SELECT proposal_id, source, risk_level, status, acceptance_status, roi_score, updated_at, COALESCE(note, '')
             FROM evolution_backlog
             WHERE NOT (
               status = 'executed'
               AND COALESCE(acceptance_status, '') IN ('met', 'not_met')
             )
             ORDER BY updated_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(EvolutionBacklogRowDto {
                proposal_id: row.get(0)?,
                source: row.get(1)?,
                risk_level: row.get(2)?,
                status: row.get(3)?,
                acceptance_status: row.get(4)?,
                roi_score: row.get(5)?,
                updated_at: row.get(6)?,
                note: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn trigger_evolution_run(
    workspace: &str,
    proposal_id: Option<&str>,
    _skilllite_path: &std::path::Path,
    overrides: Option<ChatConfigOverrides>,
) -> Result<String, String> {
    fn env_first_non_empty(
        vars: &std::collections::HashMap<String, String>,
        keys: &[&str],
    ) -> Option<String> {
        for k in keys {
            if let Some(v) = vars.get(*k) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        None
    }

    let env_map: std::collections::HashMap<String, String> =
        load_dotenv_for_child(workspace).into_iter().collect();
    let mut api_base = env_first_non_empty(
        &env_map,
        &[
            "SKILLLITE_API_BASE",
            "OPENAI_API_BASE",
            "OPENAI_BASE_URL",
            "BASE_URL",
        ],
    )
    .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_base))
    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let mut api_key = env_first_non_empty(
        &env_map,
        &["SKILLLITE_API_KEY", "OPENAI_API_KEY", "API_KEY"],
    )
    .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_key))
    .unwrap_or_default();
    let mut model = env_first_non_empty(&env_map, &["SKILLLITE_MODEL", "OPENAI_MODEL", "MODEL"])
        .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.model))
        .unwrap_or_else(|| "gpt-4o".to_string());

    // Match chat: keys/base/model from Settings are not written to process env; merge here so
    // manual evolution trigger works when the user only configured the assistant UI.
    if let Some(ref cfg) = overrides {
        if let Some(ref b) = cfg.api_base {
            if !b.trim().is_empty() {
                api_base = b.clone();
            }
        }
        if let Some(ref k) = cfg.api_key {
            if !k.trim().is_empty() {
                api_key = k.clone();
            }
        }
        if let Some(ref m) = cfg.model {
            if !m.trim().is_empty() {
                model = m.clone();
            }
        }
    }

    if api_key.trim().is_empty() {
        return Err(
            "执行 evolution run 失败: 缺少 API key（请配置 SKILLLITE_API_KEY 或 OPENAI_API_KEY）"
                .to_string(),
        );
    }

    let dotenv = load_dotenv_for_child(workspace);
    let merged_vec = merge_dotenv_with_chat_overrides(dotenv, overrides.as_ref());
    let merged_map: std::collections::HashMap<String, String> = merged_vec.into_iter().collect();
    let _evo_env_guard = EvolutionRunEnvGuard::push_from_merged(&merged_map);

    let llm = skilllite_agent::llm::LlmClient::new(&api_base, &api_key)
        .map_err(|e| format!("执行 evolution run 失败: 初始化 LLM 客户端失败: {}", e))?;
    let adapter = skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };
    let skills_root = existing_workspace_skills_root(workspace);

    let prev_force = std::env::var("SKILLLITE_EVO_FORCE_PROPOSAL_ID").ok();
    match proposal_id {
        Some(pid) => std::env::set_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID", pid),
        None => std::env::remove_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID"),
    }
    let run_result = tokio::runtime::Runtime::new()
        .map_err(|e| format!("执行 evolution run 失败: runtime 初始化失败: {}", e))?
        .block_on(skilllite_evolution::run_evolution(
            &skilllite_core::paths::chat_root(),
            skills_root.as_deref(),
            &adapter,
            &api_base,
            &api_key,
            &model,
            true,
        ));
    match prev_force {
        Some(v) => std::env::set_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID", v),
        None => std::env::remove_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID"),
    }

    let response = match run_result {
        Ok(skilllite_evolution::EvolutionRunResult::Completed(Some(txn_id))) => {
            let conn = skilllite_evolution::feedback::open_evolution_db(
                &skilllite_core::paths::chat_root(),
            )
            .map_err(|e| format!("执行 evolution run 失败: 打开进化数据库失败: {}", e))?;
            let changes = skilllite_evolution::query_changes_by_txn(&conn, &txn_id);
            let summary: Vec<String> = skilllite_evolution::format_evolution_changes(&changes);
            if summary.is_empty() {
                format!("Evolution completed (txn={})", txn_id)
            } else {
                summary.join("\n")
            }
        }
        Ok(skilllite_evolution::EvolutionRunResult::SkippedBusy) => {
            "Evolution skipped: another run in progress".to_string()
        }
        Ok(skilllite_evolution::EvolutionRunResult::NoScope)
        | Ok(skilllite_evolution::EvolutionRunResult::Completed(None)) => {
            let mut hint = String::from("Evolution: nothing to evolve");
            if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(
                &skilllite_core::paths::chat_root(),
            ) {
                if let Ok((total, with_desc)) =
                    skilllite_evolution::feedback::count_decisions_with_task_desc(&conn)
                {
                    if total > 0 && with_desc == 0 {
                        hint.push_str("\n\n提示: 进化需要 task_description。当前未进化决策均无 task_description。");
                    } else if total == 0 {
                        let all_count: i64 = conn
                            .query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))
                            .unwrap_or(0);
                        if all_count > 0 {
                            hint.push_str(
                                "\n\n提示: 未进化决策队列为空。已有决策均已标记为已进化。",
                            );
                            hint.push_str("\n请执行新任务积累新决策后再触发进化。");
                        } else {
                            hint.push_str("\n\n提示: 进化队列为空。请先运行 skilllite run 或 skilllite chat 积累决策。");
                        }
                    }
                }
            }
            hint
        }
        Err(e) => return Err(format!("执行 evolution run 失败: {}", e)),
    };

    let mut clipped = response.clone();
    if clipped.len() > 480 {
        clipped.truncate(480);
        clipped.push('…');
    }
    if let Ok(conn) =
        skilllite_evolution::feedback::open_evolution_db(&skilllite_core::paths::chat_root())
    {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &skilllite_core::paths::chat_root(),
            "manual_evolution_run_triggered",
            proposal_id.unwrap_or("all"),
            &clipped,
            workspace,
        );
    }
    Ok(clipped)
}
