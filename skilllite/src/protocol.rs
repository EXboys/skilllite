//! ProtocolHandler trait: unified Entry Layer for all transport protocols.
//!
//! ## Architecture
//!
//! This module is the **Universal Entry Layer** — every external-facing transport
//! (stdio JSON-RPC, MCP, Agent-RPC, future P2P) is registered here as a
//! `ProtocolHandler` implementation. The CLI (`lib.rs`) routes commands to handlers;
//! the handlers call into the core/agent/sandbox layers.
//!
//! ## Standard Node I/O Types
//!
//! `NodeTask` / `NodeContext` (input) and `NodeResult` / `NewSkill` (output) are the
//! shared "currency" that all handlers and the future P2P routing layer understand.
//! They are transport-agnostic: stdio_rpc serialises them as JSON-RPC fields,
//! agent-rpc maps them to JSON-Lines events, P2P broadcasts them as Gossip messages.
//!
//! ## Adding a New Protocol
//!
//! 1. Add a variant to [`ProtocolParams`].
//! 2. Implement [`ProtocolHandler`] for your handler struct.
//! 3. Add a `Commands` variant in `cli.rs`.
//! 4. Add a match arm in `lib.rs`: `Commands::X { .. } => XHandler.serve(params)?`.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::mcp;
use crate::stdio_rpc;

// Re-export protocol types from skilllite-core for Entry Layer handlers.
pub use skilllite_core::protocol::{NewSkill, NodeResult};

// ─── Standard Node I/O Types (input side) ────────────────────────────────────
//
// NodeContext and NodeTask are input types; NodeResult/NewSkill are in skilllite-core.

/// Execution context attached to every [`NodeTask`].
///
/// Provides the agent with workspace identity, session continuity, and the
/// capability tags the caller intends to use.  Remote P2P peers use
/// `required_capabilities` to decide whether to accept the task.
// Not yet wired to a caller — defined here for the future P2P routing layer.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeContext {
    /// Workspace path (local execution) or originating node ID (P2P).
    pub workspace: String,
    /// Session key for memory/transcript continuity (matches `ChatSession` key).
    pub session_key: String,
    /// Capability tags the caller expects to exercise (e.g. `["python", "web"]`).
    /// Populated from `SKILL.md` `metadata.capabilities` at the call site.
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

/// Standard task unit — the universal input for local execution and P2P routing.
///
/// `description` is a natural-language goal; `context` carries the identity and
/// capability requirements.  The P2P Discovery layer matches `required_capabilities`
/// against peer capability registries to select the best node.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTask {
    /// Unique task identifier (UUIDv4 or monotonic counter string).
    pub id: String,
    /// Natural-language description of what the agent should accomplish.
    pub description: String,
    /// Execution context (workspace, session, capabilities).
    pub context: NodeContext,
    /// Optional hint for which skill or tool to prefer (e.g. `"web-scraper"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_hint: Option<String>,
}

// ─── Protocol Parameters ─────────────────────────────────────────────────────

/// Parameters for protocol handlers (one variant per supported transport).
// P2p variant is a placeholder — P2pHandler is not yet implemented.
#[allow(dead_code)]
#[derive(Debug)]
pub enum ProtocolParams {
    /// stdio JSON-RPC 2.0 (`skilllite serve --stdio`).
    /// One request → one response.  Used by Python/TS SDKs for skill execution.
    Stdio,

    /// MCP (Model Context Protocol) over stdio (`skilllite mcp`).
    /// JSON-RPC 2.0 dialect used by Cursor / VSCode.
    Mcp { skills_dir: String },

    /// Agent Chat streaming RPC over stdio (`skilllite agent-rpc`).
    /// JSON-Lines event stream: one request → many events (text_chunk, done, …).
    /// Used by the Tauri desktop assistant and Python/TS SDK streaming clients.
    AgentRpc,

    /// P2P mesh node (`skilllite p2p --listen <ADDR>`).
    ///
    /// **Placeholder** — `P2pHandler` is not yet implemented.
    /// When implemented, this variant drives the mDNS discovery loop and
    /// the Gossip-based `NewSkill` sync protocol.
    P2p {
        /// mDNS / Libp2p listen address (e.g. `"0.0.0.0:7700"`).
        listen_addr: String,
        /// Capability tags advertised to peers.
        /// Populated from installed skills' `SKILL.md` `metadata.capabilities`.
        capability_tags: Vec<String>,
    },
}

// ─── ProtocolHandler Trait ───────────────────────────────────────────────────

/// Extension point for Entry Layer protocols.
///
/// Every transport that exposes SkillLite functionality externally implements
/// this trait.  The CLI dispatches `Commands` variants to the matching handler.
///
/// `serve` is synchronous and blocks until the server shuts down.  Async
/// handlers (e.g. a future P2P listener) should create a `tokio::Runtime`
/// internally rather than making the trait `async`, keeping the call site
/// simple and avoiding `async-trait` overhead.
pub trait ProtocolHandler: Send + Sync {
    /// Protocol name used in log output and diagnostics.
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Start the protocol server.  Blocks until shutdown.
    fn serve(&self, params: ProtocolParams) -> Result<()>;
}

// ─── Handler Implementations ─────────────────────────────────────────────────

/// Stdio JSON-RPC 2.0 handler (`skilllite serve --stdio`).
pub struct StdioRpcHandler;

impl ProtocolHandler for StdioRpcHandler {
    fn name(&self) -> &str { "stdio-rpc" }

    fn serve(&self, params: ProtocolParams) -> Result<()> {
        match params {
            ProtocolParams::Stdio => stdio_rpc::serve_stdio(),
            _ => anyhow::bail!("StdioRpcHandler requires ProtocolParams::Stdio"),
        }
    }
}

/// MCP (Model Context Protocol) handler (`skilllite mcp`).
pub struct McpHandler;

impl ProtocolHandler for McpHandler {
    fn name(&self) -> &str { "mcp" }

    fn serve(&self, params: ProtocolParams) -> Result<()> {
        match params {
            ProtocolParams::Mcp { skills_dir } => mcp::serve_mcp_stdio(&skills_dir),
            _ => anyhow::bail!("McpHandler requires ProtocolParams::Mcp"),
        }
    }
}

/// Agent Chat streaming RPC handler (`skilllite agent-rpc`).
///
/// Reads JSON-Lines requests from stdin and streams JSON-Lines events to stdout.
/// Previously invoked directly; now routed through `ProtocolHandler` for
/// consistency with all other Entry Layer transports.
#[cfg(feature = "agent")]
pub struct AgentRpcHandler;

#[cfg(feature = "agent")]
impl ProtocolHandler for AgentRpcHandler {
    fn name(&self) -> &str { "agent-rpc" }

    fn serve(&self, params: ProtocolParams) -> Result<()> {
        match params {
            ProtocolParams::AgentRpc => skilllite_agent::rpc::serve_agent_rpc(),
            _ => anyhow::bail!("AgentRpcHandler requires ProtocolParams::AgentRpc"),
        }
    }
}
