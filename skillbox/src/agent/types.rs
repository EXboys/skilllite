//! Shared types for the agent module.

use serde::{Deserialize, Serialize};

/// Load `.env` file from current directory into environment variables.
/// Simple parser: supports `KEY=VALUE`, `# comments`, and quoted values.
/// Does NOT override existing env vars.
fn load_dotenv() {
    let path = std::env::current_dir()
        .map(|d| d.join(".env"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".env"));

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return, // .env not found, that's fine
    };

    for line in content.lines() {
        let line = line.trim();
        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Parse KEY=VALUE
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim();
            let mut value = line[eq_pos + 1..].trim();

            // Strip surrounding quotes
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = &value[1..value.len() - 1];
            }

            // Don't override existing env vars
            if key.is_empty() || std::env::var(key).is_ok() {
                continue;
            }

            std::env::set_var(key, value);
        }
    }
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
    /// Skills directories to load
    pub skill_dirs: Vec<String>,
    /// Enable task planning
    pub enable_task_planning: bool,
    /// Enable memory tools
    pub enable_memory: bool,
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
            enable_task_planning: false,
            enable_memory: false,
            verbose: false,
        }
    }
}

impl AgentConfig {
    /// Load from environment variables with sensible defaults.
    /// Also reads `.env` file from current directory if present.
    /// Supports both standard OpenAI env vars and project-specific ones:
    ///   OPENAI_API_BASE / BASE_URL, OPENAI_API_KEY / API_KEY, SKILLLITE_MODEL / MODEL
    pub fn from_env() -> Self {
        // Load .env file from current directory (if exists)
        load_dotenv();

        let api_base = std::env::var("OPENAI_API_BASE")
            .or_else(|_| std::env::var("OPENAI_BASE_URL"))
            .or_else(|_| std::env::var("BASE_URL"))
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let api_key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("API_KEY"))
            .unwrap_or_default();
        let model = std::env::var("SKILLLITE_MODEL")
            .or_else(|_| std::env::var("OPENAI_MODEL"))
            .or_else(|_| std::env::var("MODEL"))
            .unwrap_or_else(|_| "gpt-4o".to_string());
        let workspace = std::env::var("SKILLLITE_WORKSPACE")
            .unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .to_string_lossy()
                    .to_string()
            });

        Self {
            api_base,
            api_key,
            model,
            workspace,
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
    pub tool_name: String,
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    /// Convert to Claude API tool_result format.
    pub fn to_claude_format(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "tool_result",
            "tool_use_id": self.tool_call_id,
            "content": self.content,
            "is_error": self.is_error
        })
    }

    /// Convert to OpenAI API tool result message.
    pub fn to_openai_format(&self) -> serde_json::Value {
        serde_json::json!({
            "role": "tool",
            "tool_call_id": self.tool_call_id,
            "content": self.content
        })
    }

    /// Convert to the specified format.
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
    pub messages: Vec<ChatMessage>,
    pub tool_calls_count: usize,
    pub iterations: usize,
}

/// Event sink trait for different output targets (CLI, RPC, SDK).
pub trait EventSink: Send {
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

/// Simple terminal event sink for CLI chat.
pub struct TerminalEventSink {
    pub verbose: bool,
    /// Tracks whether text was streamed via `on_text_chunk` during the current
    /// LLM response. When true, `on_text` becomes a no-op to avoid duplicating
    /// already-displayed content. Reset when `on_text` is called.
    streamed_text: bool,
}

impl TerminalEventSink {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            streamed_text: false,
        }
    }
}

impl EventSink for TerminalEventSink {
    fn on_text(&mut self, text: &str) {
        if self.streamed_text {
            // Text was already displayed chunk-by-chunk via on_text_chunk.
            // The trailing newline was also added by accumulate_stream.
            // Just reset the flag for the next response.
            self.streamed_text = false;
            return;
        }
        // Non-streaming path: display full text + newline
        use std::io::Write;
        print!("{}", text);
        let _ = std::io::stdout().flush();
        println!();
    }

    fn on_text_chunk(&mut self, chunk: &str) {
        self.streamed_text = true;
        use std::io::Write;
        print!("{}", chunk);
        let _ = std::io::stdout().flush();
    }

    fn on_tool_call(&mut self, name: &str, arguments: &str) {
        if self.verbose {
            eprintln!("\n🔧 Tool: {} args={}", name, arguments);
        } else {
            eprintln!("\n🔧 {}", name);
        }
    }

    fn on_tool_result(&mut self, name: &str, result: &str, is_error: bool) {
        let prefix = if is_error { "❌" } else { "✅" };
        if self.verbose {
            let truncated = if result.len() > 500 {
                format!("{}...", &result[..500])
            } else {
                result.to_string()
            };
            eprintln!("  {} {}: {}", prefix, name, truncated);
        } else {
            // Always show a brief status so users can see if tool succeeded/failed
            let first_line = result.lines().next().unwrap_or("(empty)");
            let brief = if first_line.len() > 120 {
                format!("{}...", &first_line[..120])
            } else {
                first_line.to_string()
            };
            eprintln!("  {} {}", prefix, brief);
        }
    }

    fn on_confirmation_request(&mut self, prompt: &str) -> bool {
        use std::io::Write;
        eprint!("\n⚠️  {}\n确认执行? [y/N] ", prompt);
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
        eprintln!("\n📋 Task plan ({} tasks):", tasks.len());
        for task in tasks {
            let status = if task.completed { "✅" } else { "○" };
            let hint = task
                .tool_hint
                .as_deref()
                .map(|h| format!(" [{}]", h))
                .unwrap_or_default();
            eprintln!("   {}. {} {}{}", task.id, status, task.description, hint);
        }
    }

    fn on_task_progress(&mut self, task_id: u32, completed: bool) {
        if completed {
            eprintln!("  ✅ Task {} completed", task_id);
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

/// Helper: read an optional env var as String.
pub fn env_optional(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
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

/// Threshold above which chunked LLM summarization is used instead of simple
/// truncation. `SKILLLITE_SUMMARIZE_THRESHOLD`.
pub fn get_summarize_threshold() -> usize {
    env_usize("SKILLLITE_SUMMARIZE_THRESHOLD", 15000)
}

/// Max chars per tool result. `SKILLLITE_TOOL_RESULT_MAX_CHARS`.
pub fn get_tool_result_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_MAX_CHARS", 8000)
}

/// Max chars for tool messages during context-overflow recovery.
/// `SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS`.
pub fn get_tool_result_recovery_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS", 3000)
}

/// Output directory override. `SKILLLITE_OUTPUT_DIR`.
pub fn get_output_dir() -> Option<String> {
    env_optional("SKILLLITE_OUTPUT_DIR")
}

/// Path to external planning_rules.json. `SKILLLITE_PLANNING_RULES_PATH`.
pub fn get_planning_rules_path() -> Option<String> {
    env_optional("SKILLLITE_PLANNING_RULES_PATH")
}
