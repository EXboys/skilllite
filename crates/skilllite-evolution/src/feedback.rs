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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleHistoryEntry {
    pub ts: String,
    pub event_type: String,
    pub txn_id: String,
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

pub fn open_evolution_db(chat_root: &Path) -> Result<Connection> {
    let db_path = chat_root.join("feedback.sqlite");
    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    ensure_evolution_tables(&conn)?;
    Ok(conn)
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
        "#,)
    ?;
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

pub fn query_rule_history(conn: &Connection, rule_id: &str) -> Result<Vec<RuleHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT ts, type, COALESCE(version, ''), COALESCE(reason, '')
         FROM evolution_log
         WHERE target_id = ?1
         ORDER BY ts DESC",
    )?;

    let rows = stmt.query_map(params![rule_id], |row| {
        Ok(RuleHistoryEntry {
            ts: row.get(0)?,
            event_type: row.get(1)?,
            txn_id: row.get(2)?,
            reason: row.get(3)?,
        })
    })?;

    let entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(entries)
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
    // first_success_rate: (total decisions where task_completed = 1 and feedback != 'neg') / total decisions
    let (success_count, total_count): (i64, i64) = conn.query_row(
        "SELECT
            COUNT(CASE WHEN task_completed = 1 AND feedback != 'neg' THEN 1 END),
            COUNT(*)
         FROM decisions
         WHERE date(ts) = ?1",
        params![date],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let first_success_rate = if total_count > 0 {
        success_count as f64 / total_count as f64
    } else {
        0.0
    };

    // avg_replans: average of 'replans' for all decisions
    let avg_replans: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CAST(replans AS REAL)), 0.0) FROM decisions WHERE date(ts) = ?1",
            params![date],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // user_correction_rate: count of 'neg' feedbacks / (count of 'pos' feedbacks + count of 'neg' feedbacks)
    let (pos_feedback_count, neg_feedback_count): (i64, i64) = conn.query_row(
        "SELECT
            COUNT(CASE WHEN feedback = 'pos' THEN 1 END),
            COUNT(CASE WHEN feedback = 'neg' THEN 1 END)
         FROM decisions
         WHERE date(ts) = ?1",
        params![date],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let user_correction_rate = if (pos_feedback_count + neg_feedback_count) > 0 {
        neg_feedback_count as f64 / (pos_feedback_count + neg_feedback_count) as f64
    } else {
        0.0
    };

    Ok(CoreMetrics {
        first_success_rate,
        avg_replans,
        user_correction_rate,
    })
}

// ─── Evolution decision making ────────────────────────────────────────────────

