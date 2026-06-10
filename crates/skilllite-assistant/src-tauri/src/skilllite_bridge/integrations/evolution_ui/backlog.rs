//! Evolution backlog rows and proposal status (L2 CLI only).

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
    skilllite_path: &Path,
) -> Result<EvolutionProposalStatusDto, String> {
    spawn_skilllite_json(
        skilllite_path,
        workspace,
        None,
        &[
            "evolution",
            "proposal-status",
            "--json",
            "--workspace",
            workspace,
            proposal_id,
        ],
    )
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
    skilllite_path: &Path,
) -> Result<Vec<EvolutionBacklogRowDto>, String> {
    let capped = limit.clamp(1, 200);
    spawn_skilllite_json(
        skilllite_path,
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
    )
}
