//! Agent configuration.

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
    /// Path to SOUL.md identity document (optional).
    /// Resolution: explicit path > .skilllite/SOUL.md > ~/.skilllite/SOUL.md
    pub soul_path: Option<String>,

    /// Optional extra context to append to system prompt (e.g. from RPC params.context.append).
    /// Generic extension point for callers to inject domain-specific rules without modifying SkillLite.
    pub context_append: Option<String>,

    /// [Run mode] Max consecutive tool failures before stopping (prevents infinite retry loops).
    /// None = no limit (chat mode). Some(N) = stop after N consecutive failures (run mode).
    pub max_consecutive_failures: Option<usize>,

    /// [Run mode] Extracted goal boundaries (scope, exclusions, completion conditions).
    /// Injected into planning when set.
    pub goal_boundaries: Option<crate::goal_boundaries::GoalBoundaries>,

    /// When true, exclude conversation history from the planning prompt.
    /// Use when each task is self-contained and history from previous turns would
    /// corrupt planning (e.g. multi-turn agent orchestration, batch task dispatch).
    /// Default: false.
    pub skip_history_for_planning: bool,

    /// Restrict the tool registry to read-only operations.
    /// Used by replay/eval flows that must not mutate the workspace.
    pub read_only_tools: bool,
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
            soul_path: None,
            context_append: None,
            max_consecutive_failures: None,
            goal_boundaries: None,
            skip_history_for_planning: false,
            read_only_tools: false,
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
        let loop_limits = skilllite_core::config::AgentLoopLimitsConfig::from_env();
        Self {
            api_base: llm.api_base,
            api_key: llm.api_key,
            model: llm.model,
            max_iterations: loop_limits.max_iterations,
            max_tool_calls_per_task: loop_limits.max_tool_calls_per_task,
            workspace: paths.workspace,
            enable_memory: flags.enable_memory,
            enable_memory_vector: flags.enable_memory_vector,
            enable_task_planning: flags.enable_task_planning,
            max_consecutive_failures: Some(5),
            ..Default::default()
        }
    }
}
