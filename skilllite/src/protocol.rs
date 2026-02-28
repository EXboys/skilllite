//! ProtocolHandler trait: extension point for entry-layer protocols.
//!
//! Implement this trait to add new protocols (e.g. gRPC, HTTP).
//! Registration: add a `Commands` variant in cli.rs and a match arm in lib.rs.

use anyhow::Result;

use crate::mcp;
use crate::stdio_rpc;

/// Parameters for protocol handlers (each protocol has its own variant).
#[derive(Debug)]
pub enum ProtocolParams {
    /// stdio JSON-RPC (`skilllite serve --stdio`)
    Stdio,
    /// MCP over stdio (`skilllite mcp`)
    Mcp { skills_dir: String },
    // Future: Grpc { addr: String }, Http { addr: String }, ...
}

/// Extension point for entry-layer protocols.
///
/// To add a new protocol (gRPC, HTTP):
/// 1. Add a variant to `ProtocolParams`
/// 2. Implement this trait for your handler
/// 3. Add a `Commands` variant in cli.rs
/// 4. Add a match arm in lib.rs: `Commands::X { .. } => XHandler.serve(params)?`
pub trait ProtocolHandler: Send + Sync {
    /// Protocol name for logging and diagnostics.
    #[allow(dead_code)] // required method in trait; callers use dynamic dispatch via serve()
    fn name(&self) -> &str;

    /// Start the protocol server. Blocks until shutdown.
    fn serve(&self, params: ProtocolParams) -> Result<()>;
}

/// Stdio JSON-RPC protocol handler.
pub struct StdioRpcHandler;

impl ProtocolHandler for StdioRpcHandler {
    fn name(&self) -> &str {
        "stdio-rpc"
    }

    fn serve(&self, params: ProtocolParams) -> Result<()> {
        match params {
            ProtocolParams::Stdio => stdio_rpc::serve_stdio(),
            _ => anyhow::bail!("StdioRpcHandler requires ProtocolParams::Stdio"),
        }
    }
}

/// MCP (Model Context Protocol) handler.
pub struct McpHandler;

impl ProtocolHandler for McpHandler {
    fn name(&self) -> &str {
        "mcp"
    }

    fn serve(&self, params: ProtocolParams) -> Result<()> {
        match params {
            ProtocolParams::Mcp { skills_dir } => mcp::serve_mcp_stdio(&skills_dir),
            _ => anyhow::bail!("McpHandler requires ProtocolParams::Mcp"),
        }
    }
}
