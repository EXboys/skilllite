//! OpenAI-compatible chat types.

use serde::{Deserialize, Serialize};

use super::feedback::ExecutionFeedback;
use super::task::Task;

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
    /// Whether this result should count toward task failure / replan heuristics.
    pub counts_as_failure: bool,
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
    #[allow(dead_code)]
    pub messages: Vec<ChatMessage>,
    #[allow(dead_code)]
    pub tool_calls_count: usize,
    #[allow(dead_code)]
    pub iterations: usize,
    /// Task plan generated by the planner (empty if no planning was used).
    pub task_plan: Vec<Task>,
    /// Execution feedback for the evolution engine (EVO-1).
    pub feedback: ExecutionFeedback,
}

impl AgentResult {
    /// Convert to protocol-layer [`NodeResult`] for stdio_rpc/agent_chat/P2P.
    /// `task_id` is echoed back; use a generated UUID when the caller did not provide one.
    pub fn to_node_result(&self, task_id: impl Into<String>) -> skilllite_core::protocol::NodeResult {
        skilllite_core::protocol::NodeResult {
            task_id: task_id.into(),
            response: self.response.clone(),
            task_completed: self.feedback.task_completed,
            tool_calls: self.feedback.total_tools,
            new_skill: None, // agent_chat: N/A. Use `skilllite evolution run --json` for NewSkill output.
        }
    }
}
