//! Shared types for the agent module.
//!
//! Organized by domain:
//! - `string_utils`: UTF-8 safe string helpers
//! - `config`: Agent configuration
//! - `chat`: OpenAI-compatible chat types
//! - `feedback`: Execution feedback (EVO-1)
//! - `event_sink`: Event sink trait and implementations
//! - `task`: Task planning types
//! - `env_config`: Environment config helpers

mod chat;
mod config;
mod env_config;
mod event_sink;
mod feedback;
mod string_utils;
mod task;

// Re-export all public types for backward compatibility.
pub use chat::{
    parse_claude_tool_calls, AgentResult, ChatMessage, FunctionCall, FunctionDef, ToolCall,
    ToolDefinition, ToolFormat, ToolResult,
};
pub use config::AgentConfig;
pub use env_config::{
    get_chunk_size, get_compact_planning, get_compaction_keep_recent, get_compaction_threshold,
    get_extract_top_k, get_head_chunks, get_long_text_strategy, get_map_model,
    get_max_output_chars, get_max_tokens, get_memory_flush_enabled, get_memory_flush_threshold,
    get_output_dir, get_read_file_tool_result_max_chars, get_summarize_threshold,
    get_tail_chunks, get_tool_result_max_chars,
    get_tool_result_recovery_max_chars, get_user_input_max_chars, LongTextStrategy,
};
pub use event_sink::{
    ClarificationRequest, ClarificationResponse, EventSink, RunModeEventSink, SilentEventSink,
    TerminalEventSink,
};
pub use feedback::{
    classify_user_feedback, ExecutionFeedback, FeedbackSignal, SkillAction, TaskCompletionType,
    ToolExecDetail,
};
pub use string_utils::{chunk_str, safe_slice_from, safe_truncate};
pub use task::{PlanningRule, SourceEntry, SourceRegistry, Task};
