//! ExtensionRegistry: unified registry for agent tool extensions.
//!
//! Consolidates built-in tools, memory tools, and skills into a single interface
//! for tool discovery and execution. Supports future extensibility (e.g. third-party
//! extensions via register()).

use std::path::Path;

use super::builtin;
use super::memory;
use crate::agent::llm::LlmClient;
use crate::agent::prompt;
use crate::agent::skills::{self, LoadedSkill};
use crate::agent::types::{EventSink, ToolDefinition, ToolResult};
use crate::config::EmbeddingConfig;

/// Context for memory vector search (embedding API).
#[allow(dead_code)] // used when memory_vector feature is enabled
pub struct MemoryVectorContext<'a> {
    pub client: &'a LlmClient,
    pub embed_config: &'a EmbeddingConfig,
}

/// Unified registry for agent tool extensions.
///
/// Currently supports two extension sources:
/// - **Built-in extensions**: file ops, run_command, output, preview, chat (read_file, write_file, etc.)
/// - **Memory extensions**: memory_search, memory_write, memory_list (when enable_memory)
/// - **Skills**: dynamically loaded from skill directories
///
/// Future: `register(extension)` for third-party plugins.
#[derive(Debug)]
pub struct ExtensionRegistry<'a> {
    /// Whether memory tools (memory_search, memory_write, memory_list) are enabled.
    pub enable_memory: bool,
    /// Whether memory vector search is enabled (requires memory_vector feature + embedding API).
    pub enable_memory_vector: bool,
    /// Loaded skills providing tool definitions and execution.
    pub skills: &'a [LoadedSkill],
}

impl<'a> ExtensionRegistry<'a> {
    /// Create a registry with the given configuration.
    pub fn new(
        enable_memory: bool,
        enable_memory_vector: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self {
            enable_memory,
            enable_memory_vector,
            skills,
        }
    }

    /// Collect all tool definitions from built-in, memory (if enabled), and skills.
    pub fn all_tool_definitions(&self) -> Vec<ToolDefinition> {
        let mut tools = builtin::get_builtin_tool_definitions();
        if self.enable_memory {
            tools.extend(memory::get_memory_tool_definitions());
        }
        for skill in self.skills {
            tools.extend(skill.tool_definitions.clone());
        }
        tools
    }

    /// Check if any extension owns this tool name.
    #[allow(dead_code)]
    pub fn owns_tool(&self, name: &str) -> bool {
        builtin::is_builtin_tool(name)
            || (self.enable_memory && memory::is_memory_tool(name))
            || skills::find_skill_by_tool_name(self.skills, name).is_some()
            || skills::find_skill_by_name(self.skills, name).is_some()
    }

    /// Execute a tool by name. Dispatches to the appropriate extension.
    /// `embed_ctx` is required for memory vector search when enable_memory_vector is true.
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: &str,
        workspace: &Path,
        event_sink: &mut dyn EventSink,
        embed_ctx: Option<&MemoryVectorContext<'_>>,
    ) -> ToolResult {
        if builtin::is_builtin_tool(tool_name) {
            if builtin::is_async_builtin_tool(tool_name) {
                builtin::execute_async_builtin_tool(tool_name, arguments, workspace, event_sink).await
            } else {
                builtin::execute_builtin_tool(tool_name, arguments, workspace)
            }
        } else if self.enable_memory && memory::is_memory_tool(tool_name) {
            memory::execute_memory_tool(
                tool_name,
                arguments,
                workspace,
                "default",
                self.enable_memory_vector,
                embed_ctx,
            )
            .await
        } else if let Some(skill) = skills::find_skill_by_tool_name(self.skills, tool_name) {
            skills::execute_skill(skill, tool_name, arguments, workspace, event_sink)
        } else if let Some(skill) = skills::find_skill_by_name(self.skills, tool_name) {
            // Reference-only skill (no entry_point / no scripts, just SKILL.md guidance)
            let docs = prompt::get_skill_full_docs(skill).unwrap_or_else(|| {
                format!(
                    "Skill '{}' is reference-only (no executable entry point). Use its guidance to generate content yourself using write_output.",
                    skill.name
                )
            });
            ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!(
                    "Note: '{}' is a reference-only skill (no executable script). Its documentation is provided below — use these guidelines to generate the content yourself, then save with write_output and preview with preview_server.\n\n{}",
                    skill.name, docs
                ),
                is_error: false,
            }
        } else {
            ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Unknown tool: {}", tool_name),
                is_error: true,
            }
        }
    }
}
