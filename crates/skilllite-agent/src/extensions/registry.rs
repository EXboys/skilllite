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
use serde_json::Value;

/// Executor for planning control tools (complete_task, update_task_plan).
/// Implemented by the agent loop; passed to `registry.execute()` when available.
pub trait PlanningControlExecutor {
    fn execute(
        &mut self,
        tool_name: &str,
        arguments: &str,
        event_sink: &mut dyn EventSink,
    ) -> ToolResult;
}
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
    use super::{CapabilityPolicy, ExtensionRegistry, ToolExecutionProfile};
    use crate::types::SilentEventSink;

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

    #[test]
    fn planning_only_tools_excluded_when_task_planning_disabled() {
        let registry = ExtensionRegistry::builder(true, false, &[])
            .with_task_planning(false)
            .register(super::builtin::get_builtin_tools())
            .register_memory_if(true)
            .build();

        assert!(!registry.owns_tool("complete_task"));
        assert!(!registry.owns_tool("update_task_plan"));
        assert!(registry.owns_tool("read_file"));
    }

    #[test]
    fn tool_profile_exposes_readonly_destructive_and_concurrency_flags() {
        let registry = ExtensionRegistry::new(true, false, &[]);
        let read_file = registry
            .tool_profile("read_file")
            .expect("read_file profile");
        assert_eq!(read_file, ToolExecutionProfile::new(true, false, true));

        let write_file = registry
            .tool_profile("write_file")
            .expect("write_file profile");
        assert_eq!(write_file, ToolExecutionProfile::new(false, true, false));

        let run_command = registry
            .tool_profile("run_command")
            .expect("run_command profile");
        assert_eq!(run_command, ToolExecutionProfile::new(false, true, false));
    }

    #[tokio::test]
    async fn execute_runs_validate_input_before_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = SilentEventSink;
        let registry = ExtensionRegistry::new(true, false, &[]);
        let result = registry
            .execute(
                "read_file",
                "{not valid json",
                tmp.path(),
                &mut sink,
                None,
                None,
            )
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Invalid arguments JSON"));
    }

    #[tokio::test]
    async fn execute_runs_schema_required_check_before_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = SilentEventSink;
        let registry = ExtensionRegistry::new(true, false, &[]);
        let result = registry
            .execute("read_file", "{}", tmp.path(), &mut sink, None, None)
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Missing required field(s)"));
        assert!(result.content.contains("path"));
    }

    #[tokio::test]
    async fn execute_runs_schema_type_check_before_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = SilentEventSink;
        let registry = ExtensionRegistry::new(true, false, &[]);
        let result = registry
            .execute(
                "run_command",
                r#"{"command":123}"#,
                tmp.path(),
                &mut sink,
                None,
                None,
            )
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Field 'command'"));
        assert!(result.content.contains("string"));
    }

    #[tokio::test]
    async fn execute_runs_schema_range_check_before_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = SilentEventSink;
        let registry = ExtensionRegistry::new(true, false, &[]);
        let result = registry
            .execute(
                "preview_server",
                r#"{"path":"./","port":70000}"#,
                tmp.path(),
                &mut sink,
                None,
                None,
            )
            .await;
        assert!(result.is_error);
        assert!(result.content.contains("Field 'port'"));
        assert!(result.content.contains("maximum"));
    }
}

/// Scope that controls when a tool is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolScope {
    /// Available in all modes (simple and planning).
    AllModes,
    /// Only available when task planning is enabled.
    PlanningOnly,
}

/// Concrete execution target for a registered tool.
#[derive(Debug, Clone)]
pub enum ToolHandler {
    BuiltinSync,
    BuiltinAsync,
    Memory,
    Skill {
        skill_name: String,
    },
    /// Control tools (complete_task, update_task_plan) executed via PlanningControlExecutor.
    PlanningControl,
}

