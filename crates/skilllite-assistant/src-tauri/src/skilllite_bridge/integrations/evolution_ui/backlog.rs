//! Evolution backlog rows and proposal status for the desktop UI (L2 CLI first).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionProposalStatusDto {
    pub proposal_id: String,
    pub status: String,
    pub acceptance_status: String,
    pub updated_at: String,
    pub note: Option<String>,
}

pub fn get_evolution_proposal_status(
    workspace: &str,
    proposal_id: &str,
    skilllite_path: Option<&Path>,
) -> Result<EvolutionProposalStatusDto, String> {
    if let Some(path) = skilllite_path {
        if let Ok(row) = spawn_skilllite_json(
            path,
            workspace,
            None,
            &[
                "evolution",
                "proposal-status",
                "--json",
                proposal_id,
            ],
        ) {
            return Ok(row);
        }
    }
    get_evolution_proposal_status_inprocess(proposal_id)
}

fn get_evolution_proposal_status_inprocess(
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    workspace: &str,
    limit: usize,
    skilllite_path: Option<&Path>,
) -> Result<Vec<EvolutionBacklogRowDto>, String> {
    if let Some(path) = skilllite_path {
        let capped = limit.clamp(1, 200);
        if let Ok(rows) = spawn_skilllite_json::<Vec<EvolutionBacklogRowDto>>(
            path,
            workspace,
            None,
            &[
                "evolution",
                "backlog",
                "--json",
                "--hide-closed",
                "--limit",
                &capped.to_string(),
                "--workspace",
                workspace,
            ],
        ) {
            return Ok(rows);
        }
    }
    load_evolution_backlog_inprocess(limit)
}

fn load_evolution_backlog_inprocess(limit: usize) -> Result<Vec<EvolutionBacklogRowDto>, String> {
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
