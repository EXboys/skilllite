use clap::{Parser, Subcommand};

/// SkillBox - A lightweight Skills secure execution engine
#[derive(Parser, Debug)]
#[command(name = "skillbox")]
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

        /// Input JSON string (passed via stdin)
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
}
