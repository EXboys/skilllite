# SkillLite Project Architecture

> **Note**: This document is synced to the v0.1.9 architecture. The Python SDK is a thin bridge layer (~600 lines) exporting `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`; all logic lives in the Rust binary.

## Overview

**SkillLite** is a lightweight AI Agent Skills execution engine, organized as a two-layer product:

```
┌──────────────────────────────────────────────────────────┐
│  SkillLite Agent (Upper Layer)                           │
│  Built-in agent framework: chat, planning, memory, tools │
│  Purpose: Out-of-the-box AI assistant, best practice     │
│           reference for Core                             │
│  Binary: skilllite (full binary)                         │
├──────────────────────────────────────────────────────────┤
│  SkillLite Core (Lower Layer Engine)                     │
│  Sandbox execution + Security scanning + Skills + MCP    │
│  Purpose: Secure execution engine embeddable by any      │
│           agent framework                                │
│  Binary: skilllite-sandbox (lightweight binary)          │
└──────────────────────────────────────────────────────────┘
```

Agent is the first customer of Core, and also the best reference implementation.

### Core Features

- **Built-in Native System-Level Sandbox**: Rust-implemented native system-level security isolation
- **Zero Dependencies**: Single binary, millisecond cold start
- **Local Execution**: Code and data never leave your machine
- **LLM Agnostic**: Compatible with all OpenAI API format LLM providers
- **Cross-Platform**: macOS (Seatbelt), Linux (Namespace + Seccomp), Windows (WSL2 Bridge)

### Tech Stack

| Component | Technology |
|-----------|------------|
| Sandbox Executor | Rust (skilllite binary) |
| Python SDK | Python 3.x (python-sdk) |
| macOS Sandbox | Seatbelt (sandbox-exec) |
| Linux Sandbox | Namespace + Seccomp (bubblewrap / firejail) |
| Windows Sandbox | WSL2 Bridge |

### Core Use Cases

| Scenario | Description | Users |
|----------|-------------|-------|
| **Integration** | Embed SkillLite Core when AI frameworks need secure untrusted code execution | Framework developers, IDE vendors |
| **Skills Ecosystem** | Standardized AI tool packaging (SKILL.md), distribution, secure execution | Skills developers |
| **Security Compliance** | Prove AI-executed code doesn't leak data or damage systems | Enterprise AI teams |
| **Out-of-the-Box** | `skilllite chat` for complete agent experience | Terminal developers |

---

## Project Structure

