//! Evolution feedback collection and evaluation system (EVO-1).
//!
//! Manages four SQLite tables in `memory/default.sqlite`:
//!   - `decisions`: per-task execution records (flow data)
//!   - `decision_rules`: decision–rule mapping (N:M, precise JOIN queries)
//!   - `evolution_log`: evolution product records (for EGL stats + rollback)
//!   - `evolution_metrics`: daily system-level trend metrics
//!
//! Also exports a human-readable `DECISIONS.md` from SQLite data.

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::types::{ExecutionFeedback, FeedbackSignal};

// ─── Schema ─────────────────────────────────────────────────────────────────

/// Ensure all evolution tables exist (idempotent, called on session start).
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
            tools_detail TEXT
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
    Ok(())
}

// ─── Decision recording ─────────────────────────────────────────────────────

/// Insert a decision record after each task execution. <1ms with WAL mode.
pub fn insert_decision(
    conn: &Connection,
    session_id: Option<&str>,
    feedback: &ExecutionFeedback,
    user_feedback: FeedbackSignal,
) -> Result<i64> {
    let tools_detail_json = serde_json::to_string(&feedback.tools_detail).unwrap_or_default();

    conn.execute(
        "INSERT INTO decisions (session_id, total_tools, failed_tools, replans,
         elapsed_ms, task_completed, feedback, task_description, tools_detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
        ],
    )?;
    let decision_id = conn.last_insert_rowid();

    // Insert associated rules
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

/// Update feedback signal for the most recent decision in a session.
/// Called when we classify the user's next message.
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

/// Compute effectiveness for a specific rule via JOIN (precise, no LIKE).
/// Returns -1.0 if insufficient samples (<3).
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

