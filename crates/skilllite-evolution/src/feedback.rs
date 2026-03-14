//! Evolution feedback collection and evaluation system (EVO-1).

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

// ─── Decision input (agent converts ExecutionFeedback to this) ─────────────────

/// Input for recording a decision. The agent converts its ExecutionFeedback to this.
#[derive(Debug, Clone, Default)]
pub struct DecisionInput {
    pub total_tools: usize,
    pub failed_tools: usize,
    pub replans: usize,
    pub elapsed_ms: u64,
    pub task_completed: bool,
    pub task_description: Option<String>,
    pub rules_used: Vec<String>,
    pub tools_detail: Vec<ToolExecDetail>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolExecDetail {
    pub tool: String,
    pub success: bool,
}

/// User feedback signal for the last decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackSignal {
    ExplicitPositive,
    ExplicitNegative,
    Neutral,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CoreMetrics {
    pub first_success_rate: f64,
    pub avg_replans: f64,
    pub user_correction_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionJudgement {
    Promote,
    KeepObserving,
    Rollback,
}

impl EvolutionJudgement {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Promote => "promote",
            Self::KeepObserving => "keep_observing",
            Self::Rollback => "rollback",
        }
    }

    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Promote => "保留",
            Self::KeepObserving => "继续观察",
            Self::Rollback => "回滚",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JudgementSummary {
    pub judgement: EvolutionJudgement,
    pub current: CoreMetrics,
    pub baseline: Option<CoreMetrics>,
    pub reason: String,
}

impl Default for FeedbackSignal {
    fn default() -> Self {
        Self::Neutral
    }
}

impl FeedbackSignal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExplicitPositive => "pos",
            Self::ExplicitNegative => "neg",
            Self::Neutral => "neutral",
        }
    }
}

// ─── Schema ─────────────────────────────────────────────────────────────────

pub fn ensure_evolution_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS decisions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL DEFAULT (datetime('now')),
            session_id TEXT,
            total_tools INTEGER DEFAULT 0,
            failed_tools INTEGER DEFAULT 0,
            replans INTEGER DEFAULT 0,
            elapsed_ms INTEGER DEFAULT 0,
            task_completed BOOLEAN DEFAULT 0,
            feedback TEXT DEFAULT 'neutral',
            evolved BOOLEAN DEFAULT 0,
            task_description TEXT,
            tools_detail TEXT,
            tool_sequence_key TEXT
        );

        CREATE TABLE IF NOT EXISTS decision_rules (
            decision_id INTEGER REFERENCES decisions(id) ON DELETE CASCADE,
            rule_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS evolution_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL DEFAULT (datetime('now')),
            type TEXT NOT NULL,
            target_id TEXT,
            reason TEXT,
            version TEXT
        );

        CREATE TABLE IF NOT EXISTS evolution_metrics (
            date TEXT PRIMARY KEY,
            first_success_rate REAL,
            avg_replans REAL,
            avg_tool_calls REAL,
            user_correction_rate REAL,
            evolved_rules INTEGER DEFAULT 0,
            effective_rules INTEGER DEFAULT 0,
            egl REAL DEFAULT 0.0
        );

        CREATE INDEX IF NOT EXISTS idx_decisions_evolved ON decisions(evolved);
        CREATE INDEX IF NOT EXISTS idx_decisions_ts ON decisions(ts);
        CREATE INDEX IF NOT EXISTS idx_dr_rule ON decision_rules(rule_id);
        CREATE INDEX IF NOT EXISTS idx_dr_decision ON decision_rules(decision_id);
        CREATE INDEX IF NOT EXISTS idx_evo_log_ts ON evolution_log(ts);
        "#,
    )?;
    // Backward-compatible migration: add column for existing DBs (ignored if column exists).
    let _ = conn.execute("ALTER TABLE decisions ADD COLUMN tool_sequence_key TEXT", []);
    // Index must be created after ALTER TABLE so existing DBs have the column first.
    let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_decisions_seq ON decisions(tool_sequence_key)", []);
    Ok(())
}