```
skillLite/
├── skilllite/                     # Rust sandbox executor (core)
│   ├── Cargo.toml                 # Rust dependency config
│   └── src/
│       ├── main.rs                # CLI entry (~210 lines, argument parsing & dispatch only)
│       ├── cli.rs                 # CLI argument definitions
│       ├── mcp.rs                 # MCP protocol server
│       ├── stdio_rpc.rs           # Stdio JSON-RPC service
│       ├── observability.rs       # Observability (tracing)
│       ├── path_validation.rs     # Path validation
│       │
│       ├── commands/              # Command implementations
│       │   ├── execute.rs         # run_skill, exec_script, bash_command
│       │   ├── scan.rs            # scan_skill
│       │   ├── security.rs        # security_scan, dependency_audit
│       │   ├── skill.rs           # add, remove, list, show
│       │   ├── ide.rs             # Cursor / OpenCode integration
│       │   ├── init.rs            # Project initialization
│       │   ├── quickstart.rs      # Quick start (agent feature)
│       │   ├── env.rs             # Environment management (clean)
│       │   ├── reindex.rs         # Re-index skills
│       │   └── planning_rules_gen.rs  # Planning rules generation
│       │
│       ├── config/                # Configuration module
│       │   ├── loader.rs          # Env loading + safe set_var wrappers
│       │   ├── schema.rs          # Config schema (LlmConfig, etc.)
│       │   └── env_keys.rs        # Environment variable key constants
│       │
│       ├── env/                   # Runtime environment
│       │   └── builder.rs         # build_runtime_paths, ensure_environment
│       │
│       ├── skill/                 # Skill metadata parsing
│       │   ├── metadata.rs        # SKILL.md parsing
│       │   ├── schema.rs          # Skill schema definitions
│       │   ├── deps.rs            # Dependency management
│       │   └── dependency_resolver.rs  # Dependency resolver
│       │
│       ├── sandbox/               # Sandbox implementation (core security)
│       │   ├── runner.rs          # SandboxLevel, SandboxConfig, ResourceLimits
│       │   ├── common.rs          # Cross-platform utilities (memory monitoring)
│       │   ├── macos.rs           # macOS Seatbelt sandbox
│       │   ├── linux.rs           # Linux Namespace sandbox
│       │   ├── windows.rs         # Windows WSL2 bridge
│       │   ├── seatbelt.rs        # Seatbelt profiles & mandatory deny rules
│       │   ├── seccomp.rs         # Linux Seccomp BPF filters
│       │   ├── network_proxy.rs   # HTTP/SOCKS5 network proxy (domain filtering)
│       │   ├── bash_validator.rs  # Bash command safety validation
│       │   ├── move_protection.rs # File move protection
│       │   ├── log.rs             # Sandbox logging
│       │   └── security/          # Security scanning submodule
│       │       ├── scanner.rs     # Static code scanner
│       │       ├── rules.rs       # Security rule definitions & matching
│       │       ├── types.rs       # Security type definitions
│       │       ├── policy.rs      # Runtime security policy
│       │       ├── default_rules.rs   # Default rule implementations
│       │       ├── default_rules.yaml # Configurable rules file
│       │       └── dependency_audit.rs # Supply chain vulnerability scanning (OSV API)
│       │
│       ├── executor/              # Executor module (executor feature)
│       │   ├── session.rs         # Session management
│       │   ├── transcript.rs      # Conversation transcripts
│       │   ├── memory.rs          # Memory storage (BM25 retrieval)
│       │   └── rpc.rs             # Executor RPC
│       │
│       └── agent/                 # Agent loop (agent feature)
│           ├── chat.rs            # CLI chat entry (single/REPL)
│           ├── agent_loop.rs      # Agent main loop
│           ├── llm.rs             # LLM client (OpenAI/Claude)
│           ├── chat_session.rs    # Chat session management
│           ├── prompt.rs          # Prompt construction
│           ├── skills.rs          # Skill loading and management
│           ├── rpc.rs             # Agent RPC (JSON-Lines event stream)
│           ├── task_planner.rs    # Task planner
│           ├── planning_rules.rs  # Planning rules
│           ├── types.rs           # Agent type definitions
│           ├── long_text/         # Long text handling
│           │   ├── mod.rs
│           │   └── filter.rs
│           └── extensions/        # Tool extensions
│               ├── registry.rs    # Unified extension registry
│               ├── memory.rs      # Memory tools (search/write/list)
│               └── builtin/       # Built-in tools
│                   ├── file_ops.rs     # read_file, write_file, search_replace, etc.
│                   ├── run_command.rs  # run_command + dangerous command detection
│                   ├── output.rs      # write_output, list_output
│                   ├── preview.rs     # preview_server (built-in HTTP server)
│                   └── chat_data.rs   # chat_history, chat_plan
│
├── python-sdk/                    # Python SDK (thin bridge layer)
│   ├── pyproject.toml             # Package config (v0.1.9, zero runtime deps)
│   └── skilllite/
│       ├── __init__.py            # Exports: chat, run_skill, scan_code, execute_code
│       ├── api.py                 # Core API (subprocess calls to skilllite binary)
│       ├── binary.py              # Binary management (bundled/PATH resolution)
│       ├── cli.py                 # CLI entry (forwards to binary)
│       └── ipc.py                 # IPC client
│
├── langchain-skilllite/           # LangChain adapter (separate package, v0.1.8)
│   └── langchain_skilllite/
│       ├── core.py                # SkillManager, SkillInfo
│       ├── tools.py               # SkillLiteTool, SkillLiteToolkit
│       └── callbacks.py           # Callback handler
│
├── benchmark/                     # Performance tests
│   ├── benchmark_runner.py        # Performance benchmarks (cold start/concurrency)
│   ├── security_vs.py             # Security comparison tests
│   └── security_detailed_vs.py    # Detailed security comparison
│
├── .skills/                       # Skills directory (examples)
│   ├── agent-browser/             # Browser automation
│   ├── calculator/                # Calculator
│   ├── csdn-article/             # CSDN article
│   ├── data-analysis/            # Data analysis
│   ├── frontend-design/          # Frontend design
│   ├── http-request/             # HTTP request
│   ├── nodejs-test/              # Node.js test
│   ├── skill-creator/            # Skill creator
│   ├── text-processor/           # Text processor
│   ├── weather/                  # Weather query
│   ├── writing-helper/           # Writing assistant
│   └── xiaohongshu-writer/       # Xiaohongshu writer
│
├── tutorials/                     # Tutorial examples
├── test/                          # Integration tests
├── tests/                         # Additional tests
├── scripts/                       # Build scripts
├── docs/                          # Documentation (zh/en)
│   ├── zh/                        # Chinese docs
│   └── en/                        # English docs
│
├── install.sh                     # Unix install script
├── install.ps1                    # Windows install script
├── simple_demo.py                 # Complete example
└── README.md                      # Project readme
```

