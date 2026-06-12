//! Machine-readable evolution status for desktop / integrators (`skilllite evolution status --json`).
//!
//! JSON shape matches `skilllite-assistant` `EvolutionStatusPayload` (L2 contract).

use std::path::{Path, PathBuf};

use serde::Serialize;
use skilllite_core::config::env_keys::evolution as evo_env;
use skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback;
use skilllite_evolution::growth_schedule::GrowthScheduleConfig;
use skilllite_evolution::{GrowthDueDiagnostics, PassiveScheduleDiagnostics};

use crate::Result;

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionLogEntrySnapshot {
    pub ts: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
}

/// Desktop-compatible evolution status payload (see `ASSISTANT-SPLIT-ARCHITECTURE.md` §5.2).
#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatusSnapshot {
    pub mode_key: String,
    pub mode_label: String,
    pub interval_secs: u64,
    pub decision_threshold: i64,
    pub weighted_signal_sum: i64,
    pub weighted_trigger_min: i64,
    pub signal_window: i64,
    pub evo_profile_key: String,
    pub evo_cooldown_hours: f64,
    pub unprocessed_decisions: i64,
    pub last_run_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_material_run_ts: Option<String>,
    pub judgement_label: Option<String>,
    pub judgement_reason: Option<String>,
    pub recent_events: Vec<EvolutionLogEntrySnapshot>,
    pub pending_skill_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a9: Option<GrowthDueDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passive: Option<PassiveScheduleDiagnostics>,
    pub would_have_evolution_proposals: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_proposals_reason: Option<String>,
    pub db_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvolutionStatusParams {
    pub workspace: String,
    pub periodic_anchor_unix: Option<i64>,
}

pub(crate) fn resolve_workspace_root(workspace: &str) -> PathBuf {
    let raw = workspace.trim();
    let p = if raw.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(raw)
    };
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

pub(crate) fn chat_root_for_workspace(workspace: &str) -> PathBuf {
    resolve_workspace_root(workspace).join("chat")
}

fn workspace_env_lookup(workspace_root: &Path, key: &str) -> Option<String> {
    skilllite_core::config::parse_dotenv_from_dir(workspace_root)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .or_else(|| std::env::var(key).ok())
}

fn evolution_mode_from_workspace(workspace_root: &Path) -> skilllite_evolution::EvolutionMode {
    match workspace_env_lookup(workspace_root, evo_env::SKILLLITE_EVOLUTION).as_deref() {
        None | Some("1") | Some("true") | Some("") => skilllite_evolution::EvolutionMode::All,
        Some("0") | Some("false") => skilllite_evolution::EvolutionMode::Disabled,
        Some("prompts") => skilllite_evolution::EvolutionMode::PromptsOnly,
        Some("memory") => skilllite_evolution::EvolutionMode::MemoryOnly,
        Some("skills") => skilllite_evolution::EvolutionMode::SkillsOnly,
        _ => skilllite_evolution::EvolutionMode::All,
    }
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

fn growth_schedule_for_workspace(workspace_root: &Path) -> GrowthScheduleConfig {
    let mut c = GrowthScheduleConfig::from_env();
    c.interval_secs =
        workspace_env_lookup(workspace_root, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
            .and_then(|v| v.parse().ok())
            .unwrap_or(c.interval_secs);
    c.raw_unprocessed_threshold = workspace_env_lookup(
        workspace_root,
        evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD,
    )
    .and_then(|v| v.parse().ok())
    .unwrap_or(c.raw_unprocessed_threshold);
    c
}

fn effective_evo_profile_key(workspace_root: &Path) -> &'static str {
    match workspace_env_lookup(workspace_root, evo_env::SKILLLITE_EVO_PROFILE).as_deref() {
        Some("demo") => "demo",
        Some("conservative") => "conservative",
        _ => "default",
    }
}

fn effective_evo_cooldown_hours(workspace_root: &Path) -> f64 {
    workspace_env_lookup(workspace_root, evo_env::SKILLLITE_EVO_COOLDOWN_HOURS)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| skilllite_evolution::EvolutionThresholds::default().cooldown_hours)
}

fn existing_workspace_skills_root(workspace_root: &Path) -> Option<PathBuf> {
    let skills_root =
        resolve_skills_dir_with_legacy_fallback(workspace_root, "skills").effective_path;
    skills_root.is_dir().then_some(skills_root)
}