/// Build a compact tool-sequence key from tools_detail (at most 3 tools joined by →).
/// Used to group decisions by "what tool pattern was used" rather than raw task description.
/// Example: [weather] → "weather"; [http-request, write_output] → "http-request→write_output".
pub fn compute_tool_sequence_key(tools_detail: &[ToolExecDetail]) -> Option<String> {
    if tools_detail.is_empty() {
        return None;
    }
    let key = tools_detail
        .iter()
        .take(3)
        .map(|t| t.tool.as_str())
        .collect::<Vec<_>>()
        .join("→");
    Some(key)
}

// ─── Decision recording ─────────────────────────────────────────────────────

pub fn insert_decision(
    conn: &Connection,
    session_id: Option<&str>,
    feedback: &DecisionInput,
    user_feedback: FeedbackSignal,
) -> Result<i64> {
    let tools_detail_json = serde_json::to_string(&feedback.tools_detail).unwrap_or_default();
    let tool_sequence_key = compute_tool_sequence_key(&feedback.tools_detail);

    conn.execute(
        "INSERT INTO decisions (session_id, total_tools, failed_tools, replans,
         elapsed_ms, task_completed, feedback, task_description, tools_detail, tool_sequence_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            session_id,
            feedback.total_tools as i64,
            feedback.failed_tools as i64,
            feedback.replans as i64,
            feedback.elapsed_ms as i64,
            feedback.task_completed,
            user_feedback.as_str(),
            feedback.task_description,
            tools_detail_json,
            tool_sequence_key,
        ],
    )?;
    let decision_id = conn.last_insert_rowid();

    if !feedback.rules_used.is_empty() {
        let mut stmt = conn.prepare(
            "INSERT INTO decision_rules (decision_id, rule_id) VALUES (?1, ?2)",
        )?;
        for rule_id in &feedback.rules_used {
            stmt.execute(params![decision_id, rule_id])?;
        }
    }

    Ok(decision_id)
}

pub fn count_unprocessed_decisions(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM decisions WHERE evolved = 0", [], |r| r.get(0))
        .map_err(Into::into)
}

/// Diagnostic: count unprocessed decisions with/without task_description.
/// Evolution requires task_description to learn from decisions.
pub fn count_decisions_with_task_desc(conn: &Connection) -> Result<(i64, i64)> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM decisions WHERE evolved = 0", [], |r| r.get(0))?;
    let with_desc: i64 = conn.query_row(
        "SELECT COUNT(*) FROM decisions WHERE evolved = 0 AND task_description IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    Ok((total, with_desc))
}

pub fn update_last_decision_feedback(
    conn: &Connection,
    session_id: &str,
    feedback: FeedbackSignal,
) -> Result<()> {
    conn.execute(
        "UPDATE decisions SET feedback = ?1
         WHERE id = (SELECT id FROM decisions WHERE session_id = ?2 ORDER BY ts DESC LIMIT 1)",
        params![feedback.as_str(), session_id],
    )?;
    Ok(())
}

// ─── Effectiveness aggregation ──────────────────────────────────────────────

pub fn compute_effectiveness(conn: &Connection, rule_id: &str) -> Result<f32> {
    let result: Result<(i64, i64), _> = conn.query_row(
        "SELECT
            COUNT(CASE WHEN d.task_completed = 1 AND d.feedback != 'neg' THEN 1 END),
            COUNT(*)
         FROM decisions d
         JOIN decision_rules dr ON d.id = dr.decision_id
         WHERE dr.rule_id = ?1 AND d.ts > datetime('now', '-30 days')",
        params![rule_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((success, total)) => {
            if total < 3 {
                Ok(-1.0)
            } else {
                Ok(success as f32 / total as f32)
            }
        }
        Err(_) => Ok(-1.0),
    }
}

// ─── System-level metrics ───────────────────────────────────────────────────

pub fn update_daily_metrics(conn: &Connection) -> Result<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let core = compute_core_metrics_for_date(conn, &today)?;

    let avg_tool_calls: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CAST(total_tools AS REAL)), 0.0)
             FROM decisions
             WHERE date(ts) = ?1 AND total_tools >= 1",
            params![today],
            |row| row.get(0),
        )
        .unwrap_or(0.0);
    let egl = compute_egl(conn, &today).unwrap_or(0.0);

    conn.execute(
        "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans,
         avg_tool_calls, user_correction_rate, egl)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(date) DO UPDATE SET
            first_success_rate = ?2, avg_replans = ?3,
            avg_tool_calls = ?4, user_correction_rate = ?5, egl = ?6",
        params![
            today,
            core.first_success_rate,
            core.avg_replans,
            avg_tool_calls,
            core.user_correction_rate,
            egl
        ],
    )?;

    Ok(())
}