---

## Core Modules

### 1. Rust Three-Layer Architecture

```
Entry Layer (CLI/MCP/stdio_rpc) → Agent → Executor → Sandbox → Core
Core doesn't depend on upper layers; Agent is Core's customer, not part of Core
```

**Feature Flags Control Compilation**:

| Feature | Included Modules | Build Target |
|---------|-----------------|--------------|
| `sandbox` (default) | sandbox, skill, config, env | Sandbox core |
| `audit` (default) | dependency_audit (OSV API) | Supply chain audit |
| `executor` | session, transcript, memory | Session management |
| `agent` (default) | agent_loop, llm, chat, extensions | Agent features |
| `sandbox_binary` | sandbox + core only | skilllite-sandbox lightweight binary |
| `memory_vector` | sqlite-vec vector retrieval | Optional semantic search |

**Build Targets**:
- `cargo build -p skilllite`: Full product (chat/add/list/mcp/init, etc.)
- `cargo build --features sandbox_binary`: Core engine (run/exec/bash, no agent)

### 2. Sandbox Module (sandbox/)

#### 2.1 Sandbox Security Levels (`sandbox/runner.rs`)

```rust
pub enum SandboxLevel {
    Level1,  // No sandbox - direct execution, no isolation
    Level2,  // Sandbox isolation only (macOS Seatbelt / Linux namespace + seccomp)
    Level3,  // Sandbox isolation + static code scanning (default)
}
```

#### 2.2 SandboxConfig (Decoupled sandbox from skill)

```rust
pub struct SandboxConfig {
    pub name: String,
    pub entry_point: String,
    pub language: String,
    pub network_enabled: bool,
    pub network_outbound: Vec<String>,
    pub uses_playwright: bool,
}
```

The sandbox no longer directly `use crate::skill::*`; it receives a `SandboxConfig` constructed by the caller from `SkillMetadata`.

#### 2.3 RuntimePaths (Decoupled sandbox from env)

```rust
pub struct RuntimePaths {
    pub python: PathBuf,
    pub node: PathBuf,
    pub node_modules: PathBuf,
    pub env_dir: PathBuf,
}
```

The sandbox no longer `use crate::env::builder::*`; it receives `RuntimePaths` constructed via `env/builder.rs::build_runtime_paths()`.

