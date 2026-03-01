//! Shared planning types used by agent and evolution.
//!
//! PlanningRule, SourceEntry, SourceRegistry are used for task planning
//! and evolution (prompt learning, external knowledge).

use serde::{Deserialize, Serialize};

fn default_priority() -> u32 {
    50
}

fn default_origin() -> String {
    "seed".to_string()
}

fn default_source_quality() -> f32 {
    0.70
}

fn default_source_accessibility() -> f32 {
    0.80
}

fn default_enabled() -> bool {
    true
}

/// A planning rule for task generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningRule {
    pub id: String,
    #[serde(default = "default_priority")]
    pub priority: u32,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub context_keywords: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_hint: Option<String>,
    pub instruction: String,
    #[serde(default)]
    pub mutable: bool,
    #[serde(default = "default_origin")]
    pub origin: String,
    #[serde(default)]
    pub reusable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effectiveness: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_count: Option<u32>,
}

/// A single external information source entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    pub source_type: String,
    pub parser: String,
    pub region: String,
    pub language: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default = "default_source_quality")]
    pub quality_score: f32,
    #[serde(default = "default_source_accessibility")]
    pub accessibility_score: f32,
    #[serde(default)]
    pub rules_contributed: u32,
    #[serde(default)]
    pub fetch_success_count: u32,
    #[serde(default)]
    pub fetch_fail_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_fetched: Option<String>,
    #[serde(default)]
    pub mutable: bool,
    #[serde(default = "default_origin")]
    pub origin: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// The full source registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRegistry {
    pub version: u32,
    pub sources: Vec<SourceEntry>,
}
