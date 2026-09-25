//! Evolution status for the assistant UI (`skilllite evolution status --json` only).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge::chat::ChatConfigOverrides;
use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::local::engine_types::{GrowthDueDiagnostics, PassiveScheduleDiagnostics};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionLogEntryDto {
    pub ts: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub target_id: Option<String>,
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionStatusPayload {
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
    pub recent_events: Vec<EvolutionLogEntryDto>,
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

pub fn load_evolution_status(
    workspace: &str,
    cfg: Option<ChatConfigOverrides>,
    periodic_anchor_unix: Option<i64>,
    skilllite_path: &Path,
) -> Result<EvolutionStatusPayload, String> {
    let mut args = vec!["evolution", "status", "--json", "--workspace", workspace];
    let anchor_buf;
    if let Some(anchor) = periodic_anchor_unix {
        anchor_buf = anchor.to_string();
        args.push("--periodic-anchor-unix");
        args.push(&anchor_buf);
    }
    spawn_skilllite_json(skilllite_path, workspace, cfg.as_ref(), &args)
}