#### 2.4 Resource Limits (`sandbox/runner.rs`)

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,   // Default 512MB
    pub timeout_secs: u64,    // Default 30 seconds
}
```

**Environment Variables:**
- `SKILLBOX_MAX_MEMORY_MB`: Maximum memory limit
- `SKILLBOX_TIMEOUT_SECS`: Execution timeout
- `SKILLBOX_SANDBOX_LEVEL`: Sandbox level (1/2/3)
- `SKILLBOX_AUTO_APPROVE`: Auto-approve dangerous operations

#### 2.5 macOS Sandbox (`sandbox/macos.rs`)

**Core Technology**: Uses macOS `sandbox-exec` with Seatbelt profiles

**Execution Flow:**
1. Check if sandbox is disabled (`SKILLBOX_NO_SANDBOX`)
2. Start network proxy (if networking enabled with domain whitelist)
3. Generate Seatbelt profile (restrict filesystem, network access)
4. Launch child process via `sandbox-exec`
5. Monitor memory usage and execution time
6. Terminate process on limit exceeded

#### 2.6 Linux Sandbox (`sandbox/linux.rs`)

**Sandbox Tool Priority**: bubblewrap (bwrap) → firejail → error

**Bubblewrap Isolation:**
- `--unshare-all`: Unshare all namespaces
- Minimal filesystem mounts (read-only /usr, /lib, /bin)
- Skill directory mounted read-only
- Network isolation (default `--unshare-net`; `--share-net` with proxy filtering when enabled)
- Seccomp BPF filter blocks AF_UNIX socket creation

#### 2.7 Windows Sandbox (`sandbox/windows.rs`)

Sandbox functionality implemented via WSL2 bridge.

#### 2.8 Network Proxy (`sandbox/network_proxy.rs`)

Provides HTTP and SOCKS5 proxy for domain whitelist filtering. When a skill declares network access with restricted outbound domains, the proxy intercepts non-whitelisted requests.

#### 2.9 Static Code Scanning (`sandbox/security/`)

The security scanning module contains:

| File | Responsibility |
|------|---------------|
| `scanner.rs` | Scanner main logic (ScriptScanner) |
| `rules.rs` | Security rule definitions and matching |
| `types.rs` | Security type definitions |
| `policy.rs` | Runtime security policy (path/process/network) |
| `default_rules.rs` | Default rule implementations |
| `default_rules.yaml` | Configurable rules file |
| `dependency_audit.rs` | Supply chain vulnerability scanning (OSV API, requires audit feature) |

**Security Issue Types** (`security/types.rs`):
```rust
pub enum SecurityIssueType {
    FileOperation,      // File operations
    NetworkRequest,     // Network requests
    CodeInjection,      // Code injection (eval, exec)
    MemoryBomb,         // Memory bombs
    ProcessExecution,   // Process execution
    SystemAccess,       // System access
    DangerousModule,    // Dangerous module imports
}

pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

#### 2.10 Additional Security Modules

| Module | Responsibility |
|--------|---------------|
| `bash_validator.rs` | Bash command safety validation, detects dangerous commands |
| `move_protection.rs` | File move protection, prevents malicious file overwrites |
| `seatbelt.rs` | macOS mandatory deny paths and Seatbelt profile generation |

---

### 3. Executor Module (executor/)

> Requires `executor` feature. Provides session management and persistence.

| Module | Responsibility |
|--------|---------------|
| `session.rs` | Session lifecycle management |
| `transcript.rs` | Conversation transcript persistence |
| `memory.rs` | Memory storage (BM25 retrieval, optional sqlite-vec vector search) |
| `rpc.rs` | Executor RPC interface |

**Data Storage Path**: `~/.skilllite/` (chat records, session data, memory indices)

---

### 4. Agent Module (agent/)

> Requires `agent` feature (enabled by default). Provides full AI Agent capabilities.

#### 4.1 Core Modules

