//! Auto-rollback when metrics degrade.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::audit::log_evolution_event;
use crate::snapshots::restore_extended_snapshot;
use crate::Result;

// ─── Auto-rollback ───────────────────────────────────────────────────────────

/// Executes the rollback actions (restoring snapshot, logging).
fn execute_evolution_rollback(
    conn: &Connection,
    chat_root: &Path,
    skills_root: Option<&Path>,
    txn_id: &str,
    reason: &str,
) -> Result<()> {
    tracing::warn!("Evolution rollback executed: {} (txn={})", reason, txn_id);
    restore_extended_snapshot(chat_root, skills_root, txn_id)?;

    conn.execute(
        "UPDATE evolution_log SET type = type || '_rolled_back' WHERE version = ?1",
        params![txn_id],
    )?;

    log_evolution_event(
        conn,
        chat_root,
        "auto_rollback",
        txn_id,
        reason,
        &format!("rollback_{}", txn_id),
    )?;
    Ok(())
}
pub fn check_auto_rollback(
    conn: &Connection,
    chat_root: &Path,
    skills_root: Option<&Path>,
) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-5 days')
         ORDER BY date DESC LIMIT 4",
    )?;
    let metrics: Vec<(String, f64, f64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

    if metrics.len() < 3 {
        return Ok(false);
    }

    let fsr_declining = metrics.windows(2).take(3).all(|w| w[0].1 < w[1].1 - 0.10);
    let ucr_rising = metrics.windows(2).take(3).all(|w| w[0].2 > w[1].2 + 0.20);

    if fsr_declining || ucr_rising {
        let reason = if fsr_declining {
            "first_success_rate declined >10% for 3 consecutive days"
        } else {
            "user_correction_rate rose >20% for 3 consecutive days"
        };

        let last_txn: Option<String> = conn
            .query_row(
                "SELECT DISTINCT version FROM evolution_log
                 WHERE type NOT LIKE '%_rolled_back'
                 ORDER BY ts DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(txn_id) = last_txn {
            execute_evolution_rollback(conn, chat_root, skills_root, &txn_id, reason)?;
            return Ok(true);
        }
    }

    Ok(false)
}