pub fn compute_core_metrics_for_date(conn: &Connection, date: &str) -> Result<CoreMetrics> {
    let stats: Result<(f64, f64, f64), _> = conn.query_row(
        "SELECT
            AVG(CASE WHEN replans = 0 AND task_completed = 1 THEN 1.0 ELSE 0.0 END),
            AVG(CAST(replans AS REAL)),
            CASE WHEN COUNT(CASE WHEN feedback IN ('pos','neg') THEN 1 END) > 0
                 THEN CAST(COUNT(CASE WHEN feedback = 'neg' THEN 1 END) AS REAL)
                      / COUNT(CASE WHEN feedback IN ('pos','neg') THEN 1 END)
                 ELSE 0.0 END
         FROM decisions
         WHERE date(ts) = ?1 AND total_tools >= 1",
        params![date],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );

    let (first_success_rate, avg_replans, user_correction_rate) =
        stats.unwrap_or((0.0, 0.0, 0.0));
    Ok(CoreMetrics {
        first_success_rate,
        avg_replans,
        user_correction_rate,
    })
}

fn query_recent_core_metrics(
    conn: &Connection,
    before_date: Option<&str>,
    limit: usize,
) -> Result<Vec<(String, CoreMetrics)>> {
    let rows = if let Some(date) = before_date {
        let mut stmt = conn.prepare(
            "SELECT date, first_success_rate, avg_replans, user_correction_rate
             FROM evolution_metrics
             WHERE date < ?1
             ORDER BY date DESC
             LIMIT ?2",
        )?;
        let mapped = stmt.query_map(params![date, limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CoreMetrics {
                    first_success_rate: row.get(1)?,
                    avg_replans: row.get(2)?,
                    user_correction_rate: row.get(3)?,
                },
            ))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        let mut stmt = conn.prepare(
            "SELECT date, first_success_rate, avg_replans, user_correction_rate
             FROM evolution_metrics
             ORDER BY date DESC
             LIMIT ?1",
        )?;
        let mapped = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                CoreMetrics {
                    first_success_rate: row.get(1)?,
                    avg_replans: row.get(2)?,
                    user_correction_rate: row.get(3)?,
                },
            ))
        })?;
        mapped.collect::<std::result::Result<Vec<_>, _>>()?
    };

    Ok(rows)
}

fn average_metrics(items: &[CoreMetrics]) -> Option<CoreMetrics> {
    if items.is_empty() {
        return None;
    }
    let len = items.len() as f64;
    Some(CoreMetrics {
        first_success_rate: items.iter().map(|m| m.first_success_rate).sum::<f64>() / len,
        avg_replans: items.iter().map(|m| m.avg_replans).sum::<f64>() / len,
        user_correction_rate: items.iter().map(|m| m.user_correction_rate).sum::<f64>() / len,
    })
}

