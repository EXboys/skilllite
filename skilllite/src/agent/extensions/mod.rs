//! Built-in extensions for the agent.
//!
//! All agent tools live under this directory for easy discovery:
//! - builtin: file ops, run_command, output, preview, chat (read_file, write_file, etc.)
//! - memory: memory_search, memory_write, memory_list (optional, enable_memory)

mod builtin;
mod memory;

pub use builtin::{
    execute_async_builtin_tool,
    execute_builtin_tool,
    get_builtin_tool_definitions,
    is_async_builtin_tool,
    is_builtin_tool,
    process_tool_result_content,
    process_tool_result_content_fallback,
};
pub use memory::{
    build_memory_context,
    execute_memory_tool,
    get_memory_tool_definitions,
    is_memory_tool,
};
