use clap::{Parser, Subcommand};

/// SkillBox - A lightweight Skills secure execution engine
#[derive(Parser, Debug)]
#[command(name = "skilllite")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a skill with the given input (requires entry_point in SKILL.md)
    Run {
        /// Path to the skill directory
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,

        /// Input JSON string
        #[arg(value_name = "INPUT_JSON")]
        input_json: String,

        /// Allow network access (override SKILL.md policy)
        #[arg(long, default_value = "false")]
        allow_network: bool,

        /// Custom cache directory for environments
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<String>,

        /// Maximum memory limit in MB (default: from env or 256)
        #[arg(long)]
        max_memory: Option<u64>,

        /// Execution timeout in seconds (default: from env or 30)
        #[arg(long)]
        timeout: Option<u64>,

        /// Sandbox level: 1=no sandbox, 2=sandbox only, 3=sandbox+scan (default: from env or 3)
        #[arg(long)]
        sandbox_level: Option<u8>,
    },

    /// Execute a specific script directly in sandbox (no SKILL.md entry_point required)
    Exec {
        /// Path to the skill directory (for context and dependencies)
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,

        /// Path to the script to execute (relative to skill_dir)
        #[arg(value_name = "SCRIPT_PATH")]
        script_path: String,

        /// Input JSON string. Use "-" to read from stdin (for large input > ARG_MAX)
        #[arg(value_name = "INPUT_JSON")]
        input_json: String,

        /// Script arguments (passed as command line args)
        #[arg(long, value_name = "ARGS")]
        args: Option<String>,

        /// Allow network access
        #[arg(long, default_value = "false")]
        allow_network: bool,

        /// Custom cache directory for environments
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<String>,

        /// Maximum memory limit in MB (default: from env or 256)
        #[arg(long)]
        max_memory: Option<u64>,

        /// Execution timeout in seconds (default: from env or 30)
        #[arg(long)]
        timeout: Option<u64>,

        /// Sandbox level: 1=no sandbox, 2=sandbox only, 3=sandbox+scan (default: from env or 3)
        #[arg(long)]
        sandbox_level: Option<u8>,
    },

    /// Scan skill directory and list all executable scripts (JSON output for LLM analysis)
    Scan {
        /// Path to the skill directory
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,

        /// Include file content preview (first N lines)
        #[arg(long, default_value = "10")]
        preview_lines: usize,
    },

    /// Validate a skill without running it
    Validate {
        /// Path to the skill directory
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,
    },

    /// Show skill information
    Info {
        /// Path to the skill directory
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,
    },

    /// Security scan a script for potential vulnerabilities
    SecurityScan {
        /// Path to the script file to scan
        #[arg(value_name = "SCRIPT_PATH")]
        script_path: String,

        /// Allow network operations (default: false)
        #[arg(long, default_value = "false")]
        allow_network: bool,

        /// Allow file operations (default: false)
        #[arg(long, default_value = "false")]
        allow_file_ops: bool,

        /// Allow process execution (default: false)
        #[arg(long, default_value = "false")]
        allow_process_exec: bool,

        /// Output results as structured JSON (default: false)
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// Execute a bash command for a bash-tool skill (validates against allowed-tools pattern)
    ///
    /// Bash-tool skills declare `allowed-tools: Bash(prefix:*)` in SKILL.md and have
    /// no script entry point. The LLM issues bash commands which are validated and
    /// executed by this subcommand.
    Bash {
        /// Path to the skill directory (must contain SKILL.md with allowed-tools)
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,

        /// The bash command to execute (must match an allowed-tools pattern)
        #[arg(value_name = "COMMAND")]
        command: String,

        /// Custom cache directory for environments
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<String>,

        /// Execution timeout in seconds (default: 120, higher for browser automation)
        #[arg(long)]
        timeout: Option<u64>,

        /// Working directory for command execution (default: current directory).
        /// Files created by the command (e.g. screenshots) are saved relative to this path.
        #[arg(long, value_name = "DIR")]
        cwd: Option<String>,
    },

    /// Run as IPC daemon - read JSON-RPC requests from stdin, write responses to stdout
    /// Used by Python SDK when SKILLBOX_USE_IPC=1. One JSON-RPC request per line.
    Serve {
        /// Use stdio for IPC (default, for subprocess daemon)
        #[arg(long, default_value = "true")]
        stdio: bool,
    },

    /// Interactive chat with an LLM agent (requires 'agent' feature)
    #[cfg(feature = "agent")]
    Chat {
        /// OpenAI-compatible API base URL
        #[arg(long, env = "OPENAI_API_BASE")]
        api_base: Option<String>,

        /// API key
        #[arg(long, env = "OPENAI_API_KEY")]
        api_key: Option<String>,

        /// Model name (e.g. gpt-4o, claude-3-5-sonnet-20241022)
        #[arg(long, short, env = "SKILLLITE_MODEL")]
        model: Option<String>,

        /// Workspace directory (default: current directory)
        #[arg(long, short)]
        workspace: Option<String>,

        /// Skills directories to load (can be specified multiple times)
        #[arg(long, short = 's')]
        skill_dir: Vec<String>,

        /// Session key for persistent conversation
        #[arg(long, default_value = "default")]
        session: String,

        /// Maximum agent loop iterations
        #[arg(long, default_value = "50")]
        max_iterations: usize,

        /// Custom system prompt
        #[arg(long)]
        system_prompt: Option<String>,

        /// Verbose output (default: true for agent chat)
        #[arg(long, short, default_value = "true")]
        verbose: bool,

        /// Single-shot message (non-interactive mode)
        #[arg(long)]
        message: Option<String>,

        /// Enable task planning (default: true when skills are available)
        #[arg(long)]
        plan: bool,

        /// Disable task planning
        #[arg(long)]
        no_plan: bool,

        /// Disable memory tools (default: memory enabled)
        #[arg(long)]
        no_memory: bool,
    },

    // ─── Phase 3: CLI Migration Commands (flat, no nesting) ────────────

    /// Add skills from a remote repository, ClawHub, or local path
    ///
    /// Examples:
    ///   skilllite add clawhub:<skill-name>
    ///   skilllite add owner/repo
    ///   skilllite add https://github.com/owner/repo
    ///   skilllite add ./local/path
    Add {
        /// Skill source: owner/repo, GitHub URL, git URL, or local path
        #[arg(value_name = "SOURCE")]
        source: String,

        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Force overwrite existing skills
        #[arg(long, short)]
        force: bool,

        /// List available skills without installing
        #[arg(long, short)]
        list: bool,
    },

    /// Remove an installed skill
    Remove {
        /// Name of the skill to remove
        #[arg(value_name = "SKILL_NAME")]
        skill_name: String,

        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Skip confirmation prompt
        #[arg(long, short)]
        force: bool,
    },

    /// List all installed skills
    #[command(name = "list", alias = "ls")]
    List {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// List tool definitions (OpenAI/Claude format) for LLM/adapters
    #[cfg(feature = "agent")]
    #[command(name = "list-tools")]
    ListTools {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Output format: openai (default) or claude
        #[arg(long, default_value = "openai")]
        format: String,
    },

    /// Show detailed information about a skill
    Show {
        /// Name of the skill to show
        #[arg(value_name = "SKILL_NAME")]
        skill_name: String,

        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Initialize Cursor IDE integration (.cursor/mcp.json + rules)
    #[command(name = "init-cursor")]
    InitCursor {
        /// Project directory (default: current directory)
        #[arg(long, short = 'p')]
        project_dir: Option<String>,

        /// Skills directory path (default: ./.skills)
        #[arg(long, short = 's', default_value = "./.skills")]
        skills_dir: String,

        /// Install globally to ~/.cursor/mcp.json
        #[arg(long, short = 'g')]
        global: bool,

        /// Force overwrite existing config
        #[arg(long, short)]
        force: bool,
    },

    /// Initialize OpenCode integration (opencode.json + SKILL.md)
    #[command(name = "init-opencode")]
    InitOpencode {
        /// Project directory (default: current directory)
        #[arg(long, short = 'p')]
        project_dir: Option<String>,

        /// Skills directory path (default: ./.skills)
        #[arg(long, short = 's', default_value = "./.skills")]
        skills_dir: String,

        /// Force overwrite existing config
        #[arg(long, short)]
        force: bool,
    },

    /// Audit skill dependencies for known vulnerabilities
    ///
    /// Parses requirements.txt / package.json and queries vulnerability databases.
    /// Python packages use PyPI JSON API; npm packages use OSV.dev batch API.
    ///
    /// Environment variables:
    ///   SKILLLITE_AUDIT_API  — Custom security API (overrides all backends)
    ///   PYPI_MIRROR_URL     — PyPI mirror (default: https://pypi.org)
    ///   OSV_API_URL         — OSV API for npm (default: https://api.osv.dev)
    ///
    /// Examples:
    ///   skilllite dependency-audit ./my-skill
    ///   skilllite dependency-audit ./my-skill --json
    ///   PYPI_MIRROR_URL=https://pypi.tuna.tsinghua.edu.cn skilllite dependency-audit ./my-skill
    ///   SKILLLITE_AUDIT_API=https://api.mycompany.com skilllite dependency-audit ./my-skill
    #[cfg(feature = "audit")]
    #[command(name = "dependency-audit")]
    DependencyAudit {
        /// Path to the skill directory containing requirements.txt or package.json
        #[arg(value_name = "SKILL_DIR")]
        skill_dir: String,

        /// Output results as structured JSON
        #[arg(long, default_value = "false")]
        json: bool,
    },

    /// Clean cached virtual environments
    #[command(name = "clean-env")]
    CleanEnv {
        /// Dry run — show what would be removed without deleting
        #[arg(long)]
        dry_run: bool,

        /// Force removal without confirmation
        #[arg(long, short)]
        force: bool,
    },

    /// Reindex skills — rescan skills directory and rebuild metadata cache
    Reindex {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Verbose output
        #[arg(long, short)]
        verbose: bool,

        /// Rebuild .skilllite-manifest.json for existing skills
        #[arg(long)]
        rebuild_manifest: bool,
    },

    /// Initialize a SkillLite project — create .skills/, install deps, run audit
    ///
    /// Sets up the project structure, creates an example skill if needed,
    /// resolves and installs dependencies, and runs security audits.
    ///
    /// Examples:
    ///   skilllite init
    ///   skilllite init --skip-deps
    ///   skilllite init --strict
    ///   skilllite init --force
    Init {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,

        /// Skip dependency installation
        #[arg(long)]
        skip_deps: bool,

        /// Skip security audit
        #[arg(long)]
        skip_audit: bool,

        /// Strict mode — fail if security vulnerabilities found
        #[arg(long)]
        strict: bool,

        /// Force re-resolve and update dependencies (ignore .skilllite.lock)
        #[arg(long)]
        force: bool,

        /// Use LLM to resolve dependencies from compatibility string (requires agent feature, API key)
        #[cfg(feature = "agent")]
        #[arg(long)]
        use_llm: bool,
    },

    /// Quick start — auto-detect LLM, setup skills, and launch chat (requires agent feature)
    ///
    /// Zero-config flow:
    ///   1. Detect existing .env or probe local Ollama
    ///   2. Interactive LLM provider selection if needed
    ///   3. Ensure skills are available
    ///   4. Launch interactive chat
    ///
    /// Examples:
    ///   skilllite quickstart
    ///   skilllite quickstart --skills-dir ./my-skills
    #[cfg(feature = "agent")]
    #[command(name = "quickstart")]
    Quickstart {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,
    },

    /// Clear session (OpenClaw-style): summarize to memory, archive transcript, reset counts.
    ///
    /// Used by Assistant and /new. Preserves short conversations in memory before clearing.
    #[cfg(feature = "agent")]
    #[command(name = "clear-session")]
    ClearSession {
        /// Session key (default: default)
        #[arg(long, default_value = "default")]
        session_key: String,
        /// Workspace path for .env (API key); default current dir
        #[arg(long, short = 'w', default_value = ".")]
        workspace: String,
    },

    /// Run agent_chat RPC server over stdio (JSON-Lines event stream)
    ///
    /// Used by Python/TypeScript SDKs to call the Rust agent engine.
    /// Reads JSON-Lines requests from stdin, streams events to stdout.
    #[cfg(feature = "agent")]
    #[command(name = "agent-rpc")]
    AgentRpc,

    /// Run MCP (Model Context Protocol) server over stdio
    ///
    /// Implements the standard MCP JSON-RPC 2.0 protocol for IDE integration.
    /// Provides 5 tools: list_skills, get_skill_info, run_skill, scan_code, execute_code.
    ///
    /// Used by Cursor, VSCode, and other MCP-compatible IDEs.
    ///
    /// Examples:
    ///   skilllite mcp
    ///   skilllite mcp --skills-dir ./my-skills
    #[command(name = "mcp")]
    Mcp {
        /// Skills directory path (default: .skills)
        #[arg(long, short = 's', default_value = ".skills")]
        skills_dir: String,
    },
}
