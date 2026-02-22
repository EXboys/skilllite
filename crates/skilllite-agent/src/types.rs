//! Shared types for the agent module.

use serde::{Deserialize, Serialize};

// ─── UTF-8 safe string helpers ──────────────────────────────────────────────

/// Truncate a string at a safe UTF-8 char boundary (from the start).
/// Returns a &str of at most `max_bytes` bytes, never splitting a multi-byte character.
pub fn safe_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Get a &str starting from approximately `start_pos`, adjusted forward to a safe UTF-8 boundary.
pub fn safe_slice_from(s: &str, start_pos: usize) -> &str {
    if start_pos >= s.len() {
        return "";
    }
    let mut start = start_pos;
    while start < s.len() && !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

/// Split a string into chunks of approximately `chunk_size` bytes,
/// ensuring each split occurs at a valid UTF-8 char boundary.
pub fn chunk_str(s: &str, chunk_size: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < s.len() {
        let target_end = (start + chunk_size).min(s.len());
        let mut safe_end = target_end;
        while safe_end > start && !s.is_char_boundary(safe_end) {
            safe_end -= 1;
        }
        if safe_end == start && start < s.len() {
            safe_end = start + 1;
            while safe_end < s.len() && !s.is_char_boundary(safe_end) {
                safe_end += 1;
            }
        }
        chunks.push(&s[start..safe_end]);
        start = safe_end;
    }
    chunks
}

/// Agent configuration.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// OpenAI-compatible API base URL (e.g. "https://api.openai.com/v1")
    pub api_base: String,
    /// API key
    pub api_key: String,
    /// Model name (e.g. "gpt-4o", "claude-3-5-sonnet-20241022")
    pub model: String,
    /// Maximum iterations for the agent loop
    pub max_iterations: usize,
    /// Maximum tool calls per task
    pub max_tool_calls_per_task: usize,
    /// Workspace root path
    pub workspace: String,
    /// System prompt override (optional)
    pub system_prompt: Option<String>,
    /// Temperature (0.0 - 2.0)
    pub temperature: Option<f64>,
    /// Skills directories to load (reserved for multi-dir support)
    #[allow(dead_code)]
    pub skill_dirs: Vec<String>,
    /// Enable task planning
    pub enable_task_planning: bool,
    /// Enable memory tools
    pub enable_memory: bool,
    /// Enable memory vector search (requires memory_vector feature + embedding API)
    pub enable_memory_vector: bool,
    /// Verbose output
    pub verbose: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            api_base: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            max_iterations: 50,
            max_tool_calls_per_task: 15,
            workspace: std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
                .to_string_lossy()
                .to_string(),
            system_prompt: None,
            temperature: None,
            skill_dirs: Vec::new(),
            enable_task_planning: true,
            enable_memory: true,
            enable_memory_vector: false,
            verbose: false,
        }
    }
}

impl AgentConfig {
    /// Load from environment variables with sensible defaults.
    /// Also reads `.env` file from current directory if present.
    /// Uses unified config layer: SKILLLITE_* with fallback to OPENAI_* / BASE_URL / API_KEY / MODEL.
    pub fn from_env() -> Self {
        skilllite_core::config::load_dotenv();
        let llm = skilllite_core::config::LlmConfig::from_env();
        let paths = skilllite_core::config::PathsConfig::from_env();
        let flags = skilllite_core::config::AgentFeatureFlags::from_env();
        Self {
            api_base: llm.api_base,
            api_key: llm.api_key,
            model: llm.model,
            workspace: paths.workspace,
            enable_memory: flags.enable_memory,
            enable_memory_vector: flags.enable_memory_vector,
            enable_task_planning: flags.enable_task_planning,
            ..Default::default()
        }
    }
}

// ─── OpenAI-compatible chat types ───────────────────────────────────────────

/// A chat message in OpenAI format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: &str) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant_with_tool_calls(content: Option<&str>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.map(|s| s.to_string()),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn tool_result(tool_call_id: &str, content: &str) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(content.to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.to_string()),
            name: None,
        }
    }
}

/// A tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Supported LLM tool formats.
/// Ported from Python `core/tools.py` ToolFormat enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolFormat {
    /// OpenAI function calling format (GPT-4, DeepSeek, Qwen, etc.)
    OpenAI,
    /// Claude native tool format (Anthropic SDK)
    Claude,
}

/// OpenAI-compatible tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDef,
}

impl ToolDefinition {
    /// Convert to Claude API format.
    /// Claude expects: { name, description, input_schema }
    pub fn to_claude_format(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.function.name,
            "description": self.function.description,
            "input_schema": self.function.parameters
        })
    }

    /// Convert to the specified format.
    #[allow(dead_code)]
    pub fn to_format(&self, format: &ToolFormat) -> serde_json::Value {
        match format {
            ToolFormat::OpenAI => serde_json::to_value(self).unwrap_or_default(),
            ToolFormat::Claude => self.to_claude_format(),
        }
    }
}