| Module | Responsibility |
|--------|---------------|
| `chat.rs` | CLI chat entry (single `--message` / interactive REPL) |
| `agent_loop.rs` | Agent main loop (LLM call → tool execution → result return) |
| `llm.rs` | LLM HTTP client (supports OpenAI-compatible API and Claude Native API, streaming/non-streaming) |
| `chat_session.rs` | Chat session management |
| `prompt.rs` | System prompt construction |
| `skills.rs` | Skill loading and tool definition generation |
| `rpc.rs` | Agent RPC server (JSON-Lines event stream protocol) |
| `task_planner.rs` | Task planner |
| `planning_rules.rs` | Planning rules configuration |
| `types.rs` | Agent type definitions |

#### 4.2 Long Text Handling (`long_text/`)

Automatically detects and handles overly long text output to prevent LLM context overflow.

#### 4.3 Tool Extension System (`extensions/`)

**Registry Pattern** (compile-time registration):

```rust
registry.register(builtin::file_ops::tools());
registry.register(builtin::run_command::tools());
registry.register(memory::tools());
// Adding a new tool = one line of registration, no changes to agent_loop
```

**Built-in Tools** (`extensions/builtin/`):

| File | Tools |
|------|-------|
| `file_ops.rs` | read_file, write_file, search_replace, list_directory, file_exists |
| `run_command.rs` | run_command (with dangerous command detection and user confirmation) |
| `output.rs` | write_output, list_output |
| `preview.rs` | preview_server (built-in HTTP file server) |
| `chat_data.rs` | chat_history, chat_plan, update_task_plan |

**Memory Tools** (`extensions/memory.rs`):

| Tool | Description |
|------|-------------|
| `memory_search` | Search historical conversation memory |
| `memory_write` | Write new memory |
| `memory_list` | List all memories |

---

### 5. MCP Module (mcp.rs)

**MCP (Model Context Protocol) Server**: JSON-RPC 2.0 over stdio

**Provides 5 Tools**:

| Tool | Description |
|------|-------------|
| `list_skills` | List all installed skills |
| `get_skill_info` | Get skill detailed information |
| `run_skill` | Execute skill (with two-phase security scan confirmation) |
| `scan_code` | Scan code for security issues |
| `execute_code` | Execute code (with two-phase security scan confirmation) |

**Two-Phase Confirmation**: Scan first, then execute after user confirms. Scan result cache TTL: 300 seconds.

---

### 6. Stdio RPC Module (stdio_rpc.rs)

**Skill Execution Stdio RPC**: JSON-RPC 2.0 over stdio (one request per line)

Uses rayon thread pool for concurrent request processing. Supported methods: `run`, `exec`, `bash`, `scan`, `validate`, `info`, etc.

Separate from `agent::rpc` — the latter is dedicated to Agent Chat streaming events.

---

### 7. Python SDK (python-sdk)

> **Note**: The Python SDK is a thin bridge layer (~600 lines), zero runtime dependencies, all operations performed via subprocess calls to the skilllite binary.

**Modules:**

| Module | Responsibility |
|--------|---------------|
| `api.py` | `scan_code`, `execute_code`, `chat`, `run_skill` via subprocess calls to skilllite binary |
| `binary.py` | Binary management: `get_binary`, bundled/PATH resolution |
| `cli.py` | CLI entry, forwards to binary |
| `ipc.py` | IPC client, communicates with `skilllite serve` daemon |

**Exported API**: `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`

**Programmatic Agent**: Use `skilllite chat --message` or `api.chat()` to invoke the Rust Agent loop.

---

### 8. LangChain Integration (langchain-skilllite)

> Separate package `pip install langchain-skilllite` (v0.1.8)

| Module | Responsibility |
|--------|---------------|
| `core.py` | SkillManager, SkillInfo — Skill scanning and management |
| `tools.py` | SkillLiteTool, SkillLiteToolkit — LangChain tool adapters |
| `callbacks.py` | Callback handler |

