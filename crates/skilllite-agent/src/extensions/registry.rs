//! ExtensionRegistry: unified registry for agent tool extensions.
//!
//! Uses compile-time registration: add new tools by calling `register(tools())`.
//! Pattern: `registry.register(builtin::file_ops::tools());` — no changes to agent_loop.

use std::collections::{HashMap, HashSet};
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

/// Coarse-grained capabilities used to gate tools in different execution modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCapability {
    FilesystemWrite,
    MemoryWrite,
    ProcessExec,
    Preview,
    Delegation,
    SkillExecution,
}

/// Policy that decides which capabilities are allowed in the current mode.
#[derive(Debug, Clone, Copy)]
pub struct CapabilityPolicy {
    allow_filesystem_write: bool,
    allow_memory_write: bool,
    allow_process_exec: bool,
    allow_preview: bool,
    allow_delegation: bool,
    allow_skill_execution: bool,
}

impl Default for CapabilityPolicy {
    fn default() -> Self {
        Self::full_access()
    }
}

impl CapabilityPolicy {
    /// Allow the complete built-in tool surface.
    pub const fn full_access() -> Self {
        Self {
            allow_filesystem_write: true,
            allow_memory_write: true,
            allow_process_exec: true,
            allow_preview: true,
            allow_delegation: true,
            allow_skill_execution: true,
        }
    }

    /// Restrict to inspection-oriented tools only.
    pub const fn read_only() -> Self {
        Self {
            allow_filesystem_write: false,
            allow_memory_write: false,
            allow_process_exec: false,
            allow_preview: false,
            allow_delegation: false,
            allow_skill_execution: false,
        }
    }

    #[must_use]
    pub fn with_filesystem_write(mut self, allow: bool) -> Self {
        self.allow_filesystem_write = allow;
        self
    }

    #[must_use]
    pub fn with_memory_write(mut self, allow: bool) -> Self {
        self.allow_memory_write = allow;
        self
    }

    #[must_use]
    pub fn with_process_exec(mut self, allow: bool) -> Self {
        self.allow_process_exec = allow;
        self
    }

    #[must_use]
    pub fn with_preview(mut self, allow: bool) -> Self {
        self.allow_preview = allow;
        self
    }

    #[must_use]
    pub fn with_delegation(mut self, allow: bool) -> Self {
        self.allow_delegation = allow;
        self
    }

    #[must_use]
    pub fn with_skill_execution(mut self, allow: bool) -> Self {
        self.allow_skill_execution = allow;
        self
    }

