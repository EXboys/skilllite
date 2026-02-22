//! ExtensionRegistry: unified registry for agent tool extensions.
//!
//! Uses compile-time registration: add new tools by calling `register(tools())`.
//! Pattern: `registry.register(builtin::file_ops::tools());` — no changes to agent_loop.

use std::path::Path;

use super::builtin;
use super::memory;
use crate::llm::LlmClient;
use crate::prompt;
use crate::skills::{self, LoadedSkill};
use crate::types::{EventSink, ToolDefinition, ToolResult};
use skilllite_core::config::EmbeddingConfig;

/// Context for memory vector search (embedding API).
#[allow(dead_code)] // used when memory_vector feature is enabled
pub struct MemoryVectorContext<'a> {
    pub client: &'a LlmClient,
    pub embed_config: &'a EmbeddingConfig,
}

/// Unified registry for agent tool extensions.
///
/// Tool sources are registered at construction. Pattern:
/// ```ignore
/// let registry = ExtensionRegistry::builder(enable_memory, enable_memory_vector, skills)
///     .register(builtin::get_builtin_tool_definitions())
///     .register_memory_if(enable_memory)
///     .build();
/// ```
/// Adding a new tool module = add to builtin, or `.register(new_tools())`.
#[derive(Debug)]
pub struct ExtensionRegistry<'a> {
    /// Cached tool definitions (from registered extensions + skills).
    tool_definitions: Vec<ToolDefinition>,
    /// Whether memory tools are enabled.
    pub enable_memory: bool,
    /// Whether memory vector search is enabled.
    pub enable_memory_vector: bool,
    /// Loaded skills (for execution dispatch).
    pub skills: &'a [LoadedSkill],
}

/// Builder for ExtensionRegistry with explicit tool registration.
#[derive(Debug)]
pub struct ExtensionRegistryBuilder<'a> {
    tool_definitions: Vec<ToolDefinition>,
    enable_memory: bool,
    enable_memory_vector: bool,
    skills: &'a [LoadedSkill],
}

impl<'a> ExtensionRegistryBuilder<'a> {
    /// Create a new builder. Call `register()` for each tool provider, then `build()`.
    pub fn new(
        enable_memory: bool,
        enable_memory_vector: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self {
            tool_definitions: Vec::new(),
            enable_memory,
            enable_memory_vector,
            skills,
        }
    }

    /// Register tool definitions from an extension. Add one line per tool module.
    #[must_use]
    pub fn register(mut self, defs: impl IntoIterator<Item = ToolDefinition>) -> Self {
        self.tool_definitions.extend(defs);
        self
    }

    /// Register memory tools if enable_memory is true.
    #[must_use]
    pub fn register_memory_if(mut self, enable: bool) -> Self {
        if enable {
            self.tool_definitions
                .extend(memory::get_memory_tool_definitions());
        }
        self
    }

    /// Build the registry. Skills' tool definitions are added at build time.
    pub fn build(self) -> ExtensionRegistry<'a> {
        let mut tool_definitions = self.tool_definitions;
        for skill in self.skills {
            tool_definitions.extend(skill.tool_definitions.clone());
        }
        ExtensionRegistry {
            tool_definitions,
            enable_memory: self.enable_memory,
            enable_memory_vector: self.enable_memory_vector,
            skills: self.skills,
        }
    }
}

impl<'a> ExtensionRegistry<'a> {
    /// Create a registry with default tool registration (builtin + memory + skills).
    pub fn new(
        enable_memory: bool,
        enable_memory_vector: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self::builder(enable_memory, enable_memory_vector, skills)
            .register(builtin::get_builtin_tool_definitions())
            .register_memory_if(enable_memory)
            .build()
    }

    /// Start building a registry with explicit registration.
    pub fn builder(
        enable_memory: bool,
        enable_memory_vector: bool,
        skills: &'a [LoadedSkill],
    ) -> ExtensionRegistryBuilder<'a> {
        ExtensionRegistryBuilder::new(enable_memory, enable_memory_vector, skills)
    }

    /// Collect all tool definitions (from registered extensions + skills).
    pub fn all_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.tool_definitions.clone()
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
