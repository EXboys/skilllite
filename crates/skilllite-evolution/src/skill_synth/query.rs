//! DB 查询：decisions 表中模式、执行记录等

use skilllite_core::config::env_keys::evolution as evo_keys;

use crate::Result;
use rusqlite::{params, Connection};

/// Returns `(recent_ts_condition, row_limit)` for skill-synth decision queries (env-tunable).
pub(super) fn recent_decisions_condition() -> (String, i64) {
    let days = std::env::var(evo_keys::SKILLLITE_EVO_SKILL_QUERY_RECENT_DAYS)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7)
        .clamp(1, 90);
    let limit = std::env::var(evo_keys::SKILLLITE_EVO_SKILL_QUERY_DECISION_LIMIT)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
        .clamp(1, 500);
    (format!("ts >= datetime('now', '-{} days')", days), limit)
}

fn skill_failure_sample_limit() -> i64 {
    std::env::var(evo_keys::SKILLLITE_EVO_SKILL_FAILURE_SAMPLE_LIMIT)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5)
        .clamp(1, 50)
}

/// Query patterns that repeat but have LOW success rate (for failure-driven skill generation).
pub(super) fn query_failed_patterns(conn: &Connection, min_count: u32) -> Result<String> {
    let (recent_cond, recent_limit) = recent_decisions_condition();
    let min_i64 = min_count as i64;
    let mut stmt = conn.prepare(&format!(
        "SELECT task_description, COUNT(*) as cnt,
                SUM(CASE WHEN task_completed = 1 THEN 1 ELSE 0 END) as successes
         FROM decisions
         WHERE {} AND task_description IS NOT NULL
         GROUP BY task_description
         HAVING cnt >= ?1 AND CAST(successes AS REAL) / cnt < 0.5
         ORDER BY cnt DESC LIMIT {}",
        recent_cond, recent_limit
    ))?;

    let rows: Vec<String> = stmt
        .query_map(params![min_i64], |row| {
            let desc: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let succ: i64 = row.get(2)?;
            Ok(format!(
                "- 模式: {} | 出现: {}次 | 成功: {}次 ({:.0}%)",
                desc,
                cnt,
                succ,
                if cnt > 0 {
                    succ as f64 / cnt as f64 * 100.0
                } else {
                    0.0
                }
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

/// Query recent failed executions (for failure-driven prompt).
pub(super) fn query_failed_executions(conn: &Connection) -> Result<String> {
    let (recent_cond, recent_limit) = recent_decisions_condition();
    let mut stmt = conn.prepare(&format!(
        "SELECT task_description, tools_detail, feedback
         FROM decisions
         WHERE {} AND (task_completed = 0 OR failed_tools > 0) AND task_description IS NOT NULL
         ORDER BY ts DESC LIMIT {}",
        recent_cond, recent_limit
    ))?;

    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let desc: Option<String> = row.get(0)?;
            let tools: Option<String> = row.get(1)?;
            let fb: Option<String> = row.get(2)?;
            Ok(format!(
                "- 任务: {} | 工具: {} | 反馈: {}",
                desc.unwrap_or_default(),
                tools.unwrap_or_default(),
                fb.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

/// Recent failed execution row ids (same filter as `query_failed_executions`).
pub(super) fn query_failed_execution_ids(conn: &Connection) -> Result<Vec<i64>> {
    let (recent_cond, recent_limit) = recent_decisions_condition();
    let sql = format!(
        "SELECT id FROM decisions
         WHERE {recent_cond} AND (task_completed = 0 OR failed_tools > 0) AND task_description IS NOT NULL
         ORDER BY ts DESC LIMIT {recent_limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| row.get::<_, i64>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Successful execution row ids for the given task descriptions (aligned with `query_pattern_executions`).
pub(super) fn query_pattern_execution_ids(
    conn: &Connection,
    task_descriptions: &[String],
) -> Result<Vec<i64>> {
    if task_descriptions.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = task_descriptions
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let (recent_cond, recent_limit) = recent_decisions_condition();
    let sql = format!(
        "SELECT id FROM decisions
         WHERE {recent_cond} AND task_completed = 1 AND task_description IN ({placeholders})
         ORDER BY ts DESC LIMIT {recent_limit}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(task_descriptions.iter()),
        |row| row.get::<_, i64>(0),
    )?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Returns `(display_string, task_desc_list)`.
/// `display_string` is the human-readable summary sent to the LLM prompt.
/// `task_desc_list` is the raw task descriptions used to filter matching executions.
pub(super) fn query_repeated_patterns(
    conn: &Connection,
    min_count: u32,
) -> Result<(String, Vec<String>)> {
    let (recent_cond, recent_limit) = recent_decisions_condition();
    let min_i64 = min_count as i64;
    let mut stmt = conn.prepare(&format!(
        "SELECT task_description, COUNT(*) as cnt,
                SUM(CASE WHEN task_completed = 1 THEN 1 ELSE 0 END) as successes
         FROM decisions
         WHERE {} AND task_description IS NOT NULL
         GROUP BY task_description
         HAVING cnt >= ?1 AND CAST(successes AS REAL) / cnt >= 0.8
         ORDER BY cnt DESC LIMIT {}",
        recent_cond, recent_limit
    ))?;

    let mut display_rows: Vec<String> = Vec::new();
    let mut task_descs: Vec<String> = Vec::new();

    for row in stmt
        .query_map(params![min_i64], |row| {
            let desc: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let succ: i64 = row.get(2)?;
            Ok((desc, cnt, succ))
        })?
        .filter_map(|r| r.ok())
    {
        let (desc, cnt, succ) = row;
        display_rows.push(format!(
            "- 模式: {} | 出现: {}次 | 成功: {}次 ({:.0}%)",
            desc,
            cnt,
            succ,
            succ as f64 / cnt as f64 * 100.0
        ));
        task_descs.push(desc);
    }

    Ok((display_rows.join("\n"), task_descs))
}

/// Query successful executions that belong to the given pattern descriptions.
pub(super) fn query_pattern_executions(
    conn: &Connection,
    task_descriptions: &[String],
) -> Result<String> {
    if task_descriptions.is_empty() {
        return Ok(String::new());
    }

    let placeholders = task_descriptions
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let (recent_cond, recent_limit) = recent_decisions_condition();

    let sql = format!(
        "SELECT task_description, tools_detail, elapsed_ms
         FROM decisions
         WHERE {} AND task_completed = 1 AND task_description IN ({})
         ORDER BY ts DESC LIMIT {}",
        recent_cond, placeholders, recent_limit
    );

    let mut stmt = conn.prepare(&sql)?;

    let rows: Vec<String> = stmt
        .query_map(
            rusqlite::params_from_iter(task_descriptions.iter()),
            |row| {
                let desc: String = row.get(0)?;
                let tools: Option<String> = row.get(1)?;
                let elapsed: i64 = row.get(2)?;
                Ok(format!(
                    "- 任务: {} | 工具: {} | 耗时: {}ms",
                    desc,
                    tools.unwrap_or_else(|| "N/A".to_string()),
                    elapsed
                ))
            },
        )?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

pub(super) fn query_skill_failures(conn: &Connection, skill_name: &str) -> Result<String> {
    let tool_pattern = format!("%{}%", skill_name);
    let lim = skill_failure_sample_limit();
    let sql = format!(
        "SELECT task_description, tools_detail, feedback
         FROM decisions
         WHERE failed_tools > 0 AND tools_detail LIKE ?1
         ORDER BY ts DESC LIMIT {}",
        lim
    );
    let mut stmt = conn.prepare(&sql)?;

    let rows: Vec<String> = stmt
        .query_map(params![tool_pattern], |row| {
            let desc: Option<String> = row.get(0)?;
            let tools: Option<String> = row.get(1)?;
            let fb: Option<String> = row.get(2)?;
            Ok(format!(
                "- 任务: {} | 工具详情: {} | 反馈: {}",
                desc.unwrap_or_default(),
                tools.unwrap_or_default(),
                fb.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}