/// Aggregate tool effectiveness (replaces tool_effectiveness.json).
pub fn query_tool_effectiveness(conn: &Connection) -> Result<Vec<ToolEffectivenessRow>> {
    let mut stmt = conn.prepare(
        "SELECT
            json_extract(value, '$.tool') as tool_name,
            COUNT(*) as total,
            SUM(CASE WHEN json_extract(value, '$.success') THEN 1 ELSE 0 END) as successes
         FROM decisions, json_each(decisions.tools_detail)
         WHERE ts > datetime('now', '-30 days')
         GROUP BY tool_name
         ORDER BY total DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ToolEffectivenessRow {
                tool: row.get(0)?,
                total: row.get(1)?,
                successes: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate task patterns (replaces task_patterns.json).
pub fn query_task_patterns(conn: &Connection) -> Result<Vec<TaskPatternRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_description, COUNT(*) as freq,
                AVG(CASE WHEN task_completed THEN 1.0 ELSE 0.0 END) as success_rate
         FROM decisions
         WHERE task_description IS NOT NULL AND ts > datetime('now', '-30 days')
         GROUP BY task_description ORDER BY freq DESC LIMIT 20",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TaskPatternRow {
                description: row.get(0)?,
                frequency: row.get(1)?,
                success_rate: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate failure lessons (replaces error_lessons.json).
pub fn query_failure_lessons(conn: &Connection) -> Result<Vec<FailureLessonRow>> {
    let mut stmt = conn.prepare(
        "SELECT task_description, feedback, COUNT(*) as freq
         FROM decisions
         WHERE failed_tools > 0 AND ts > datetime('now', '-30 days')
         GROUP BY task_description ORDER BY freq DESC LIMIT 20",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(FailureLessonRow {
                description: row.get(0)?,
                feedback: row.get(1)?,
                frequency: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ─── System-level metrics ───────────────────────────────────────────────────

/// Compute and upsert daily system-level metrics for today.
pub fn update_daily_metrics(conn: &Connection) -> Result<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let stats: Result<(f64, f64, f64, f64), _> = conn.query_row(
        "SELECT
            AVG(CASE WHEN replans = 0 AND task_completed = 1 THEN 1.0 ELSE 0.0 END),
            AVG(CAST(replans AS REAL)),
            AVG(CAST(total_tools AS REAL)),
            CASE WHEN COUNT(CASE WHEN feedback IN ('pos','neg') THEN 1 END) > 0
                 THEN CAST(COUNT(CASE WHEN feedback = 'neg' THEN 1 END) AS REAL)
                      / COUNT(CASE WHEN feedback IN ('pos','neg') THEN 1 END)
                 ELSE 0.0 END
         FROM decisions
         WHERE date(ts) = ?1 AND total_tools >= 2",
        params![today],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );

    let (fsr, avg_r, avg_tc, ucr) = stats.unwrap_or((0.0, 0.0, 0.0, 0.0));
    let egl = compute_egl(conn, &today).unwrap_or(0.0);

    conn.execute(
        "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans,
         avg_tool_calls, user_correction_rate, egl)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(date) DO UPDATE SET
            first_success_rate = ?2, avg_replans = ?3,
            avg_tool_calls = ?4, user_correction_rate = ?5, egl = ?6",
        params![today, fsr, avg_r, avg_tc, ucr, egl],
    )?;

    Ok(())
}

/// EGL (Evolutionary Generality Loss) — convergence metric (Yunjue Agent inspired).
/// EGL = new_evolution_products / total_triggered_tasks * 1000.
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

// ─── EVO-5: Time trends (user profile extension) ────────────────────────────

/// Hourly activity distribution (0-23).
#[derive(Debug, serde::Serialize)]
pub struct HourlyActivity {
    pub hour: i32,
    pub count: i64,
    pub success_rate: f64,
}

/// Weekday activity distribution (0=Sunday, 6=Saturday).
#[derive(Debug, serde::Serialize)]
pub struct WeekdayActivity {
    pub weekday: i32,
    pub weekday_name: &'static str,
    pub count: i64,
    pub success_rate: f64,
    pub dominant_task: Option<String>,
}

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// Aggregate decisions by hour of day (last 30 days).
pub fn query_hourly_activity(conn: &Connection) -> Result<Vec<HourlyActivity>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', ts) AS INTEGER) as hour,
                COUNT(*) as cnt,
                AVG(CASE WHEN task_completed = 1 THEN 1.0 ELSE 0.0 END) as sr
         FROM decisions
         WHERE ts > datetime('now', '-30 days') AND total_tools >= 2
         GROUP BY hour ORDER BY hour",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(HourlyActivity {
                hour: row.get(0)?,
                count: row.get(1)?,
                success_rate: row.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Aggregate decisions by weekday (last 30 days), with dominant task type.
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

    // Fill dominant task for each weekday
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

/// Peak activity hours (top 3 most active hours).
pub fn query_peak_hours(conn: &Connection) -> Result<Vec<(i32, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', ts) AS INTEGER) as hour, COUNT(*) as cnt
         FROM decisions
         WHERE ts > datetime('now', '-30 days') AND total_tools >= 2
         GROUP BY hour ORDER BY cnt DESC LIMIT 3",
    )?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ─── Archival ───────────────────────────────────────────────────────────────

/// Archive old decisions to prevent unbounded growth.
/// Keeps the most recent `keep` records, CASCADE deletes decision_rules.
pub fn archive_old_decisions(conn: &Connection, keep: i64) -> Result<usize> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))?;
    if count <= keep {
        return Ok(0);
    }
    let deleted = conn.execute(
        "DELETE FROM decisions WHERE id NOT IN
         (SELECT id FROM decisions ORDER BY ts DESC LIMIT ?1)",
        params![keep],
    )?;
    Ok(deleted)
}

// ─── DECISIONS.md export ────────────────────────────────────────────────────

/// Export recent evolution decisions to a human-readable Markdown file.
pub fn export_decisions_md(conn: &Connection, output_path: &Path) -> Result<()> {
    let mut md = String::from(
        "# SkillLite 进化决策记录\n\n\
         > 自动维护。每次进化事件追加一行。\n\
         > 用户可手动编辑：删除不想要的进化、添加备注。\n\n\
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
        let date = &ts[..10]; // YYYY-MM-DD
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

    // System metrics summary
    md.push_str("\n## 系统指标趋势 (最近7天)\n\n");
    md.push_str("| 日期 | 首次成功率 | 平均replan | 用户纠正率 | EGL |\n");
    md.push_str("|------|-----------|-----------|-----------|-----|\n");

    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, avg_replans, user_correction_rate, egl
         FROM evolution_metrics
         WHERE date > date('now', '-7 days') ORDER BY date DESC",
    )?;
    let metrics = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    for m in metrics {
        let (date, fsr, avg_r, ucr, egl) = m?;
        md.push_str(&format!(
            "| {} | {:.0}% | {:.1} | {:.0}% | {:.1} |\n",
            date,
            fsr * 100.0,
            avg_r,
            ucr * 100.0,
            egl
        ));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, md)?;
    Ok(())
}

// ─── Data types ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct ToolEffectivenessRow {
    pub tool: String,
    pub total: i64,
    pub successes: i64,
}

#[derive(Debug)]
pub struct TaskPatternRow {
    pub description: String,
    pub frequency: i64,
    pub success_rate: f64,
}