/// EGL (Evolutionary Grade Level) captures a single, combined score of "goodness"
/// of a given rule or agent behavior.
/// EGL = (first_success_rate * A) - (avg_replans * B) - (user_correction_rate * C)
/// where A, B, C are configurable weights.
/// A higher EGL indicates better performance.
///
/// This is meant for comparing rule effectiveness, so it's only computed for specific rules.
pub fn compute_egl_for_rule(conn: &Connection, rule_id: &str) -> Result<f64> {
    let (success_count, total_count, total_replans, pos_feedback, neg_feedback): (
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = conn.query_row(
        "SELECT
            COUNT(CASE WHEN d.task_completed = 1 AND d.feedback != 'neg' THEN 1 END),
            COUNT(*),
            SUM(d.replans),
            COUNT(CASE WHEN d.feedback = 'pos' THEN 1 END),
            COUNT(CASE WHEN d.feedback = 'neg' THEN 1 END)
         FROM decisions d
         JOIN decision_rules dr ON d.id = dr.decision_id
         WHERE dr.rule_id = ?1 AND d.ts > datetime('now', '-30 days')", // Last 30 days
        params![rule_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )?;

    if total_count == 0 {
        return Ok(0.0);
    }

    let success_rate = success_count as f64 / total_count as f64;
    let avg_replans = total_replans as f64 / total_count as f64;
    let user_correction_rate = if (pos_feedback + neg_feedback) > 0 {
        neg_feedback as f64 / (pos_feedback + neg_feedback) as f64
    } else {
        0.0
    };

    // Weights (can be configured)
    let w_success = 1.0;
    let w_replans = 0.5;
    let w_correction = 0.7;

    let egl = (success_rate * w_success) - (avg_replans * w_replans) - (user_correction_rate * w_correction);
    Ok(egl)
}

/// System-wide EGL based on global metrics for the current day.
pub fn compute_egl(conn: &Connection, date: &str) -> Result<f64> {
    let metrics = compute_core_metrics_for_date(conn, date)?;

    // Weights (can be configured)
    let w_success = 1.0;
    let w_replans = 0.5;
    let w_correction = 0.7;

    let egl = (metrics.first_success_rate * w_success)
        - (metrics.avg_replans * w_replans)
        - (metrics.user_correction_rate * w_correction);
    Ok(egl)
}

pub fn fetch_latest_metrics(conn: &Connection) -> Result<Option<CoreMetrics>> {
    let mut stmt = conn.prepare(
        "SELECT first_success_rate, avg_replans, user_correction_rate
         FROM evolution_metrics ORDER BY date DESC LIMIT 1",
    )?;
    let metrics = stmt.query_row([], |row| {
        Ok(CoreMetrics {
            first_success_rate: row.get(0)?,
            avg_replans: row.get(1)?,
            user_correction_rate: row.get(2)?,
        })
    });
    match metrics {
        Ok(m) => Ok(Some(m)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn build_latest_judgement(conn: &Connection) -> Result<Option<JudgementSummary>> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let current_metrics = compute_core_metrics_for_date(conn, &today)?;
    let baseline_metrics = fetch_latest_metrics(conn)?;

    if let Some(baseline) = baseline_metrics {
        let mut reason_parts = vec![];
        let mut promote_score = 0; // +1 for improvement, -1 for degradation
        let mut degrade_score = 0;

        if current_metrics.first_success_rate > baseline.first_success_rate {
            reason_parts.push(format!(
                "首次成功率从 {:.2}% 提升到 {:.2}%",
                baseline.first_success_rate * 100.0,
                current_metrics.first_success_rate * 100.0
            ));
            promote_score += 1;
        } else if current_metrics.first_success_rate < baseline.first_success_rate {
            reason_parts.push(format!(
                "首次成功率从 {:.2}% 下降到 {:.2}%",
                baseline.first_success_rate * 100.0,
                current_metrics.first_success_rate * 100.0
            ));
            degrade_score += 1;
        }

        if current_metrics.avg_replans < baseline.avg_replans {
            reason_parts.push(format!(
                "平均重试次数从 {:.2} 减少到 {:.2}",
                baseline.avg_replans, current_metrics.avg_replans
            ));
            promote_score += 1;
        } else if current_metrics.avg_replans > baseline.avg_replans {
            reason_parts.push(format!(
                "平均重试次数从 {:.2} 增加到 {:.2}",
                baseline.avg_replans, current_metrics.avg_replans
            ));
            degrade_score += 1;
        }

        if current_metrics.user_correction_rate < baseline.user_correction_rate {
            reason_parts.push(format!(
                "用户修正率从 {:.2}% 减少到 {:.2}%",
                baseline.user_correction_rate * 100.0,
                current_metrics.user_correction_rate * 100.0
            ));
            promote_score += 1;
        } else if current_metrics.user_correction_rate > baseline.user_correction_rate {
            reason_parts.push(format!(
                "用户修正率从 {:.2}% 增加到 {:.2}%",
                baseline.user_correction_rate * 100.0,
                current_metrics.user_correction_rate * 100.0
            ));
            degrade_score += 1;
        }

        let judgement = if promote_score > degrade_score && promote_score > 0 {
            EvolutionJudgement::Promote
        } else if degrade_score > promote_score && degrade_score > 0 {
            EvolutionJudgement::Rollback
        } else {
            EvolutionJudgement::KeepObserving
        };

        let reason = if reason_parts.is_empty() {
            "指标无显著变化".to_string()
        } else {
            reason_parts.join("，")
        };

        Ok(Some(JudgementSummary {
            judgement,
            current: current_metrics,
            baseline: Some(baseline),
            reason,
        }))
    } else {
        // No baseline data, keep observing
        Ok(Some(JudgementSummary {
            judgement: EvolutionJudgement::KeepObserving,
            current: current_metrics,
            baseline: None,
            reason: "无基线数据，继续观察".to_string(),
        }))
    }
}

// ─── Logging and persistence ────────────────────────────────────────────────

pub fn log_evolution_event(
    conn: &Connection,
    event_type: &str,
    target_id: Option<&str>,
    reason: Option<&str>,
    version: Option<&str>,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO evolution_log (type, target_id, reason, version) VALUES (?1, ?2, ?3, ?4)",
        params![event_type, target_id, reason, version],
    )?;
    Ok(conn.last_insert_rowid())
}

// ─── Test helpers ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_ensure_evolution_tables() {
        let conn = setup_conn();
        let tables = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='decisions'",
                [],
                |r| r.get::<_, String>(0),
            )
            .unwrap();
        assert_eq!(tables, "decisions");
    }

    #[test]
    fn test_insert_decision() {
        let conn = setup_conn();
        let input = DecisionInput {
            total_tools: 1,
            failed_tools: 0,
            replans: 0,
            elapsed_ms: 100,
            task_completed: true,
            task_description: Some("test task".to_string()),
            rules_used: vec![],
            tools_detail: vec![],
        };
        let decision_id = insert_decision(&conn, Some("session1"), &input, FeedbackSignal::Neutral)
            .unwrap();
        assert!(decision_id > 0);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_update_last_decision_feedback() {
        let conn = setup_conn();
        let input = DecisionInput {
            total_tools: 1,
            failed_tools: 0,
            replans: 0,
            elapsed_ms: 100,
            task_completed: true,
            task_description: Some("test task".to_string()),
            rules_used: vec![],
            tools_detail: vec![],
        };
        insert_decision(&conn, Some("s1"), &input, FeedbackSignal::Neutral).unwrap();
        update_last_decision_feedback(&conn, "s1", FeedbackSignal::ExplicitPositive).unwrap();

        let feedback: String = conn
            .query_row("SELECT feedback FROM decisions WHERE session_id = 's1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(feedback, "pos");
    }

    #[test]
    fn test_compute_effectiveness() {
        let conn = setup_conn();
        conn.execute_batch(
            "INSERT INTO decisions (ts, task_completed, feedback) VALUES
             ('2026-03-14 09:00:00', 1, 'pos'),
             ('2026-03-14 10:00:00', 0, 'neg'),
             ('2026-03-14 11:00:00', 1, 'neutral'),
             ('2026-03-14 12:00:00', 1, 'pos');
             INSERT INTO decision_rules (decision_id, rule_id) VALUES
             (1, 'test-rule'), (2, 'test-rule'), (3, 'test-rule'), (4, 'test-rule');",
        )
        .unwrap();

        let effectiveness = compute_effectiveness(&conn, "test-rule").unwrap();
        // 3 successful (pos, neutral, pos) out of 4 total = 0.75
        assert!((effectiveness - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_compute_effectiveness_less_than_three_decisions() {
        let conn = setup_conn();
        conn.execute_batch(
            "INSERT INTO decisions (ts, task_completed, feedback) VALUES
             ('2026-03-14 09:00:00', 1, 'pos'),
             ('2026-03-14 10:00:00', 1, 'neutral');
             INSERT INTO decision_rules (decision_id, rule_id) VALUES
             (1, 'test-rule-2'), (2, 'test-rule-2');",
        )
        .unwrap();

        let effectiveness = compute_effectiveness(&conn, "test-rule-2").unwrap();
        assert!((effectiveness - -1.0).abs() < 1e-6);
    }

    #[test]
    fn test_query_rule_history_returns_events_for_rule() {
        let conn = setup_conn();
        conn.execute_batch(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES
             ('2026-03-14T09:00:00Z', 'rule_added', 'rule-a', 'seeded', 'txn-1'),
             ('2026-03-14T10:00:00Z', 'rule_promoted', 'rule-a', 'effective', 'txn-2'),
             ('2026-03-14T11:00:00Z', 'rule_added', 'rule-b', 'other', 'txn-3')",
        )
        .unwrap();

        let history = query_rule_history(&conn, "rule-a").unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].event_type, "rule_promoted");
        assert_eq!(history[0].txn_id, "txn-2");
        assert_eq!(history[0].reason, "effective");
        assert_eq!(history[1].event_type, "rule_added");
        assert_eq!(history[1].txn_id, "txn-1");
    }

    #[test]
    fn test_compute_tool_sequence_key() {
        let tools_detail = vec![
            ToolExecDetail {
                tool: "read_file".to_string(),
                success: true,
            },
            ToolExecDetail {
                tool: "write_file".to_string(),
                success: true,
            },
            ToolExecDetail {
                tool: "run_command".to_string(),
                success: false,
            },
            ToolExecDetail {
                tool: "http_request".to_string(),
                success: true,
            }, // Should be ignored
        ];
        let key = compute_tool_sequence_key(&tools_detail);
        assert_eq!(key, Some("read_file→write_file→run_command".to_string()));

        let empty_detail: Vec<ToolExecDetail> = vec![];
        let key = compute_tool_sequence_key(&empty_detail);
        assert_eq!(key, None);

        let single_detail = vec![ToolExecDetail {
            tool: "list_directory".to_string(),
            success: true,
        }];
        let key = compute_tool_sequence_key(&single_detail);
        assert_eq!(key, Some("list_directory".to_string()));
    }

    #[test]
    fn test_insert_decision_with_rules() {
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
    fn test_compute_core_metrics_for_date_avg_replans_and_user_correction_rate_extended() {
        let conn = setup_conn();
        conn.execute_batch(
            "INSERT INTO decisions (ts, total_tools, replans, task_completed, feedback)
             VALUES
             ('2026-03-15 09:00:00', 1, 0, 1, 'neutral'),
             ('2026-03-15 10:00:00', 2, 1, 1, 'neg'),
             ('2026-03-15 11:00:00', 1, 2, 0, 'pos'),
             ('2026-03-15 12:00:00', 3, 3, 1, 'neutral'),
             ('2026-03-15 13:00:00', 1, 0, 1, 'pos'),
             ('2026-03-15 14:00:00', 2, 1, 0, 'neg'),
             ('2026-03-15 15:00:00', 1, 0, 1, 'neutral')",
        )
        .unwrap();

        let metrics = compute_core_metrics_for_date(&conn, "2026-03-15").unwrap();
        // avg_replans: (0 + 1 + 2 + 3 + 0 + 1 + 0) / 7 = 7 / 7 = 1.0
        assert!((metrics.avg_replans - 1.0).abs() < 1e-6);
        // user_correction_rate: neg (2) / (pos (2) + neg (2)) = 2 / 4 = 0.5
        assert!((metrics.user_correction_rate - 0.5).abs() < 1e-6);
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