pub fn build_latest_judgement(conn: &Connection) -> Result<Option<JudgementSummary>> {
    let latest = query_recent_core_metrics(conn, None, 1)?;
    let Some((latest_date, current)) = latest.into_iter().next() else {
        return Ok(None);
    };

    let baseline_samples = query_recent_core_metrics(conn, Some(&latest_date), 7)?;
    let baseline_metrics: Vec<CoreMetrics> = baseline_samples.into_iter().map(|(_, m)| m).collect();
    let baseline = average_metrics(&baseline_metrics);

    let summary = if let Some(baseline) = baseline {
        let fsr_delta = current.first_success_rate - baseline.first_success_rate;
        let replan_delta = current.avg_replans - baseline.avg_replans;
        let ucr_delta = current.user_correction_rate - baseline.user_correction_rate;

        if fsr_delta <= -0.05 || ucr_delta >= 0.10 || replan_delta >= 0.50 {
            JudgementSummary {
                judgement: EvolutionJudgement::Rollback,
                current,
                baseline: Some(baseline),
                reason: format!(
                    "首次成功率较近7日基线下降 {:.1}pct，平均 replan {:+.2}，用户纠正率 {:+.1}pct",
                    fsr_delta * 100.0,
                    replan_delta,
                    ucr_delta * 100.0
                ),
            }
        } else if fsr_delta >= 0.05 && replan_delta <= 0.0 && ucr_delta <= 0.0 {
            JudgementSummary {
                judgement: EvolutionJudgement::Promote,
                current,
                baseline: Some(baseline),
                reason: format!(
                    "首次成功率较近7日基线提升 {:.1}pct，且 replan/用户纠正未恶化",
                    fsr_delta * 100.0
                ),
            }
        } else {
            JudgementSummary {
                judgement: EvolutionJudgement::KeepObserving,
                current,
                baseline: Some(baseline),
                reason: format!(
                    "核心指标波动有限：首次成功率 {:+.1}pct，平均 replan {:+.2}，用户纠正率 {:+.1}pct",
                    fsr_delta * 100.0,
                    replan_delta,
                    ucr_delta * 100.0
                ),
            }
        }
    } else {
        JudgementSummary {
            judgement: EvolutionJudgement::KeepObserving,
            current,
            baseline: None,
            reason: "历史基线不足，先继续观察三项核心指标".to_string(),
        }
    };

    Ok(Some(summary))
}

fn compute_egl(conn: &Connection, date: &str) -> Result<f64> {
    let new_items: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log
             WHERE date(ts) = ?1 AND type IN ('rule_added', 'example_added', 'skill_generated')",
            params![date],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decisions
             WHERE date(ts) = ?1 AND total_tools >= 1",
            params![date],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if total_triggers == 0 {
        return Ok(0.0);
    }
    Ok(new_items as f64 / total_triggers as f64 * 1000.0)
}

/// 滚动窗口 EGL：过去 N 天内 (新增进化条数 / 触发数) * 1000，用于看近期整体。
pub fn compute_egl_rolling(conn: &Connection, days: u32) -> Result<f64> {
    let modifier = format!("-{} days", days);
    let new_items: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log
             WHERE date(ts) >= date('now', ?1) AND type IN ('rule_added', 'example_added', 'skill_generated')",
            params![modifier],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total_triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decisions
             WHERE date(ts) >= date('now', ?1) AND total_tools >= 1",
            params![modifier],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if total_triggers == 0 {
        return Ok(0.0);
    }
    Ok(new_items as f64 / total_triggers as f64 * 1000.0)
}

/// 全量 EGL：至今 (新增进化条数 / 触发数) * 1000，用于看全局进化率。
pub fn compute_egl_all_time(conn: &Connection) -> Result<f64> {
    let new_items: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log
             WHERE type IN ('rule_added', 'example_added', 'skill_generated')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let total_triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decisions WHERE total_tools >= 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if total_triggers == 0 {
        return Ok(0.0);
    }
    Ok(new_items as f64 / total_triggers as f64 * 1000.0)
}

// ─── Time trends ─────────────────────────────────────────────────────────────

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Debug)]
pub struct WeekdayActivity {
    pub weekday: i32,
    pub weekday_name: &'static str,
    pub count: i64,
    pub success_rate: f64,
    pub dominant_task: Option<String>,
}