    pub fn allows(&self, capabilities: &[ToolCapability]) -> bool {
        capabilities.iter().all(|capability| match capability {
            ToolCapability::FilesystemWrite => self.allow_filesystem_write,
            ToolCapability::MemoryWrite => self.allow_memory_write,
            ToolCapability::ProcessExec => self.allow_process_exec,
            ToolCapability::Preview => self.allow_preview,
            ToolCapability::Delegation => self.allow_delegation,
            ToolCapability::SkillExecution => self.allow_skill_execution,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{CapabilityPolicy, ExtensionRegistry};

    #[test]
    fn read_only_policy_filters_mutating_tools() {
        let registry = ExtensionRegistry::read_only(true, false, &[]);

        assert!(registry.owns_tool("read_file"));
        assert!(registry.owns_tool("memory_search"));
        assert!(registry.owns_tool("complete_task"));
        assert!(!registry.owns_tool("write_file"));
        assert!(!registry.owns_tool("memory_write"));
        assert!(!registry.owns_tool("run_command"));
        assert!(!registry.owns_tool("preview_server"));
    }

    #[test]
    fn full_registry_keeps_mutating_tools() {
        let registry = ExtensionRegistry::new(true, false, &[]);

        assert!(registry.owns_tool("write_file"));
        assert!(registry.owns_tool("memory_write"));
        assert!(registry.owns_tool("run_command"));
        assert!(registry.owns_tool("preview_server"));
    }

    #[test]
    fn custom_policy_can_allow_preview_without_other_writes() {
        let registry = ExtensionRegistry::builder(true, false, &[])
            .with_policy(CapabilityPolicy::read_only().with_preview(true))
            .register(super::builtin::get_builtin_tools())
            .register_memory_if(true)
            .build();

        assert!(registry.owns_tool("preview_server"));
        assert!(!registry.owns_tool("write_file"));
        assert!(!registry.owns_tool("memory_write"));
        assert!(!registry.owns_tool("run_command"));
    }
}

/// Concrete execution target for a registered tool.
#[derive(Debug, Clone)]
pub enum ToolHandler {
    BuiltinSync,
    BuiltinAsync,
    Memory,
    Skill { skill_name: String },
}

/// A tool registration that keeps definition, capability requirements, and handler together.
#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub capabilities: Vec<ToolCapability>,
    pub handler: ToolHandler,
}

impl RegisteredTool {
    pub fn new(
        definition: ToolDefinition,
        capabilities: Vec<ToolCapability>,
        handler: ToolHandler,
    ) -> Self {
        Self {
            definition,
            capabilities,
            handler,
        }
    }

    pub fn name(&self) -> &str {
        &self.definition.function.name
    }
}

/// Read-only view of the final tool surface after policy filtering.
///
/// This is the single source of truth for "what is actually callable right now"
/// and should be consumed by planner / prompt / hint resolution code instead of
/// re-deriving availability from static tables.
#[derive(Debug, Clone, Default)]
pub struct ToolAvailabilityView {
    tool_names: HashSet<String>,
    skill_names: HashSet<String>,
}

impl ToolAvailabilityView {
    fn register(&mut self, tool: &RegisteredTool) {
        self.tool_names.insert(tool.name().to_string());
        if let ToolHandler::Skill { skill_name } = &tool.handler {
            self.skill_names.insert(skill_name.clone());
            self.skill_names.insert(skill_name.replace('-', "_"));
        }
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tool_names.contains(name)
    }

    pub fn has_any_tool(&self, names: &[&str]) -> bool {
        names.iter().any(|name| self.has_tool(name))
    }

    pub fn has_skill_hint(&self, hint: &str) -> bool {
        self.skill_names.contains(hint) || self.skill_names.contains(&hint.replace('-', "_"))
    }

    pub fn has_any_skills(&self) -> bool {
        !self.skill_names.is_empty()
    }

    pub fn filter_callable_skills<'a>(
        &self,
        skills: &'a [LoadedSkill],
    ) -> Vec<&'a LoadedSkill> {
        skills
            .iter()
            .filter(|skill| {
                self.has_skill_hint(&skill.name)
                    || skill
                        .tool_definitions
                        .iter()
                        .any(|td| self.has_tool(&td.function.name))
            })
            .collect()
    }
}