**Dependencies**: `langchain-core>=0.3.0`, `skilllite>=0.1.8`

---

### 9. Skill Metadata Parsing (`skill/`)

#### 9.1 SKILL.md Format

```yaml
---
name: my-skill
description: A skill that does something useful.
license: Apache-2.0
compatibility: Requires Python 3.x with pandas library, network access
metadata:
  author: example-org
  version: "1.0"
---
```

**Field Reference** (follows Claude Agent Skills spec):

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Skill name, max 64 chars, lowercase letters, digits, and hyphens only |
| `description` | Yes | Skill description, max 1024 chars |
| `license` | No | License name or reference |
| `compatibility` | No | Environment requirements, max 500 chars (infers network permissions, language, and dependencies) |
| `metadata` | No | Additional metadata (author, version, etc.) |
| `allowed-tools` | No | Pre-approved tool list (experimental) |

#### 9.2 Auto-Inference from `compatibility` Field

1. **Network Permissions**: Contains "network", "internet", "http", "api", "web" → enables network access
2. **Language Detection**: Python / Node / JavaScript / bash / shell
3. **Dependency Management**: Automatically extracts known package names and installs them

#### 9.3 Entry Point Auto-Detection

```rust
fn detect_entry_point(skill_dir: &Path) -> Option<String> {
    // Priority: main.py > main.js > main.ts > main.sh
    // Then: index.* > run.* > entry.* > app.* > cli.*
    // Finally: if only one script file exists, use it
}
```

#### 9.4 Dependency Resolver (`dependency_resolver.rs`)

Standalone dependency resolver supporting automatic parsing and installation of Python/Node dependencies from SKILL.md and compatibility fields.

---

## Execution Flow

### Complete Execution Flow

```
User Input
    ↓
skilllite chat / api.chat() / skilllite chat --message
    ↓
Rust Agent (skilllite binary)
    ↓
┌─────────────────────────────────────┐
│ 1. Generate system prompt (w/ Skills)│
│ 2. Call LLM                         │
│ 3. Parse tool calls                 │
│ 4. Execute tools (built-in / Skill) │
│ 5. Return results to LLM           │
│ 6. Repeat until done or max iters   │
└─────────────────────────────────────┘
    ↓
Rust Sandbox.execute()
    ↓
┌─────────────────────────────────────┐
│ 1. Parse SKILL.md metadata          │
│ 2. Setup runtime env (RuntimePaths) │
│ 3. Level 3: Static code scanning    │
│ 4. Level 2+: Start system sandbox   │
│ 5. Execute script                   │
│ 6. Monitor resource usage           │
│ 7. Return result                    │
└─────────────────────────────────────┘
    ↓
Return execution result
```

### CLI Commands

```bash
# Execution
skilllite run <skill_dir> '<input_json>'       # Run Skill
skilllite exec <skill_dir> <script> '<json>'   # Execute script directly
skilllite bash <skill_dir> '<command>'         # Execute Bash command

# Scanning
skilllite scan <skill_dir>                     # Scan Skill
skilllite validate <skill_dir>                 # Validate Skill
skilllite info <skill_dir>                     # Show Skill info
skilllite security-scan <script_path>          # Security scan
skilllite dependency-audit <skill_dir>         # Supply chain audit

# Agent (agent feature)
skilllite chat                                 # Interactive chat
skilllite chat --message "..."                 # Single message
skilllite quickstart                           # Quick start
skilllite agent-rpc                            # Agent RPC server

# Management
skilllite add <source>                         # Add Skill
skilllite remove <skill_name>                  # Remove Skill
skilllite list                                 # List all Skills
skilllite show <skill_name>                    # Show Skill details
skilllite list-tools                           # List tool definitions

# Services
skilllite serve                                # IPC daemon (stdio JSON-RPC)
skilllite mcp                                  # MCP protocol server

# IDE Integration
skilllite init-cursor                          # Initialize Cursor integration
skilllite init-opencode                        # Initialize OpenCode integration

# Maintenance
skilllite init                                 # Project initialization
skilllite clean-env                            # Clean cached environments
skilllite reindex                              # Re-index Skills
```

