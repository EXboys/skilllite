//! Agent tool extensions and unified registry.
//!
//! Extension sources:
//! - **builtin**: file ops, run_command, output, preview, chat (read_file, write_file, etc.)
//! - **memory**: memory_search, memory_write, memory_list (optional, enable_memory)
//! - **skills**: dynamically loaded from skill directories
//!
//! `ExtensionRegistry` provides a unified interface for tool discovery and execution.

mod builtin;
mod memory;
mod registry;

pub use builtin::{
    process_read_file_tool_result_content, process_tool_result_content,
    process_tool_result_content_fallback,
};
pub use memory::{build_memory_context, index_evolution_knowledge};
pub use registry::{
    CapabilityPolicy, ExtensionRegistry, ExtensionRegistryBuilder, MemoryVectorContext,
    PlanningControlExecutor, RegisteredTool, ToolAvailabilityView, ToolCapability,
    ToolExecutionProfile, ToolHandler, ToolScope,
};