/// A tool registration that keeps definition, capability requirements, scope, and handler together.
#[derive(Debug, Clone)]
pub struct RegisteredTool {
    pub definition: ToolDefinition,
    pub capabilities: Vec<ToolCapability>,
    pub handler: ToolHandler,
    pub scope: ToolScope,
    profile: ToolExecutionProfile,
}

impl RegisteredTool {
    pub fn new(
        definition: ToolDefinition,
        capabilities: Vec<ToolCapability>,
        handler: ToolHandler,
    ) -> Self {
        let profile = ToolExecutionProfile::from_capabilities(&capabilities);
        Self {
            definition,
            capabilities,
            handler,
            scope: ToolScope::AllModes,
            profile,
        }
    }

    #[must_use]
    pub fn with_scope(mut self, scope: ToolScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    pub fn with_execution_profile(mut self, profile: ToolExecutionProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn name(&self) -> &str {
        &self.definition.function.name
    }

    pub fn validate_input(&self, arguments: &str) -> Result<(), String> {
        if self.definition.function.name == "write_file"
            || self.definition.function.name == "write_output"
        {
            // Keep tolerant recovery path for potentially truncated JSON in file/content transfer tools.
            return Ok(());
        }
        let parsed = serde_json::from_str::<Value>(arguments)
            .map_err(|e| format!("Invalid arguments JSON: {}", e))?;
        self.validate_required_fields(&parsed)?;
        self.validate_schema_constraints(&parsed)
    }

    pub fn check_permissions(&self, policy: &CapabilityPolicy) -> Result<(), String> {
        if policy.allows(&self.capabilities) {
            Ok(())
        } else {
            Err(format!(
                "Tool '{}' is unavailable in the current execution mode",
                self.name()
            ))
        }
    }

    pub fn render_use_result(&self, event_sink: &mut dyn EventSink, result: &ToolResult) {
        event_sink.on_tool_result(self.name(), &result.content, result.is_error);
    }

    pub fn execution_profile(&self) -> ToolExecutionProfile {
        self.profile
    }

    pub fn is_read_only(&self) -> bool {
        self.profile.is_read_only
    }

    pub fn is_destructive(&self) -> bool {
        self.profile.is_destructive
    }

    pub fn is_concurrency_safe(&self) -> bool {
        self.profile.is_concurrency_safe
    }

    fn validate_required_fields(&self, parsed: &Value) -> Result<(), String> {
        let required = self
            .definition
            .function
            .parameters
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if required.is_empty() {
            return Ok(());
        }
        let obj = parsed.as_object().ok_or_else(|| {
            format!(
                "Invalid arguments JSON: expected object for tool '{}'",
                self.name()
            )
        })?;
        let missing: Vec<String> = required
            .iter()
            .filter_map(Value::as_str)
            .filter(|key| !obj.contains_key(*key))
            .map(ToString::to_string)
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(format!("Missing required field(s): {}", missing.join(", ")))
        }
    }

    fn validate_schema_constraints(&self, parsed: &Value) -> Result<(), String> {
        let params = &self.definition.function.parameters;
        if let Some(schema_type) = params.get("type").and_then(Value::as_str) {
            self.ensure_type(schema_type, parsed, "arguments")?;
        }

        let Some(obj) = parsed.as_object() else {
            return Ok(());
        };
        let Some(properties) = params.get("properties").and_then(Value::as_object) else {
            return Ok(());
        };
        for (name, value) in obj {
            if let Some(schema) = properties.get(name) {
                self.validate_field_against_schema(name, value, schema)?;
            }
        }
        Ok(())
    }

    fn validate_field_against_schema(
        &self,
        field_name: &str,
        value: &Value,
        schema: &Value,
    ) -> Result<(), String> {
        if self.accepts_legacy_field_shape(field_name, value) {
            return Ok(());
        }
        if let Some(expected_type) = schema.get("type").and_then(Value::as_str) {
            self.ensure_type(expected_type, value, field_name)?;
        }
        if let Some(enum_values) = schema.get("enum").and_then(Value::as_array) {
            if !enum_values.iter().any(|allowed| allowed == value) {
                return Err(format!(
                    "Field '{}' must be one of {}",
                    field_name,
                    serde_json::to_string(enum_values).unwrap_or_else(|_| "[]".to_string())
                ));
            }
        }
        if value.is_number() {
            let as_f64 = value.as_f64().unwrap_or(0.0);
            if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
                if as_f64 < min {
                    return Err(format!("Field '{}' must be >= minimum {}", field_name, min));
                }
            }
            if let Some(max) = schema.get("maximum").and_then(Value::as_f64) {
                if as_f64 > max {
                    return Err(format!("Field '{}' must be <= maximum {}", field_name, max));
                }
            }
        }
        Ok(())
    }