pub fn query_weekday_activity(conn: &Connection) -> Result<Vec<WeekdayActivity>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%w', ts) AS INTEGER) as wd,
                COUNT(*) as cnt,
                AVG(CASE WHEN task_completed = 1 THEN 1.0 ELSE 0.0 END) as sr
         FROM decisions
         WHERE ts > datetime('now', '-30 days') AND total_tools >= 2
         GROUP BY wd ORDER BY wd",
    )?;
    let mut results: Vec<WeekdayActivity> = stmt
        .query_map([], |row| {
            let wd: i32 = row.get(0)?;
            Ok(WeekdayActivity {
                weekday: wd,
                weekday_name: WEEKDAY_NAMES[wd as usize % 7],
                count: row.get(1)?,
                success_rate: row.get(2)?,
                dominant_task: None,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    for entry in &mut results {
        if let Ok(task) = conn.query_row(
            "SELECT task_description FROM decisions
             WHERE CAST(strftime('%w', ts) AS INTEGER) = ?1
               AND ts > datetime('now', '-30 days')
               AND task_description IS NOT NULL AND total_tools >= 2
             GROUP BY task_description ORDER BY COUNT(*) DESC LIMIT 1",
            params![entry.weekday],
            |row| row.get::<_, String>(0),
        ) {
            entry.dominant_task = Some(task);
        }
    }

    Ok(results)
}

pub fn query_peak_hours(conn: &Connection) -> Result<Vec<(i32, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', ts) AS INTEGER) as hour, COUNT(*) as cnt
         FROM decisions
         WHERE ts > datetime('now', '-30 days') AND total_tools >= 2
         GROUP BY hour ORDER BY cnt DESC LIMIT 3",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

// ─── Export ─────────────────────────────────────────────────────────────────

pub fn export_decisions_md(conn: &Connection, output_path: &Path) -> Result<()> {
    let mut md = String::from(
        "# SkillLite 进化决策记录\n\n\
         > 自动维护。每次进化事件追加一行。\n\n\
         ## 进化决策\n\n\
         | 日期 | 决策 | 效果 |\n\
         |------|------|------|\n",
    );

    let mut stmt = conn.prepare(
        "SELECT ts, type, target_id, reason FROM evolution_log
         ORDER BY ts DESC LIMIT 50",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    for row in rows {
        let (ts, etype, target_id, reason) = row?;
        let date = &ts[..10.min(ts.len())];
        let target = target_id.unwrap_or_default();
        let reason_text = reason.unwrap_or_default();

        let (icon, desc) = match etype.as_str() {
            "rule_added" => ("✅", format!("新增规则 {}: {}", target, reason_text)),
            "example_added" => ("✅", format!("新增示例 {}: {}", target, reason_text)),
            "skill_generated" => ("✅", format!("自动生成 Skill {}", target)),
            "rule_retired" => ("❌", format!("退役规则 {}: {}", target, reason_text)),
            t if t.ends_with("_rolled_back") => {
                ("🔙", format!("回滚 {}: {}", target, reason_text))
            }
            _ => ("—", format!("{} {}", etype, target)),
        };

        md.push_str(&format!("| {} | {} | {} |\n", date, desc, icon));
    }

    md.push_str("\n## 核心指标趋势 (最近7天)\n\n");
    md.push_str("| 日期 | 首次成功率 | 平均replan | 用户纠正率 |\n");
    md.push_str("|------|-----------|-----------|-----------|\n");

    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, avg_replans, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-7 days') ORDER BY date DESC",
    )?;
    let metrics = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })?;

    for m in metrics {
        let (date, fsr, avg_r, ucr) = m?;
        md.push_str(&format!(
            "| {} | {:.0}% | {:.1} | {:.0}% |\n",
            date, fsr * 100.0, avg_r, ucr * 100.0
        ));
    }

    if let Some(summary) = build_latest_judgement(conn)? {
        md.push_str(&format!(
            "\n**本轮简单判断:** {} (`{}`)\n\n",
            summary.judgement.label_zh(),
            summary.judgement.as_str()
        ));
        md.push_str(&format!("原因: {}\n", summary.reason));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, md)?;
    Ok(())
}

// ─── Rule history ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct EvolutionHistoryEntry {
    pub ts: String,
    pub event_type: String,
    pub reason: String,
    pub txn_id: String,
}

