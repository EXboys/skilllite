//! Agent module: LLM-powered tool loop, built-in tools, skills invocation, CLI chat, RPC.
//!
//! This is Layer 3 of the three-layer composable architecture:
//!   Sandbox (Layer 1) → Executor (Layer 2) → Agent (Layer 3)
//!
//! Only compiled when the `agent` feature is enabled.
//! The `rpc` submodule provides the agent_chat JSON-Lines event stream protocol
//! for Python/TypeScript SDKs.

pub mod llm;
pub mod tools;
pub mod skills;
pub mod prompt;
pub mod agent_loop;
pub mod chat_session;
pub mod types;
pub mod task_planner;
pub mod long_text;
pub mod memory;
pub mod rpc;
