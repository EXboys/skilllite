//! SQLite + file evolution audit and decision marking.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::scope::EvolutionScope;
use crate::Result;

// ─── Audit log ───────────────────────────────────────────────────────────────

pub fn log_evolution_event(
    conn: &Connection,
    chat_root: &Path,
    event_type: &str,
    target_id: &str,
    reason: &str,
    txn_id: &str,
) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ts, event_type, target_id, reason, txn_id],
    )?;

    let log_path = chat_root.join("evolution.log");
    let entry = serde_json::json!({
        "ts": ts,
        "type": event_type,
        "id": target_id,
        "reason": reason,
        "txn_id": txn_id,
    });
    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    file.write_all(line.as_bytes())?;

    skilllite_core::observability::audit_evolution_event(event_type, target_id, reason, txn_id);

    Ok(())
}

// ─── Mark decisions evolved ───────────────────────────────────────────────────

pub fn mark_decisions_evolved(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "UPDATE decisions SET evolved = 1 WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    stmt.execute(param_refs.as_slice())?;
    Ok(())
}

/// Decision ids to mark `evolved=1` after a run: only rows actually read by enabled learners,
/// intersected with `scope.decision_ids` (avoids marking the entire recent window).
pub fn decision_ids_to_mark_after_run(
    conn: &Connection,
    scope: &EvolutionScope,
    force: bool,
) -> Result<Vec<i64>> {
    let allowed: HashSet<i64> = scope.decision_ids.iter().copied().collect();
    if allowed.is_empty() {
        return Ok(Vec::new());
    }
    let mut acc: HashSet<i64> = HashSet::new();
    if scope.prompts {
        for id in crate::prompt_learner::decision_ids_read_for_prompt_evolution(conn)? {
            if allowed.contains(&id) {
                acc.insert(id);
            }
        }
    }
    if scope.memory {
        for id in crate::memory_learner::decision_ids_read_for_memory_evolution(conn)? {
            if allowed.contains(&id) {
                acc.insert(id);
            }
        }
    }
    if scope.skills {
        let try_generate = scope.skill_action.should_run_skill_generation_paths();
        for id in
            crate::skill_synth::decision_ids_read_for_skill_evolution(conn, try_generate, force)?
        {
            if allowed.contains(&id) {
                acc.insert(id);
            }
        }
    }
    let mut v: Vec<i64> = acc.into_iter().collect();
    v.sort_unstable();
    Ok(v)
}

#[cfg(test)]
mod decision_mark_tests {
    use super::*;
    use crate::config::SkillAction;
    use crate::feedback;
    use rusqlite::Connection;

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        feedback::ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn marks_only_learner_read_ids_intersected_with_scope() {
        let conn = open_mem();
        for i in 1..=12i64 {
            conn.execute(
                "INSERT INTO decisions (evolved, total_tools, failed_tools, replans, task_completed, task_description, ts)
                 VALUES (0, 2, 0, 0, 1, ?1, datetime('now', ?2))",
                rusqlite::params![format!("task-{i}"), format!("-{i} minutes")],
            )
            .unwrap();
        }
        let ids: Vec<i64> = conn
            .prepare("SELECT id FROM decisions ORDER BY id ASC")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(ids.len(), 12);

        let scope = EvolutionScope {
            skills: false,
            skill_action: SkillAction::None,
            memory: false,
            prompts: true,
            decision_ids: ids.clone(),
        };
        let to_mark = decision_ids_to_mark_after_run(&conn, &scope, false).unwrap();
        assert!(
            !to_mark.is_empty() && to_mark.len() < ids.len(),
            "expected strict subset of scope ids, got len {} (total ids {})",
            to_mark.len(),
            ids.len()
        );
        for id in &to_mark {
            assert!(ids.contains(id));
        }
        mark_decisions_evolved(&conn, &to_mark).unwrap();
        let evolved_cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM decisions WHERE evolved = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(evolved_cnt, to_mark.len() as i64);
    }
}