    fn accepts_legacy_field_shape(&self, field_name: &str, value: &Value) -> bool {
        match self.name() {
            // Legacy compatibility: planner control parsers coerce numeric string task_id.
            "complete_task" if field_name == "task_id" => value.as_str().is_some(),
            // Legacy compatibility: planner parser accepts stringified JSON array for tasks.
            "update_task_plan" if field_name == "tasks" => value.as_str().is_some(),
            _ => false,
        }
    }

    fn ensure_type(
        &self,
        expected_type: &str,
        value: &Value,
        field_name: &str,
    ) -> Result<(), String> {
        let valid = match expected_type {
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "object" => value.is_object(),
            "array" => value.is_array(),
            "null" => value.is_null(),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "Field '{}' must be of type {}",
                field_name, expected_type
            ))
        }
    }
}

/// Unified lifecycle metadata used by audit/test/policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolExecutionProfile {
    pub is_read_only: bool,
    pub is_destructive: bool,
    pub is_concurrency_safe: bool,
}

impl ToolExecutionProfile {
    pub const fn new(is_read_only: bool, is_destructive: bool, is_concurrency_safe: bool) -> Self {
        Self {
            is_read_only,
            is_destructive,
            is_concurrency_safe,
        }
    }

    fn from_capabilities(capabilities: &[ToolCapability]) -> Self {
        let writes_or_exec = capabilities.iter().any(|cap| {
            matches!(
                cap,
                ToolCapability::FilesystemWrite
                    | ToolCapability::MemoryWrite
                    | ToolCapability::ProcessExec
                    | ToolCapability::Preview
                    | ToolCapability::Delegation
                    | ToolCapability::SkillExecution
            )
        });
        let non_parallel = capabilities.iter().any(|cap| {
            matches!(
                cap,
                ToolCapability::FilesystemWrite
                    | ToolCapability::MemoryWrite
                    | ToolCapability::ProcessExec
                    | ToolCapability::Preview
                    | ToolCapability::Delegation
            )
        });
        Self::new(!writes_or_exec, writes_or_exec, !non_parallel)
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

    pub fn filter_callable_skills<'a>(&self, skills: &'a [LoadedSkill]) -> Vec<&'a LoadedSkill> {
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
    enable_task_planning: bool,
    skills: &'a [LoadedSkill],
}

impl<'a> ExtensionRegistryBuilder<'a> {
    /// Create a new builder. Call `register()` for each tool provider, then `build()`.
    pub fn new(enable_memory: bool, enable_memory_vector: bool, skills: &'a [LoadedSkill]) -> Self {
        Self {
            registered_tools: Vec::new(),
            policy: CapabilityPolicy::default(),
            enable_memory,
            enable_memory_vector,
            enable_task_planning: true, // default: include planning tools for backward compat
            skills,
        }
    }

