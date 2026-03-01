//! Standard Node I/O types for protocol handlers and P2P routing.
//!
//! These types are the shared "currency" across stdio_rpc, agent_chat, MCP,
//! and the future P2P layer. They intentionally carry only what a remote peer
//! (or routing layer) needs — not full agent internals.

use serde::{Deserialize, Serialize};

/// An evolved skill produced during task execution.
///
/// Emitted in [`NodeResult::new_skill`] when the Evolution Engine synthesises
/// or refines a skill as a side-effect of completing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewSkill {
    /// Skill name — matches the `name` field in `SKILL.md`.
    pub name: String,
    /// Human-readable description of what the skill does.
    pub description: String,
    /// Local filesystem path where the skill was installed.
    pub path: String,
    /// Evolution transaction ID — used for rollback via `skilllite evolution reset`.
    pub txn_id: String,
}

/// Standard result unit — the universal output for local execution and P2P routing.
///
/// Maps from `AgentResult` internally; fields are intentionally minimal so
/// that routing layers and remote peers can parse results without knowing agent internals.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResult {
    /// Echoed task ID (matches caller's task id when available; otherwise a generated UUID).
    pub task_id: String,
    /// Agent's final response text.
    pub response: String,
    /// Whether the agent marked the task as completed.
    pub task_completed: bool,
    /// Total tool calls made during execution.
    pub tool_calls: usize,
    /// Newly synthesised skill, if the Evolution Engine produced one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_skill: Option<NewSkill>,
}