/// Build evolution status snapshot (workspace `.env` + process env; no desktop UI overrides).
pub fn build_evolution_status_snapshot(params: &EvolutionStatusParams) -> EvolutionStatusSnapshot {
    let workspace_root = resolve_workspace_root(&params.workspace);
    skilllite_core::config::load_dotenv_from_dir(&workspace_root);

    let mode = evolution_mode_from_workspace(&workspace_root);
    let (mode_key, mode_label) = evolution_mode_labels(&mode);
    let schedule_cfg = growth_schedule_for_workspace(&workspace_root);

    let mut pending_skill_count = 0;
    if let Some(skills_root) = existing_workspace_skills_root(&workspace_root) {
        pending_skill_count =
            skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root).len();
    }

    let chat_root = workspace_root.join("chat");
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
    let mut empty_proposals_reason = None;

    match skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        Ok(conn) => {
            if let Ok(c) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) {
                unprocessed_decisions = c;
            }
            if let Ok(w) = skilllite_evolution::growth_schedule::weighted_unprocessed_signal_sum(
                &conn,
                schedule_cfg.signal_window,
            ) {
                weighted_signal_sum = w;
            }
            if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn)
            {
                judgement_label = Some(summary.judgement.label_zh().to_string());
                judgement_reason = Some(summary.reason);
            }
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
                params.periodic_anchor_unix,
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
                    Ok(EvolutionLogEntrySnapshot {
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
            db_error = Some(format!("cannot open evolution database: {}", e));
        }
    }

    EvolutionStatusSnapshot {
        mode_key: mode_key.to_string(),
        mode_label: mode_label.to_string(),
        interval_secs: schedule_cfg.interval_secs,
        decision_threshold: schedule_cfg.raw_unprocessed_threshold,
        weighted_signal_sum,
        weighted_trigger_min: schedule_cfg.weighted_min,
        signal_window: schedule_cfg.signal_window,
        evo_profile_key: effective_evo_profile_key(&workspace_root).to_string(),
        evo_cooldown_hours: effective_evo_cooldown_hours(&workspace_root),
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

/// `skilllite evolution status` — human table or JSON snapshot.
pub fn cmd_status(json: bool, workspace: &str, periodic_anchor_unix: Option<i64>) -> Result<()> {
    if json {
        let snapshot = build_evolution_status_snapshot(&EvolutionStatusParams {
            workspace: workspace.to_string(),
            periodic_anchor_unix,
        });
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }
    cmd_status_human(workspace)
}

fn reason_preview(reason: &str) -> String {
    const DISPLAY_LIMIT_BYTES: usize = 50;
    const PREFIX_LIMIT_BYTES: usize = 47;

    if reason.len() <= DISPLAY_LIMIT_BYTES {
        return reason.to_string();
    }

    let mut end = PREFIX_LIMIT_BYTES.min(reason.len());
    while end > 0 && !reason.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &reason[..end])
}