/// Unified registry for agent tool extensions.
///
/// Tool sources are registered at construction. Pattern:
/// ```ignore
/// let registry = ExtensionRegistry::builder(enable_memory, enable_memory_vector, skills)
///     .register(builtin::get_builtin_tools())
///     .register_memory_if(enable_memory)
///     .build();
/// ```
/// Adding a new tool module = add to builtin, or `.register(new_tools())`.
#[derive(Debug)]
pub struct ExtensionRegistry<'a> {
    /// Cached tool definitions (from registered extensions + skills).
    tool_definitions: Vec<ToolDefinition>,
    /// Executable tools keyed by function name.
    tools_by_name: HashMap<String, RegisteredTool>,
    /// Final availability view after policy filtering and deduplication.
    availability: ToolAvailabilityView,
    /// Execution capability policy for this registry instance.
    policy: CapabilityPolicy,
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
    registered_tools: Vec<RegisteredTool>,
    policy: CapabilityPolicy,
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
            registered_tools: Vec::new(),
            policy: CapabilityPolicy::default(),
            enable_memory,
            enable_memory_vector,
            skills,
        }
    }

    /// Apply a capability policy before building the registry.
    #[must_use]
    pub fn with_policy(mut self, policy: CapabilityPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Register tools from an extension. Add one line per tool module.
    #[must_use]
    pub fn register(mut self, tools: impl IntoIterator<Item = RegisteredTool>) -> Self {
        self.registered_tools.extend(tools);
        self
    }

    /// Register memory tools if enable_memory is true.
    #[must_use]
    pub fn register_memory_if(mut self, enable: bool) -> Self {
        if enable {
            self.registered_tools.extend(memory::get_memory_tools());
        }
        self
    }

    /// Build the registry. Skills' tool definitions are added at build time.
    /// 按 function.name 去重，避免重复声明导致 Gemini 等 API 报 Duplicate function declaration。
    pub fn build(self) -> ExtensionRegistry<'a> {
        let mut registered_tools = self.registered_tools;
        for skill in self.skills {
            for td in &skill.tool_definitions {
                registered_tools.push(RegisteredTool::new(
                    td.clone(),
                    vec![ToolCapability::SkillExecution],
                    ToolHandler::Skill {
                        skill_name: skill.name.clone(),
                    },
                ));
            }
        }

        let mut tool_definitions = Vec::new();
        let mut tools_by_name = HashMap::new();
        let mut availability = ToolAvailabilityView::default();
        for registered in registered_tools {
            if !self.policy.allows(&registered.capabilities) {
                tracing::debug!("Skip tool due to capability policy: {}", registered.name());
                continue;
            }
            let tool_name = registered.name().to_string();
            if tools_by_name.contains_key(&tool_name) {
                tracing::debug!("Skip duplicate tool name: {}", tool_name);
                continue;
            }
            tool_definitions.push(registered.definition.clone());
            availability.register(&registered);
            tools_by_name.insert(tool_name, registered);
        }

        ExtensionRegistry {
            tool_definitions,
            tools_by_name,
            availability,
            policy: self.policy,
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
            .with_policy(CapabilityPolicy::full_access())
            .register(builtin::get_builtin_tools())
            .register_memory_if(enable_memory)
            .build()
    }

    /// Create a registry restricted to read-only tools.
    pub fn read_only(
        enable_memory: bool,
        enable_memory_vector: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self::builder(enable_memory, enable_memory_vector, skills)
            .with_policy(CapabilityPolicy::read_only())
            .register(builtin::get_builtin_tools())
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

    /// Final tool / skill availability after policy filtering.
    pub fn availability(&self) -> &ToolAvailabilityView {
        &self.availability
    }

    /// Check if any extension owns this tool name.
    pub fn owns_tool(&self, name: &str) -> bool {
        self.tools_by_name.contains_key(name)
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
        let Some(registered) = self.tools_by_name.get(tool_name) else {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Tool '{}' is unavailable in the current execution mode", tool_name),
                is_error: true,
                counts_as_failure: true,
            }
        };

        if !self.policy.allows(&registered.capabilities) {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Tool '{}' is unavailable in the current execution mode", tool_name),
                is_error: true,
                counts_as_failure: true,
            };
        }

        match &registered.handler {
            ToolHandler::BuiltinSync => {
                builtin::execute_builtin_tool(tool_name, arguments, workspace, Some(event_sink))
            }
            ToolHandler::BuiltinAsync => {
                builtin::execute_async_builtin_tool(tool_name, arguments, workspace, event_sink).await
            }
            ToolHandler::Memory => {
                memory::execute_memory_tool(
                    tool_name,
                    arguments,
                    workspace,
                    "default",
                    self.enable_memory_vector,
                    embed_ctx,
                )
                .await
            }
            ToolHandler::Skill { skill_name } => {
                if let Some(skill) = skills::find_skill_by_name(self.skills, skill_name) {
                    skills::execute_skill(skill, tool_name, arguments, workspace, event_sink, None)
                } else if let Some(skill) = skills::find_skill_by_tool_name(self.skills, tool_name) {
                    skills::execute_skill(skill, tool_name, arguments, workspace, event_sink, None)
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
                        counts_as_failure: false,
                    }
                } else {
                    ToolResult {
                        tool_call_id: String::new(),
                        tool_name: tool_name.to_string(),
                        content: format!("Unknown skill tool: {}", tool_name),
                        is_error: true,
                        counts_as_failure: true,
                    }
                }
            }
        }
    }
}
