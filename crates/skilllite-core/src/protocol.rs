//! Standard Node I/O types for protocol handlers and P2P routing.
//!
//! These types are the shared "currency" across stdio_rpc, agent_chat, MCP,
//! and the future P2P layer. They intentionally carry only what a remote peer
//! (or routing layer) needs — not full agent internals.

use serde::{Deserialize, Serialize};

// ─── Input types (NodeTask, NodeContext) ─────────────────────────────────────

/// Execution context attached to every [`NodeTask`].
///
/// Provides the agent with workspace identity, session continuity, and the
/// capability tags the caller intends to use. Remote P2P peers use
/// `required_capabilities` to decide whether to accept the task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeContext {
    /// Workspace path (local execution) or originating node ID (P2P).
    pub workspace: String,
    /// Session key for memory/transcript continuity (matches `ChatSession` key).
    pub session_key: String,
    /// Capability tags the caller expects to exercise (e.g. `["python", "web"]`).
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

/// Standard task unit — the universal input for local execution and P2P routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTask {
    /// Unique task identifier (UUIDv4 or monotonic counter string).
    pub id: String,
    /// Natural-language description of what the agent should accomplish.
    pub description: String,
    /// Execution context (workspace, session, capabilities).
    pub context: NodeContext,
    /// Optional hint for which skill or tool to prefer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_hint: Option<String>,
}

// ─── Output types (NodeResult, NewSkill) ─────────────────────────────────────

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