fn cmd_status_human(workspace: &str) -> Result<()> {
    let workspace_root = resolve_workspace_root(workspace);
    skilllite_core::config::load_dotenv_from_dir(&workspace_root);
    let root = workspace_root.join("chat");
    let conn = skilllite_evolution::feedback::open_evolution_db(&root)?;
    let mode = evolution_mode_from_workspace(&workspace_root);

    println!("╭─────────────────────────────────────────────╮");
    println!("│       SkillLite 自进化引擎状态               │");
    println!("╰─────────────────────────────────────────────╯");
    println!();

    let (_, mode_label) = evolution_mode_labels(&mode);
    println!("进化模式: {}", mode_label);
    println!();

    println!("📈 核心指标趋势 (最近 7 天)");
    println!(
        "  {:10} {:>8} {:>8} {:>8}",
        "日期", "成功率", "Replan", "纠正率"
    );
    println!(
        "  {:10} {:>8} {:>8} {:>8}",
        "──────────", "────────", "────────", "────────"
    );

    let mut stmt = conn
        .prepare(
            "SELECT date, first_success_rate, avg_replans, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-7 days') ORDER BY date DESC",
        )
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    let metrics = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;

    let mut has_metrics = false;
    for m in metrics {
        let (date, fsr, avg_r, ucr) = m.map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
        println!(
            "  {:10} {:>7.0}% {:>8.1} {:>7.0}%",
            date,
            fsr * 100.0,
            avg_r,
            ucr * 100.0,
        );
        has_metrics = true;
    }
    if !has_metrics {
        println!("  (暂无数据 — 需要更多使用后才会出现)");
    }
    if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn) {
        println!(
            "  — 当前简单判断: {} ({})",
            summary.judgement.label_zh(),
            summary.judgement.as_str()
        );
        println!("    原因: {}", summary.reason);
    }
    println!();

    println!("📜 最近进化事件");
    let mut stmt = conn
        .prepare(
            "SELECT ts, type, target_id, reason FROM evolution_log
         ORDER BY ts DESC LIMIT 10",
        )
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    let events = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;

    let mut has_events = false;
    for e in events {
        let (ts, etype, target, reason) =
            e.map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
        let date = &ts[..std::cmp::min(16, ts.len())];
        let target = target.unwrap_or_default();
        let reason = reason.unwrap_or_default();
        let icon = match etype.as_str() {
            "rule_added" => "✅",
            "example_added" => "📖",
            "skill_generated" => "✨",
            "skill_pending" => "🆕",
            "skill_refined" => "🔧",
            "evolution_judgement" => "🧭",
            "auto_rollback" => "⚠️ ",
            t if t.contains("retired") => "🗑️ ",
            t if t.contains("rolled_back") => "🔙",
            _ => "  ",
        };
        let reason_short = reason_preview(&reason);
        println!("  {} {} {} {}", icon, date, etype, reason_short);
        if !target.is_empty() {
            println!("     └─ target: {}", target);
        }
        has_events = true;
    }
    if !has_events {
        println!("  (暂无进化事件)");
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod workspace_scope_tests {
    use super::*;
    use skilllite_core::config::env_keys::paths as env_paths;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            skilllite_core::config::set_env_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                skilllite_core::config::set_env_var(self.key, value);
            } else {
                skilllite_core::config::remove_env_var(self.key);
            }
        }
    }

    fn temp_workspace(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "skilllite-evo-status-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn seed_decision(workspace: &Path) {
        let conn = skilllite_evolution::feedback::open_evolution_db(&workspace.join("chat"))
            .expect("open db");
        conn.execute(
            "INSERT INTO decisions
             (evolved, total_tools, failed_tools, replans, task_completed, task_description, ts)
             VALUES (0, 1, 0, 0, 1, 'workspace scoped decision', datetime('now'))",
            [],
        )
        .expect("insert decision");
    }

    #[test]
    fn status_snapshot_uses_workspace_argument_for_db_over_env() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let env_workspace = temp_workspace("env");
        let target_workspace = temp_workspace("target");
        let _env_restore = EnvRestore::set(
            env_paths::SKILLLITE_WORKSPACE,
            env_workspace.to_string_lossy().as_ref(),
        );
        let _ = skilllite_evolution::feedback::open_evolution_db(&env_workspace.join("chat"))
            .expect("open env db");
        seed_decision(&target_workspace);

        let snapshot = build_evolution_status_snapshot(&EvolutionStatusParams {
            workspace: target_workspace.to_string_lossy().to_string(),
            periodic_anchor_unix: None,
        });

        assert_eq!(snapshot.unprocessed_decisions, 1);
        assert!(snapshot.db_error.is_none());
        let _ = std::fs::remove_dir_all(env_workspace);
        let _ = std::fs::remove_dir_all(target_workspace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_preview_truncates_on_utf8_boundary() {
        let reason = "界".repeat(17);

        let preview = reason_preview(&reason);

        assert_eq!(preview, format!("{}...", "界".repeat(15)));
    }

    #[test]
    fn reason_preview_keeps_short_text_unchanged() {
        assert_eq!(reason_preview("short reason"), "short reason");
    }

    #[test]
    fn evolution_status_snapshot_serializes_required_fields() {
        let snap = EvolutionStatusSnapshot {
            mode_key: "all".into(),
            mode_label: "全部启用".into(),
            interval_secs: 600,
            decision_threshold: 10,
            weighted_signal_sum: 0,
            weighted_trigger_min: 3,
            signal_window: 20,
            evo_profile_key: "default".into(),
            evo_cooldown_hours: 24.0,
            unprocessed_decisions: 0,
            last_run_ts: None,
            last_material_run_ts: None,
            judgement_label: None,
            judgement_reason: None,
            recent_events: vec![],
            pending_skill_count: 0,
            a9: None,
            passive: None,
            would_have_evolution_proposals: false,
            empty_proposals_reason: None,
            db_error: None,
        };
        let v = serde_json::to_value(&snap).expect("serialize");
        assert!(v.get("mode_key").is_some());
        assert!(v.get("would_have_evolution_proposals").is_some());
        assert!(v.get("recent_events").is_some());
    }
}
