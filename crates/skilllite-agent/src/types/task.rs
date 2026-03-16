//! Task planning types.

use serde::{Deserialize, Serialize};

/// A task in the task plan.
/// Ported from Python `TaskPlanner.task_list` dict structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_hint: Option<String>,
    pub completed: bool,
}

// Re-export planning types from skilllite-core for backward compatibility.
pub use skilllite_core::planning::{PlanningRule, SourceEntry, SourceRegistry};
