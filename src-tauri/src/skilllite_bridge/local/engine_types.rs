//! JSON shapes from `skilllite … --json` (L2 contract types for the desktop bridge).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUiLine {
    pub source: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reveal_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUiSnapshot {
    pub python: RuntimeUiLine,
    pub node: RuntimeUiLine,
    pub cache_root: Option<String>,
    pub cache_root_abs: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRuntimeItem {
    pub requested: bool,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRuntimesResult {
    pub python: ProvisionRuntimeItem,
    pub node: ProvisionRuntimeItem,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSkill {
    pub name: String,
    pub description: String,
    pub path: String,
    pub txn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    pub task_id: String,
    pub response: String,
    pub task_completed: bool,
    pub tool_calls: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_skill: Option<NewSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrowthDueDiagnostics {
    pub min_run_gap_secs: u64,
    pub min_run_gap_blocked: bool,
    pub seconds_since_last_material_run: Option<i64>,
    pub weighted_signal_sum: i64,
    pub weighted_trigger_min: i64,
    pub signal_window: i64,
    pub raw_unprocessed_decisions: i64,
    pub raw_unprocessed_threshold: i64,
    pub weighted_arm_met: bool,
    pub raw_arm_met: bool,
    pub arm_signal: bool,
    pub sweep_interval_secs: u64,
    pub arm_sweep: bool,
    pub interval_secs: u64,
    pub periodic_anchor_unix: Option<i64>,
    pub periodic_elapsed_secs: i64,
    pub arm_periodic: bool,
    pub growth_tick_would_be_due: bool,
    pub periodic_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassiveScheduleDiagnostics {
    pub evolution_disabled: bool,
    pub daily_runs_today: i64,
    pub daily_cap: i64,
    pub daily_cap_blocked: bool,
    pub hours_since_last_material_run: Option<f64>,
    pub cooldown_hours: f64,
    pub cooldown_blocked: bool,
    pub passive_cooldown_uses_log_types: String,
    pub meaningful: i64,
    pub failures: i64,
    pub replans: i64,
    pub repeated_patterns: i64,
    pub recent_days: i64,
    pub recent_decision_sample_limit: i64,
    pub arm_prompts: bool,
    pub arm_memory: bool,
    pub arm_skills: bool,
    pub skills_skill_action: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuggestFollowupResponse {
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthorizeCapabilityResponse {
    pub proposal_id: String,
}