pub fn query_rule_history(conn: &Connection, target_id: &str) -> Result<Vec<EvolutionHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT ts, type, reason, version FROM evolution_log
         WHERE target_id = ?1 ORDER BY ts DESC LIMIT 10",
    )?;
    let rows = stmt.query_map(params![target_id], |row| {
        Ok(EvolutionHistoryEntry {
            ts: row.get(0)?,
            event_type: row.get(1)?,
            reason: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            txn_id: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}

// ─── Promotable external rules ──────────────────────────────────────────────

pub fn find_promotable_external_rules(conn: &Connection, chat_root: &Path) -> Result<Vec<String>> {
    let rules = crate::seed::load_rules(chat_root);
    let mut promotable = Vec::new();
    for rule in rules.iter().filter(|r| r.origin == "external" && r.priority < 65) {
        let eff = compute_effectiveness(conn, &rule.id)?;
        if eff >= 0.7 {
            promotable.push(rule.id.clone());
        }
    }
    Ok(promotable)
}

// ─── Open evolution DB ──────────────────────────────────────────────────────

pub fn open_evolution_db(chat_root: &Path) -> Result<Connection> {
    let db_path = chat_root.join("memory").join("default.sqlite");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    ensure_evolution_tables(&conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_decision_persists_rules() {
        let conn = setup_conn();
        let input = DecisionInput {
            total_tools: 2,
            failed_tools: 0,
            replans: 0,
            elapsed_ms: 100,
            task_completed: true,
            task_description: Some("test".to_string()),
            rules_used: vec!["rule-a".to_string(), "rule-b".to_string()],
            tools_detail: vec![ToolExecDetail {
                tool: "read_file".to_string(),
                success: true,
            }],
        };

        let id = insert_decision(&conn, Some("s1"), &input, FeedbackSignal::Neutral).unwrap();
        let mut stmt = conn
            .prepare("SELECT rule_id FROM decision_rules WHERE decision_id = ?1 ORDER BY rule_id")
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map(params![id], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(rows, vec!["rule-a".to_string(), "rule-b".to_string()]);
    }

    #[test]
    fn test_compute_core_metrics_for_date_uses_minimal_metrics() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO decisions (ts, total_tools, replans, task_completed, feedback)
             VALUES
             ('2026-03-14 09:00:00', 1, 0, 1, 'neutral'),
             ('2026-03-14 10:00:00', 2, 1, 1, 'neg'),
             ('2026-03-14 11:00:00', 1, 2, 0, 'pos')",
            [],
        )
        .unwrap();

        let metrics = compute_core_metrics_for_date(&conn, "2026-03-14").unwrap();
        assert!((metrics.first_success_rate - (1.0 / 3.0)).abs() < 1e-6);
        assert!((metrics.avg_replans - 1.0).abs() < 1e-6);
        assert!((metrics.user_correction_rate - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_compute_core_metrics_for_date_avg_replans_and_user_correction_rate() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO decisions (ts, total_tools, replans, task_completed, feedback)
             VALUES
             ('2026-03-15 09:00:00', 1, 0, 1, 'neutral'),
             ('2026-03-15 10:00:00', 2, 1, 1, 'neg'),
             ('2026-03-15 11:00:00', 1, 2, 0, 'pos'),
             ('2026-03-15 12:00:00', 3, 3, 1, 'neutral'),
             ('2026-03-15 13:00:00', 1, 0, 1, 'pos')",
            [],
        )
        .unwrap();

        let metrics = compute_core_metrics_for_date(&conn, "2026-03-15").unwrap();
        // avg_replans: (0 + 1 + 2 + 3 + 0) / 5 = 6 / 5 = 1.2
        assert!((metrics.avg_replans - 1.2).abs() < 1e-6);
        // user_correction_rate: neg (1) / (pos (2) + neg (1)) = 1 / 3 = 0.333...
        assert!((metrics.user_correction_rate - (1.0 / 3.0)).abs() < 1e-6);
    }

    #[test]
    fn test_build_latest_judgement_promotes_improving_metrics() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES
             ('2026-03-10', 0.40, 1.5, 3.0, 0.30, 0.0),
             ('2026-03-11', 0.50, 1.4, 3.0, 0.20, 0.0),
             ('2026-03-12', 0.55, 1.2, 3.0, 0.15, 0.0),
             ('2026-03-14', 0.72, 0.8, 2.5, 0.10, 0.0)",
            [],
        )
        .unwrap();

        let summary = build_latest_judgement(&conn).unwrap().unwrap();
        assert_eq!(summary.judgement, EvolutionJudgement::Promote);
    }
}