/// Function definition within a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Result from executing a tool.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_call_id: String,
    #[allow(dead_code)]
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    /// Convert to Claude API tool_result format.
    #[allow(dead_code)]
    pub fn to_claude_format(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_result",
            "tool_use_id": self.tool_call_id,
            "content": self.content,
            "is_error": self.is_error
        })
    }

    /// Convert to OpenAI API tool result message.
    #[allow(dead_code)]
    pub fn to_openai_format(&self) -> serde_json::Value {
        serde_json::json!({
            "role": "tool",
            "tool_call_id": self.tool_call_id,
            "content": self.content
        })
    }

    /// Convert to the specified format.
    #[allow(dead_code)]
    pub fn to_format(&self, format: &ToolFormat) -> serde_json::Value {
        match format {
            ToolFormat::OpenAI => self.to_openai_format(),
            ToolFormat::Claude => self.to_claude_format(),
        }
    }
}

/// Parse tool calls from a Claude native API response.
/// Claude returns content blocks with type "tool_use".
/// Ported from Python `ToolUseRequest.parse_from_claude_response`.
pub fn parse_claude_tool_calls(content_blocks: &[serde_json::Value]) -> Vec<ToolCall> {
    let mut calls = Vec::new();
    for block in content_blocks {
        if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
            let id = block
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = block
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = block
                .get("input")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            let arguments = serde_json::to_string(&input).unwrap_or_else(|_| "{}".to_string());

            calls.push(ToolCall {
                id,
                call_type: "function".to_string(),
                function: FunctionCall { name, arguments },
            });
        }
    }
    calls
}

/// Agent loop result.
#[derive(Debug)]
pub struct AgentResult {
    pub response: String,
    #[allow(dead_code)]
    pub messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    pub tool_calls_count: usize,
    #[allow(dead_code)]
    pub iterations: usize,
    /// Task plan generated by the planner (empty if no planning was used).
    pub task_plan: Vec<Task>,
}

/// Event sink trait for different output targets (CLI, RPC, SDK).
pub trait EventSink: Send {
    /// Called at the start of each conversation turn (before any other events).
    fn on_turn_start(&mut self) {}
    /// Called when the assistant produces text content.
    fn on_text(&mut self, text: &str);
    /// Called when a tool is about to be invoked.
    fn on_tool_call(&mut self, name: &str, arguments: &str);
    /// Called when a tool returns a result.
    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool);
    /// Called when the agent needs user confirmation (L3 security).
    /// Returns true if the user approves.
    fn on_confirmation_request(&mut self, prompt: &str) -> bool;
    /// Called for streaming text chunks.
    #[allow(dead_code)]
    fn on_text_chunk(&mut self, _chunk: &str) {}
    /// Called when a task plan is generated. (Phase 2)
    fn on_task_plan(&mut self, _tasks: &[Task]) {}
    /// Called when a task's status changes. (Phase 2)
    fn on_task_progress(&mut self, _task_id: u32, _completed: bool) {}
}

/// Separator for CLI section headers.
const SECTION_SEP: &str = "──────────────────────────────────────";

/// Simple terminal event sink for CLI chat.
pub struct TerminalEventSink {
    pub verbose: bool,
    streamed_text: bool,
    /// Whether we've shown the "执行" section header this turn.
    execution_section_shown: bool,
    /// Whether we've shown the "结果" section header this turn.
    result_section_shown: bool,
}

impl TerminalEventSink {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            streamed_text: false,
            execution_section_shown: false,
            result_section_shown: false,
        }
    }

    #[inline]
    fn msg(&self, s: &str) {
        eprintln!("{}", s);
    }

    #[inline]
    fn msg_opt(&self, s: &str) {
        if !s.is_empty() {
            for line in s.lines() {
                eprintln!("{}", line);
            }
        }
    }

    fn show_execution_section(&mut self) {
        if !self.execution_section_shown {
            self.execution_section_shown = true;
            self.msg(&format!("─── 🔧 执行 ─── {}", SECTION_SEP));
        }
    }

    fn show_result_section(&mut self) {
        if !self.result_section_shown {
            self.result_section_shown = true;
            self.msg(&format!("─── 📄 结果 ─── {}", SECTION_SEP));
            self.msg("");
        }
    }
}

impl EventSink for TerminalEventSink {
    fn on_turn_start(&mut self) {
        self.execution_section_shown = false;
        self.result_section_shown = false;
    }