#[derive(Debug)]
pub struct FailureLessonRow {
    pub description: Option<String>,
    pub feedback: String,
    pub frequency: i64,
}

// ─── EVO-5: High-level query helpers for CLI ────────────────────────────────

/// Evolution history for a specific target_id.
#[derive(Debug)]
pub struct EvolutionHistoryEntry {
    pub ts: String,
    pub event_type: String,
    pub reason: String,
    pub txn_id: String,
}

/// Query evolution history for a given target_id.
pub fn query_rule_history(conn: &Connection, target_id: &str) -> Result<Vec<EvolutionHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT ts, type, reason, version FROM evolution_log
         WHERE target_id = ?1 ORDER BY ts DESC LIMIT 10",
    )?;
    let rows = stmt
        .query_map(params![target_id], |row| {
            Ok(EvolutionHistoryEntry {
                ts: row.get(0)?,
                event_type: row.get(1)?,
                reason: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                txn_id: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ─── Open evolution DB ──────────────────────────────────────────────────────

/// Open (or create) the evolution SQLite database, ensuring all tables exist.
/// Reuses the memory/default.sqlite path.
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
    use crate::types::ToolExecDetail;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_query_decision() {
        let conn = test_db();
        let feedback = ExecutionFeedback {
            total_tools: 5,
            failed_tools: 1,
            replans: 0,
            iterations: 3,
            elapsed_ms: 1200,
            context_overflow_retries: 0,
            task_completed: true,
            task_description: Some("debug a Rust project".into()),
            rules_used: vec!["rule_grep_first".into()],
            tools_detail: vec![
                ToolExecDetail { tool: "read_file".into(), success: true },
                ToolExecDetail { tool: "run_command".into(), success: false },
            ],
        };

        let id = insert_decision(&conn, Some("test-session"), &feedback, FeedbackSignal::Neutral).unwrap();
        assert!(id > 0);

        // Verify decision_rules association
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM decision_rules WHERE decision_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_effectiveness_insufficient_samples() {
        let conn = test_db();
        let eff = compute_effectiveness(&conn, "nonexistent_rule").unwrap();
        assert_eq!(eff, -1.0);
    }

    #[test]
    fn test_archive_old_decisions() {
        let conn = test_db();
        let feedback = ExecutionFeedback::default();
        for _ in 0..15 {
            insert_decision(&conn, Some("s"), &feedback, FeedbackSignal::Neutral).unwrap();
        }
        let deleted = archive_old_decisions(&conn, 10).unwrap();
        assert_eq!(deleted, 5);
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 10);
    }

    #[test]
    fn test_update_feedback() {
        let conn = test_db();
        let feedback = ExecutionFeedback::default();
        insert_decision(&conn, Some("s1"), &feedback, FeedbackSignal::Neutral).unwrap();
        update_last_decision_feedback(&conn, "s1", FeedbackSignal::ExplicitPositive).unwrap();

        let fb: String = conn
            .query_row(
                "SELECT feedback FROM decisions WHERE session_id = 's1' ORDER BY ts DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fb, "pos");
    }

    // ─── EVO-5 tests ────────────────────────────────────────────────────────

    #[test]
    fn test_time_trends_empty() {
        let conn = test_db();
        let hourly = query_hourly_activity(&conn).unwrap();
        assert!(hourly.is_empty());
        let weekday = query_weekday_activity(&conn).unwrap();
        assert!(weekday.is_empty());
        let peaks = query_peak_hours(&conn).unwrap();
        assert!(peaks.is_empty());
    }

    #[test]
    fn test_time_trends_with_data() {
        let conn = test_db();
        let feedback = ExecutionFeedback {
            total_tools: 5,
            failed_tools: 0,
            task_completed: true,
            ..Default::default()
        };
        for _ in 0..5 {
            insert_decision(&conn, Some("s"), &feedback, FeedbackSignal::Neutral).unwrap();
        }
        let hourly = query_hourly_activity(&conn).unwrap();
        assert!(!hourly.is_empty(), "should have activity for current hour");
        assert_eq!(hourly[0].count, 5);

        let peaks = query_peak_hours(&conn).unwrap();
        assert!(!peaks.is_empty());
    }

    #[test]
    fn test_rule_history_empty() {
        let conn = test_db();
        let history = query_rule_history(&conn, "nonexistent").unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn test_rule_history_with_data() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version)
             VALUES (datetime('now'), 'rule_added', 'test_rule', 'test reason', 'evo_001')",
            [],
        ).unwrap();
        let history = query_rule_history(&conn, "test_rule").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event_type, "rule_added");
        assert_eq!(history[0].reason, "test reason");
    }
}