---

## Skill Structure

### Standard Skill Directory

```
my-skill/
├── SKILL.md           # Required: metadata and documentation (includes deps)
├── scripts/           # Script directory
│   └── main.py        # Entry script
├── references/        # Optional: reference documents
│   └── api-docs.md
└── assets/            # Optional: resource files
    └── config.json
```

> **Note**: Python dependencies are no longer declared in `requirements.txt` but via the `compatibility` field in `SKILL.md`.

### Full SKILL.md Example

```markdown
---
name: weather
description: Query weather information for any location. Use when user asks about weather, temperature, or forecast.
license: MIT
compatibility: Requires Python 3.x with requests library, network access
metadata:
  author: example-org
  version: "1.0"
---

# Weather Skill

Query weather information for a specified city.

## Input Parameters

- `city`: City name (required)

## Output Format

Returns weather data in JSON format.
```

---

## Configuration

### Environment Variables

```bash
# LLM Configuration
BASE_URL=https://api.deepseek.com/v1
API_KEY=your_api_key
MODEL=deepseek-chat

# Sandbox Configuration
SKILLBOX_SANDBOX_LEVEL=3      # 1/2/3
SKILLBOX_MAX_MEMORY_MB=512    # Memory limit
SKILLBOX_TIMEOUT_SECS=30      # Timeout
SKILLBOX_AUTO_APPROVE=false   # Auto-approve dangerous operations
SKILLBOX_NO_SANDBOX=false     # Disable sandbox
```

Environment variable keys are defined in `config/env_keys.rs` with legacy compatibility. Config loading priority: constructor args > env vars > .env file > defaults.

---

## Security Mechanisms

### 1. Sandbox Isolation

**macOS (Seatbelt)**:
- Filesystem isolation: Only Skill directory and temp directory accessible
- Network isolation: Disabled by default, can be enabled per domain whitelist (via network_proxy)
- Process isolation: Each Skill runs in its own process

**Linux (Namespace + Seccomp)**:
- Mount namespace: Isolated filesystem view
- PID namespace: Isolated process space
- Network namespace: Isolated networking
- Seccomp BPF: Restricted syscalls (blocks AF_UNIX socket creation)
- Supported tools: bubblewrap (bwrap) or firejail

**Windows (WSL2 Bridge)**:
- Bridges to Linux sandbox via WSL2

### 2. Static Code Scanning

**Detection:**
- Code injection: `eval()`, `exec()`, `__import__()`
- Process execution: `subprocess`, `os.system`
- Unsafe deserialization: `pickle.loads`, `yaml.unsafe_load`
- Memory bombs: Large array allocation, infinite loops
- System access: Environment variables, user info

### 3. Resource Limits

- Memory: RSS monitoring, process terminated on limit exceeded
- Time: Automatic termination on timeout
- Process count: Fork bomb prevention

### 4. Mandatory Deny Paths (`sandbox/seatbelt.rs`)

**Always-blocked sensitive files:**

| Category | Examples |
|----------|----------|
| Shell configs | `.bashrc`, `.zshrc`, `.profile` |
| Git configs | `.gitconfig`, `.git/hooks/*` |
| IDE configs | `.vscode/settings.json`, `.idea/*` |
| Package managers | `.npmrc`, `.pypirc`, `.cargo/config` |
| Security files | `.ssh/*`, `.gnupg/*`, `.aws/credentials` |
| AI/Agent configs | `.mcp.json`, `.claude/*`, `.cursor/*` |

### 5. Supply Chain Security (`security/dependency_audit.rs`)

Scans Skill dependencies for known vulnerabilities using OSV (Open Source Vulnerabilities) API. Requires `audit` feature.

### 6. Additional Protections