    fn on_text(&mut self, text: &str) {
        if self.streamed_text {
            // Text was already displayed chunk-by-chunk via on_text_chunk.
            // The trailing newline was also added by accumulate_stream.
            // Just reset the flag for the next response.
            self.streamed_text = false;
            return;
        }
        // Non-streaming path: display full text + newline
        // Only show result section when we have actual content (avoids empty "结果" between plan and execution)
        if !text.trim().is_empty() {
            self.show_result_section();
        }
        use std::io::Write;
        print!("{}", text);
        let _ = std::io::stdout().flush();
        println!();
    }

    fn on_text_chunk(&mut self, chunk: &str) {
        self.streamed_text = true;
        // Only show result section when we have actual content (avoids empty "结果" between plan and execution)
        if !chunk.trim().is_empty() {
            self.show_result_section();
        }
        use std::io::Write;
        print!("{}", chunk);
        let _ = std::io::stdout().flush();
    }

    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        self.show_execution_section();
        if self.verbose {
            // Truncate long JSON args for display
            let args_display = if arguments.len() > 200 {
                format!("{}…", safe_truncate(arguments, 200))
            } else {
                arguments.to_string()
            };
            self.msg(&format!("🔧 Tool: {}  args={}", name, args_display));
        } else {
            self.msg(&format!("🔧 {}", name));
        }
    }

    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        let icon = if is_error { "❌" } else { "✅" };
        if self.verbose {
            let brief = if result.len() > 400 {
                format!("{}…", safe_truncate(result, 400))
            } else {
                result.to_string()
            };
            self.msg(&format!("  {} {}: {}", icon, name, brief));
        } else {
            let first = result.lines().next().unwrap_or("(ok)");
            let brief = if first.len() > 80 {
                format!("{}…", safe_truncate(first, 80))
            } else {
                first.to_string()
            };
            self.msg(&format!("  {} {} {}", icon, name, brief));
        }
    }

    fn on_confirmation_request(&mut self, prompt: &str) -> bool {
        use std::io::Write;
        self.msg_opt(prompt);
        eprint!("确认执行? [y/N] ");
        let _ = std::io::stderr().flush();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let trimmed = input.trim().to_lowercase();
            trimmed == "y" || trimmed == "yes"
        } else {
            false
        }
    }

    fn on_task_plan(&mut self, tasks: &[Task]) {
        self.msg(&format!("─── 📋 计划 ─── {}", SECTION_SEP));
        self.msg(&format!("Task plan ({} tasks):", tasks.len()));
        for task in tasks {
            let status = if task.completed { "✅" } else { "○" };
            let hint = task
                .tool_hint
                .as_deref()
                .map(|h| format!(" [{}]", h))
                .unwrap_or_default();
            self.msg(&format!("   {}. {} {}{}", task.id, status, task.description, hint));
        }
    }

    fn on_task_progress(&mut self, task_id: u32, completed: bool) {
        if completed {
            self.msg(&format!("  ✅ Task {} completed", task_id));
        }
    }
}

// ─── Phase 2: Task planning types ──────────────────────────────────────────

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

/// A planning rule for task generation.
/// Ported from Python `config/planning_rules.json` schema.
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
}

fn default_priority() -> u32 {
    50
}

// ─── Phase 2: Environment config helpers ───────────────────────────────────
//
// Ported from Python `config/env_config.py`.
// Centralised env-var parsing for long-text summarization, tool result limits, etc.

/// Helper: read an env var as usize with fallback.
fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Chunk size for long text summarization (~1.5k tokens). `SKILLLITE_CHUNK_SIZE`.
pub fn get_chunk_size() -> usize {
    env_usize("SKILLLITE_CHUNK_SIZE", 6000)
}

/// Number of head chunks for head+tail summarization. `SKILLLITE_HEAD_CHUNKS`.
pub fn get_head_chunks() -> usize {
    env_usize("SKILLLITE_HEAD_CHUNKS", 3)
}

/// Number of tail chunks for head+tail summarization. `SKILLLITE_TAIL_CHUNKS`.
pub fn get_tail_chunks() -> usize {
    env_usize("SKILLLITE_TAIL_CHUNKS", 3)
}

/// Max output length for summarization (~2k tokens). `SKILLLITE_MAX_OUTPUT_CHARS`.
pub fn get_max_output_chars() -> usize {
    env_usize("SKILLLITE_MAX_OUTPUT_CHARS", 8000)
}

/// Model for Map stage in MapReduce summarization. `SKILLLITE_MAP_MODEL`.
/// When set, Map (per-chunk summarization) uses this cheaper model; Reduce (merge) uses main model.
/// E.g. `qwen-plus`, `gemini-1.5-flash`. If unset, both stages use main model.
pub fn get_map_model(main_model: &str) -> String {
    skilllite_core::config::loader::env_optional("SKILLLITE_MAP_MODEL", &[])
        .unwrap_or_else(|| main_model.to_string())
}