    /// Exclude PlanningOnly tools when false (simple mode).
    #[must_use]
    pub fn with_task_planning(mut self, enable: bool) -> Self {
        self.enable_task_planning = enable;
        self
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
            if registered.scope == ToolScope::PlanningOnly && !self.enable_task_planning {
                tracing::debug!(
                    "Skip PlanningOnly tool (task planning disabled): {}",
                    registered.name()
                );
                continue;
            }
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
    pub fn new(enable_memory: bool, enable_memory_vector: bool, skills: &'a [LoadedSkill]) -> Self {
        Self::builder(enable_memory, enable_memory_vector, skills)
            .with_policy(CapabilityPolicy::full_access())
            .register(builtin::get_builtin_tools())
            .register_memory_if(enable_memory)
            .build()
    }

    /// Create a registry with explicit task-planning mode.
    /// When `enable_task_planning` is false, PlanningOnly tools (complete_task, update_task_plan) are excluded.
    pub fn with_task_planning(
        enable_memory: bool,
        enable_memory_vector: bool,
        enable_task_planning: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self::builder(enable_memory, enable_memory_vector, skills)
            .with_task_planning(enable_task_planning)
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

    /// Create a read-only registry with explicit task-planning mode.
    pub fn read_only_with_task_planning(
        enable_memory: bool,
        enable_memory_vector: bool,
        enable_task_planning: bool,
        skills: &'a [LoadedSkill],
    ) -> Self {
        Self::builder(enable_memory, enable_memory_vector, skills)
            .with_task_planning(enable_task_planning)
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

    /// Returns unified lifecycle profile for a callable tool.
    pub fn tool_profile(&self, name: &str) -> Option<ToolExecutionProfile> {
        self.tools_by_name.get(name).map(|t| t.execution_profile())
    }

    /// Render/use tool result via the tool's unified lifecycle hook.
    pub fn render_tool_result(
        &self,
        tool_name: &str,
        result: &ToolResult,
        event_sink: &mut dyn EventSink,
    ) {
        if let Some(registered) = self.tools_by_name.get(tool_name) {
            registered.render_use_result(event_sink, result);
        } else {
            event_sink.on_tool_result(tool_name, &result.content, result.is_error);
        }
    }

    /// Execute a tool by name. Dispatches to the appropriate extension.
    /// `embed_ctx` is required for memory vector search when enable_memory_vector is true.
    /// `planning_ctx` is required for PlanningControl tools (complete_task, update_task_plan).
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: &str,
        workspace: &Path,
        event_sink: &mut dyn EventSink,
        embed_ctx: Option<&MemoryVectorContext<'_>>,
        planning_ctx: Option<&mut dyn PlanningControlExecutor>,
    ) -> ToolResult {
        let Some(registered) = self.tools_by_name.get(tool_name) else {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!(
                    "Tool '{}' is unavailable in the current execution mode",
                    tool_name
                ),
                is_error: true,
                counts_as_failure: true,
            };
        };

        if let Err(message) = registered.validate_input(arguments) {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: message,
                is_error: true,
                counts_as_failure: true,
            };
        }

        if let Err(message) = registered.check_permissions(&self.policy) {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: message,
                is_error: true,
                counts_as_failure: true,
            };
        }

        match &registered.handler {
            ToolHandler::PlanningControl => {
                if let Some(ctx) = planning_ctx {
                    ctx.execute(tool_name, arguments, event_sink)
                } else {
                    ToolResult {
                        tool_call_id: String::new(),
                        tool_name: tool_name.to_string(),
                        content: format!(
                            "Tool '{}' requires task-planning mode and must be executed by the agent loop",
                            tool_name
                        ),
                        is_error: true,
                        counts_as_failure: true,
                    }
                }
            }
            ToolHandler::BuiltinSync => {
                builtin::execute_builtin_tool(tool_name, arguments, workspace, Some(event_sink))
            }
            ToolHandler::BuiltinAsync => {
                builtin::execute_async_builtin_tool(tool_name, arguments, workspace, event_sink)
                    .await
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
                } else if let Some(skill) = skills::find_skill_by_tool_name(self.skills, tool_name)
                {
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