- **Bash Validator** (`bash_validator.rs`): Detects dangerous bash commands
- **File Move Protection** (`move_protection.rs`): Prevents malicious file overwrites of critical paths
- **User Authorization**: Level 3 requires user confirmation when Critical/High severity issues are found before execution

---

## Dependencies

### Rust Dependencies (Cargo.toml)

```toml
[dependencies]
# Core
clap = { version = "4", features = ["derive"] }  # CLI parsing
serde = { version = "1", features = ["derive"] } # Serialization
serde_yaml = "0.9"                               # YAML parsing
serde_json = "1.0"                               # JSON parsing
anyhow = "1.0"                                   # Error handling
thiserror = "..."                                # Typed errors
regex = "1.10"                                   # Regular expressions
tempfile = "3.10"                                # Temporary files
sha2 = "..."                                     # SHA hashing
tracing = "..."                                  # Structured logging
chrono = "..."                                   # Time handling
rayon = "..."                                    # Thread pool
zip = "..."                                      # ZIP extraction

# Optional (feature-gated)
tokio = { ..., optional = true }                 # Async runtime (agent)
reqwest = { ..., optional = true }               # HTTP client (agent)
rusqlite = { ..., optional = true }              # SQLite (executor)
ureq = { ..., optional = true }                  # HTTP (audit)
sqlite-vec = { ..., optional = true }            # Vector search (memory_vector)

# Platform-specific
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.29", features = ["process", "mount", "sched", "signal"] }
libc = "0.2"

[target.'cfg(target_os = "macos")'.dependencies]
nix = { version = "0.29", features = ["process", "signal"] }
```

### Python SDK

Zero runtime dependencies. All operations performed via the bundled skilllite binary.

---

## Anti-Corruption Principles

### Dependency Rules

```
Entry Layer(CLI/MCP/stdio_rpc) → Agent → Executor → Sandbox → Core
Core doesn't depend on upper layers; Agent is Core's customer, not part of Core
```

### Interface First

- Sandbox only depends on `SandboxConfig` struct, not `SkillMetadata` concrete types
- New capabilities are added via "registration", no `if tool_name == "xxx"` hardcoding

### Dependency Discipline

| Layer | Allowed | Forbidden |
|-------|---------|-----------|
| Core | serde, anyhow, regex, dirs | tokio, reqwest, rusqlite |
| Sandbox | core, tempfile, nix | tokio, reqwest |
| Executor | core, rusqlite | tokio |
| Agent | All | — |

---

## Refactoring Guide

### Refactoring the Rust Sandbox

1. **Maintain CLI compatibility**: `run`, `exec`, `scan`, `validate`, `info`, `security-scan`, `bash` commands
2. **Maintain output format**: JSON to stdout on success, errors to stderr
3. **Security level logic**: Level 1 no sandbox / Level 2 isolation only / Level 3 isolation + scanning
4. **Decoupling convention**: Pass `SandboxConfig` and `RuntimePaths` as parameters, no direct upper-layer dependencies

### Adding New Tools

1. Create a module under `agent/extensions/`, implement `tool_definitions()` and execution logic
2. Register the tool in `extensions/registry.rs`
3. Do not modify `agent_loop.rs`

### Supporting New Platform Sandboxes

1. Implement platform module under `sandbox/` (e.g., `landlock.rs`)
2. Select backend by platform in `sandbox/runner.rs`
3. Control compilation via feature flags

---

## Important Notes

1. **Do not modify `.skills/` directory**: These are example Skills; users may have custom content
2. **Maintain backward compatibility**: API changes must consider existing users
3. **Security first**: Any sandbox-related changes require careful review
4. **Cross-platform support**: macOS, Linux, and Windows sandbox implementations differ and must be tested separately
5. **Feature flag discipline**: New modules should clearly belong to a specific feature to avoid unnecessary dependency inclusion

---

*Document version: 1.3.0*
*Last updated: 2026-02-21*