/// Long text selection strategy. `SKILLLITE_LONG_TEXT_STRATEGY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongTextStrategy {
    /// Head + tail only (existing behavior).
    HeadTailOnly,
    /// Score all chunks (Position + Discourse + Entity), take top-K.
    HeadTailExtract,
    /// Map all chunks (no filtering), Reduce merge. Best with SKILLLITE_MAP_MODEL.
    MapReduceFull,
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn get_long_text_strategy() -> LongTextStrategy {
    let v = env_str("SKILLLITE_LONG_TEXT_STRATEGY", "head_tail_only")
        .to_lowercase()
        .trim()
        .to_string();
    match v.as_str() {
        "head_tail_extract" | "extract" => LongTextStrategy::HeadTailExtract,
        "mapreduce_full" | "mapreduce" | "map_reduce" => LongTextStrategy::MapReduceFull,
        _ => LongTextStrategy::HeadTailOnly,
    }
}

/// Number of chunks to select in extract mode. Uses ratio or head+tail count as floor.
pub fn get_extract_top_k(
    total_chunks: usize,
    head_chunks: usize,
    tail_chunks: usize,
) -> usize {
    let ratio = std::env::var("SKILLLITE_EXTRACT_TOP_K_RATIO")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.5);
    let by_ratio = (total_chunks as f64 * ratio).ceil() as usize;
    let floor = head_chunks + tail_chunks;
    by_ratio.max(floor).min(total_chunks)
}

/// Threshold above which chunked LLM summarization is used instead of simple
/// truncation. `SKILLLITE_SUMMARIZE_THRESHOLD`.
/// Default raised from 15000→30000 to avoid summarizing medium-sized HTML/code
/// files (e.g. 17KB website) which destroys content needed for downstream tasks.
pub fn get_summarize_threshold() -> usize {
    env_usize("SKILLLITE_SUMMARIZE_THRESHOLD", 30000)
}

/// Max output tokens for LLM completion. `SKILLLITE_MAX_TOKENS`.
/// Higher values reduce write_output/write_file truncation when generating large content.
/// Default 8192 to match common API limits (e.g. DeepSeek). Set higher if your API supports it.
pub fn get_max_tokens() -> usize {
    env_usize("SKILLLITE_MAX_TOKENS", 8192)
}

/// Max chars per tool result. `SKILLLITE_TOOL_RESULT_MAX_CHARS`.
/// Default raised from 8000→12000 to better accommodate HTML/code tool results
/// without triggering unnecessary truncation.
pub fn get_tool_result_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_MAX_CHARS", 12000)
}

/// Max chars for tool messages during context-overflow recovery.
/// `SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS`.
pub fn get_tool_result_recovery_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS", 3000)
}

/// Output directory override. `SKILLLITE_OUTPUT_DIR`.
pub fn get_output_dir() -> Option<String> {
    skilllite_core::config::PathsConfig::from_env().output_dir
}

/// Compaction threshold: compact conversation history when message count exceeds this.
/// `SKILLLITE_COMPACTION_THRESHOLD`. Default 16 (~8 turns).
pub fn get_compaction_threshold() -> usize {
    env_usize("SKILLLITE_COMPACTION_THRESHOLD", 16)
}

/// Number of recent messages to keep after compaction. `SKILLLITE_COMPACTION_KEEP_RECENT`.
pub fn get_compaction_keep_recent() -> usize {
    env_usize("SKILLLITE_COMPACTION_KEEP_RECENT", 10)
}

/// Whether to use compact planning prompt (rule filtering + fewer examples).
/// - If SKILLLITE_COMPACT_PLANNING is set: use that (1=compact, 0=full).
/// - If not set: only latest/best models (claude, gpt-4, gpt-5, gemini-2) use compact; deepseek, qwen, 7b, ollama etc. get full.
pub fn get_compact_planning(model: Option<&str>) -> bool {
    if let Some(v) = skilllite_core::config::loader::env_optional(
        skilllite_core::config::env_keys::misc::SKILLLITE_COMPACT_PLANNING,
        &[],
    ) {
        return !matches!(v.to_lowercase().as_str(), "0" | "false" | "no" | "off");
    }
    // Auto: only top-tier models use compact; others (deepseek, qwen, 7b, ollama) get full prompt
    let model = match model {
        Some(m) => m.to_lowercase(),
        None => return false, // unknown model → full
    };
    let compact_models = [
        "claude-4.6",
        "gpt-4.5",
        "gpt-5",
        "gemini-2.5",
        "gemini-3.0",
    ];
    compact_models.iter().any(|p| model.starts_with(p) || model.contains(p))
}
