# SkillLite Project Architecture

> **Note**: This document matches the root `Cargo.toml` `[workspace.package]` version (currently **v0.1.27**). Rust uses a Cargo workspace with modules split into separate crates; the Python SDK is a thin bridge layer (~770 lines of Python under `python-sdk/skilllite/`) exporting `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`, `artifact_put`, `artifact_get`, and helpers.
>
> **Entry points and domains**: For a one-page map of CLI / Python / MCP / Desktop / Swarm (what they are, which crates they use, one-line use case), see [Entry Points and Capability Domains](./ENTRYPOINTS-AND-DOMAINS.md). (中文版：[入口与能力域一览](../zh/ENTRYPOINTS-AND-DOMAINS.md))

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

Rust uses a **Cargo workspace** multi-crate layout; responsibilities are split by crate with dependencies flowing Core → upper layers.

### Workspace and Crate Overview

```
skillLite/
├── Cargo.toml                     # Workspace config (members: skilllite, crates/*)
│
├── skilllite/                     # Main binary (thin entry, CLI dispatch only)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                # Entry point
│       ├── lib.rs
│       ├── error.rs               # skilllite::Error (thiserror; anyhow from deps via Other)
│       ├── cli.rs                 # CLI args
│       ├── protocol.rs            # Protocol definitions
│       ├── command_registry.rs    # Command registration & dispatch
│       ├── stdio_rpc.rs           # Stdio JSON-RPC (skill execution)
│       ├── mcp/                   # MCP protocol server
│       │   ├── mod.rs
│       │   ├── tools.rs
│       │   ├── handlers.rs
│       │   └── scan.rs
│       ├── dispatch/              # Dispatch to skilllite-commands (+ artifact-serve when feature enabled)
│       │   ├── mod.rs
│       │   ├── execute.rs
│       │   ├── skill.rs
│       │   ├── artifact.rs
│       │   └── protocol.rs
│       └── bin/
│           └── skilllite-sandbox.rs  # Lightweight binary (sandbox + core only)
│
├── crates/
│   ├── skilllite-core/            # Core lib: config, paths, Skill metadata, protocol
│   │   └── src/
│   │       ├── config/            # Env loading, schema (LlmConfig, etc.)
│   │       ├── paths.rs
│   │       ├── path_validation.rs # Path validation
│   │       ├── error.rs           # Structured errors (PathValidationError)
│   │       ├── skill/             # SKILL.md parsing, manifest, dependency_resolver
│   │       ├── scan_cache.rs
│   │       ├── planning.rs
│   │       └── protocol.rs
│   │
│   ├── skilllite-fs/              # File system: read/write/grep/search_replace/atomic_write
│   │
│   ├── skilllite-sandbox/         # Sandbox execution (core security)
│   │   └── src/
│   │       ├── runner.rs          # SandboxLevel, SandboxConfig, ResourceLimits
│   │       ├── common.rs
│   │       ├── macos.rs           # macOS Seatbelt
│   │       ├── linux.rs           # Linux Namespace + Seccomp
│   │       ├── windows.rs         # WSL2 bridge
│   │       ├── seatbelt.rs
│   │       ├── seccomp.rs
│   │       ├── network_proxy/      # HTTP/SOCKS5 proxy, config, tunnel, manager
│   │       ├── bash_validator.rs  # Bash command validation (BashValidationError)
│   │       ├── move_protection.rs
│   │       ├── env/               # RuntimePaths construction
│   │       └── security/          # Static scan, rules, dependency_audit
│   │
│   ├── skilllite-executor/        # Session, transcripts, memory (executor feature)
│   │   └── src/
│   │       ├── error.rs           # ExecutorError
│   │       ├── session.rs
│   │       ├── transcript.rs
│   │       ├── memory.rs          # BM25, optional sqlite-vec vector
│   │       ├── plan.rs
│   │       └── rpc.rs
│   │
│   ├── skilllite-evolution/       # Self-evolution: prompts, memory, skills feedback
│   │
│   ├── skilllite-agent/           # Agent loop (agent feature)
│   │   └── src/
│   │       ├── agent_loop/        # mod + planning / execution / reflection / helpers
│   │       ├── chat.rs
│   │       ├── chat_session.rs
│   │       ├── llm/
│   │       ├── prompt.rs
│   │       ├── task_planner.rs
│   │       ├── skills/
│   │       ├── extensions/        # registry, builtin (file_ops, run_command, etc.)
│   │       └── rpc.rs             # Agent RPC (JSON-Lines event stream)
│   │
│   ├── skilllite-commands/        # CLI command implementations
│   │   └── src/
│   │       ├── execute.rs         # run_skill, exec_script, bash_command
│   │       ├── scan.rs            # walkdir recursive + rayon parallel analysis
│   │       ├── security.rs
│   │       ├── skill/             # add, remove, list, show, verify
│   │       ├── ide.rs
│   │       ├── init.rs
│   │       ├── env.rs
│   │       ├── reindex.rs
│   │       ├── evolution.rs
│   │       ├── quickstart.rs
│   │       └── planning_rules_gen.rs
│   │
│   ├── skilllite-swarm/           # P2P (mDNS, task routing; swarm feature)
│   │
│   ├── skilllite-artifact/        # ArtifactStore impls: local dir (default for agent), optional HTTP server/client
│   │
│   └── skilllite-assistant/       # Tauri 2 + React desktop (not in default workspace members; see root Cargo.toml exclude)
│       ├── vite.config.ts         # sole Vite config (do not duplicate vite.config.js; see crate README)
│       └── src-tauri/             # cargo build --manifest-path crates/skilllite-assistant/src-tauri/Cargo.toml
│
├── python-sdk/                    # Python SDK (thin bridge layer)
│   ├── pyproject.toml             # Package config (v0.1.27, zero runtime deps)
│   └── skilllite/
│       ├── __init__.py            # Public exports (API + artifacts helpers)
│       ├── api.py                 # Core API (subprocess calls to skilllite binary)
│       ├── artifacts.py           # artifact_put / artifact_get (OpenAPI v1 HTTP client, stdlib urllib)
│       ├── binary.py              # Binary management (bundled/PATH resolution)
│       ├── cli.py                 # CLI entry (forwards to binary)
│       └── ipc.py                 # IPC client
│
├── langchain-skilllite/           # LangChain adapter (separate package; version in that crate's pyproject.toml)
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

### 1. Crate Dependencies and Architecture

```
skilllite (main binary)
  ├── skilllite-commands
  │     ├── skilllite-core, skilllite-fs, skilllite-sandbox
  │     └── skilllite-agent (agent feature)
  │           ├── skilllite-core, skilllite-evolution, skilllite-fs
  │           ├── skilllite-sandbox, skilllite-executor
  │           └── skilllite-executor → skilllite-core, skilllite-fs
  ├── skilllite-swarm (swarm feature) → skilllite-core
  ├── skilllite-artifact → skilllite-core (agent: `local` only; main `skilllite` binary: `local` + `server` when `artifact_http` is enabled — default-on)
  └── skilllite-core (root)

Execution chain: CLI/MCP/stdio_rpc → skilllite-commands → skilllite-agent → skilllite-executor → skilllite-sandbox → skilllite-core
Core doesn't depend on upper layers; Agent is Core's customer.
```

**CI / local guard**: Root `deny.toml` configures [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) `bans` so only allowed workspace crates may *directly* depend on each upper-layer `skilllite-*` crate (see `spec/architecture-boundaries.md`). Run `cargo deny check bans` from the repo root (CI runs this on every PR).

**Feature Flags**:

| Feature | Included Crate | Build Target |
|---------|----------------|--------------|
| `sandbox` (default) | skilllite-sandbox | Sandbox core |
| `audit` (default) | dependency_audit (OSV API) | Supply chain audit |
| `executor` | skilllite-executor | Session, transcript, memory |
| `agent` (default) | skilllite-agent | chat, planning, extensions |
| `sandbox_binary` | skilllite-sandbox + skilllite-core | skilllite-sandbox lightweight binary |
| `memory_vector` | sqlite-vec | Optional semantic search |
| `swarm` | skilllite-swarm | P2P networking |
| `artifact_http` (default) | skilllite-artifact (`local` + `server` in main binary) | Run-scoped artifact HTTP (`skilllite artifact-serve`; **bind** gated by `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`) |

**Artifact implementations (`skilllite-artifact`)**:

- Crate: `skilllite-artifact` (depends only on `skilllite-core`). Features: `local` (filesystem store), `server` (Axum), `client` (blocking HTTP).
- **`skilllite-agent`** depends on `skilllite-artifact` with `default-features = false, features = ["local"]` so the agent crate does not pull Axum into its default build.
- The **`skilllite`** binary enables `artifact_http` by default and depends on `skilllite-artifact` with `features = ["local", "server"]` so `skilllite artifact-serve` is compiled in; listening still requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`.
- OpenAPI (HTTP): [`docs/openapi/artifact-store-http-v1.yaml`](../openapi/artifact-store-http-v1.yaml).
- Embedders can expose `GET`/`PUT` `/v1/runs/{run_id}/artifacts?key=...` via `artifact_router`, or call the same API from other languages.
- `run_artifact_http_server` refuses **non-loopback** binds without a bearer token unless `SKILLLITE_ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH=1` (emits a high-severity warning); loopback-without-token only **warns**. Optional `SKILLLITE_ARTIFACT_HTTP_REQUIRE_AUTH=1` requires a token even on loopback — see [`ENV_REFERENCE.md`](./ENV_REFERENCE.md).
- HTTP `PUT` bodies are capped at **`MAX_ARTIFACT_BODY_BYTES` (64 MiB)** via Axum `DefaultBodyLimit`; larger requests get **413**. The crate uses `skilllite_artifact::Error` / `skilllite_artifact::Result` for serve and client construction (`Other(#[from] anyhow::Error)` per workspace conventions).

**Build Targets**:
- `cargo build -p skilllite`: Full product (`artifact-serve` is compiled in; **bind** requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`)
- `cargo build -p skilllite --no-default-features --features sandbox_binary`: skilllite-sandbox lightweight binary

### 2. Sandbox Module (skilllite-sandbox)

#### 2.1 Sandbox Security Levels (`crates/skilllite-sandbox/src/runner.rs`)

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

The sandbox does not depend on skilllite-core's skill module; it receives `SandboxConfig` constructed by skilllite-commands from `SkillMetadata`.

#### 2.3 RuntimePaths (Decoupled sandbox from env)

```rust
pub struct RuntimePaths {
    pub python: PathBuf,
    pub node: PathBuf,
    pub node_modules: PathBuf,
    pub env_dir: PathBuf,
}
```

The sandbox receives `RuntimePaths` constructed by `skilllite-sandbox::env::builder` or skilllite-commands callers.

#### 2.4 Resource Limits (`runner.rs`)

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,   // Default 256MB
    pub timeout_secs: u64,    // Default 30 seconds
}
```

**Environment Variables:**
- Sandbox-related vars are read via **config** (`SandboxEnvConfig::from_env()`); use `SKILLLITE_*` (legacy `SKILLBOX_*` still accepted):
- `SKILLLITE_SANDBOX_LEVEL`: Sandbox level (1/2/3)
- `SKILLLITE_MAX_MEMORY_MB`: Maximum memory limit
- `SKILLLITE_TIMEOUT_SECS`: Execution timeout
- `SKILLLITE_AUTO_APPROVE`: Auto-approve dangerous operations

#### 2.5 macOS Sandbox (`skilllite-sandbox/macos.rs`)

**Core Technology**: Uses macOS `sandbox-exec` with Seatbelt profiles

**Execution Flow:**
1. Check if sandbox is disabled (`SKILLLITE_NO_SANDBOX`)
2. Start network proxy (if networking enabled with domain whitelist)
3. Generate Seatbelt profile (restrict filesystem, network access)
4. Launch child process via `sandbox-exec`
5. Monitor memory usage and execution time
6. Terminate process on limit exceeded

#### 2.6 Linux Sandbox (`skilllite-sandbox/linux.rs`)

**Sandbox Tool Priority**: bubblewrap (bwrap) → firejail. If both are unavailable or execution fails, **execution is refused by default** (fail-closed, aligned with Windows). Only with `SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK=1` is a **weak** fallback allowed (PID/UTS/network namespaces only, no bwrap filesystem sandbox), recorded as a security event (`security_sandbox_fallback` / `linux_namespace_fallback`).

**Bubblewrap Isolation:**
- `--unshare-all`: Unshare all namespaces
- Minimal filesystem mounts (read-only /usr, /lib, /bin)
- Skill directory mounted read-only
- Network isolation (default `--unshare-net`; `--share-net` with proxy filtering when enabled)
- Seccomp BPF filter blocks AF_UNIX socket creation

#### 2.7 Windows Sandbox (`skilllite-sandbox/windows.rs`)

Sandbox functionality implemented via WSL2 bridge.

#### 2.8 Network Proxy (`skilllite-sandbox/network_proxy/`)

Provides HTTP and SOCKS5 proxy for domain whitelist filtering. When a skill declares network access with restricted outbound domains, the proxy intercepts non-whitelisted requests.

#### 2.9 Static Code Scanning (`skilllite-sandbox/security/`)

The security scanning module contains:

| File | Responsibility |
|------|---------------|
| `scanner.rs` | Scanner main logic (ScriptScanner) |
| `rules.rs` | Security rule definitions and matching |
| `types.rs` | Security type definitions |
| `policy.rs` | Runtime security policy (path/process/network) |
| `default_rules.rs` | Default rule implementations |
| `default_rules.yaml` | Configurable rules file |
| `dependency_audit/` | Supply chain vulnerability scanning (OSV API, requires audit feature) |

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
| `bash_validator.rs` | Bash command validation (BashValidationError) |
| `move_protection.rs` | File move protection |
| `seatbelt.rs` | macOS mandatory deny paths and Seatbelt profiles |

---

### 3. Executor Module (skilllite-executor)

> Requires `executor` feature. Provides session, transcript, and memory storage.

| Module | Responsibility |
|--------|---------------|
| `session.rs` | Session lifecycle management |
| `transcript.rs` | Conversation transcript persistence |
| `memory.rs` | Memory storage (BM25 retrieval, optional sqlite-vec vector search) |
| `rpc.rs` | Executor RPC interface |

**Data Storage Path**: `~/.skilllite/` (chat records, session data, memory indices)

---

### 4. Agent Module (skilllite-agent)

> Requires `agent` feature (enabled by default). Provides chat, planning, tools, and extensions.

#### 4.1 Core Modules

| Module | Responsibility |
|--------|---------------|
| `chat.rs` | CLI chat entry (single `--message` / interactive REPL) |
| `agent_loop/` | Agent main loop (`mod` orchestration; `planning` / `execution` / `reflection` / `helpers` submodules) |
| `llm/` | LLM HTTP client (OpenAI-compatible API, Claude Native API, streaming/non-streaming) |
| `chat_session.rs` | Chat session management |
| `prompt.rs` | System prompt construction |
| `skills/` | Skill loading, execution, and tool definition generation |
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

**Evolution extracted knowledge** (workspace `memory/evolution/`): five dimensions with monthly `YYYY-MM.md` shards per folder; after each memory-evolution write, the engine regenerates **`YYYY-MM.rollup.md`** deduplicated summaries (append-only shard content is unchanged). Rollups are indexed for BM25 like other evolution markdown.

---

### 5. MCP Module (skilllite/mcp/)

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

### 6. Stdio RPC Module (skilllite/stdio_rpc.rs)

**Skill Execution Stdio RPC**: JSON-RPC 2.0 over stdio (one request per line)

Uses rayon thread pool for concurrent request processing. Supported methods: `run`, `exec`, `bash`, `scan`, `validate`, `info`, etc.

Separate from `skilllite-agent::rpc` — the latter is dedicated to Agent Chat streaming events.

---

### 7. Python SDK (python-sdk)

> **Note**: The Python SDK is a thin bridge layer, **zero PyPI runtime dependencies**. Sandbox/chat APIs call the skilllite binary via subprocess (or IPC). **Artifact** helpers use stdlib `urllib` against the HTTP API (`skilllite artifact-serve` with **`SKILLLITE_ARTIFACT_SERVE_ALLOW=1`**, or any compatible server).

**Modules:**

| Module | Responsibility |
|--------|---------------|
| `api.py` | `scan_code`, `execute_code`, `chat`, `run_skill` via subprocess calls to skilllite binary |
| `artifacts.py` | `artifact_put`, `artifact_get` — OpenAPI v1 client for run-scoped artifact HTTP |
| `binary.py` | Binary management: `get_binary`, bundled/PATH resolution |
| `cli.py` | CLI entry, forwards to binary |
| `ipc.py` | IPC client, communicates with `skilllite serve` daemon |

**Exported API**: `scan_code`, `execute_code`, `chat`, `run_skill`, `get_binary`, `artifact_put`, `artifact_get`, `ArtifactHttpError`, `parse_listen_line`

**Programmatic Agent**: Use `skilllite chat --message` or `api.chat()` to invoke the Rust Agent loop.

---

### 8. LangChain Integration (langchain-skilllite)

> Separate package `pip install langchain-skilllite`; version is defined in `langchain-skilllite/pyproject.toml` (released independently from the main repo `skilllite` PyPI package).

| Module | Responsibility |
|--------|---------------|
| `core.py` | SkillManager, SkillInfo — Skill scanning and management |
| `tools.py` | SkillLiteTool, SkillLiteToolkit — LangChain tool adapters |
| `callbacks.py` | Callback handler |

**Dependencies** (see that package's `pyproject.toml`): `langchain-core>=0.3.0`, `skilllite>=0.1.8`

---

### 9. Skill Metadata Parsing (`skilllite-core/skill/`)

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
skilllite artifact-serve                       # Run-scoped artifact HTTP (bind requires SKILLLITE_ARTIFACT_SERVE_ALLOW=1)
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
SKILLLITE_SANDBOX_LEVEL=3     # 1/2/3
SKILLLITE_MAX_MEMORY_MB=256   # Memory limit
SKILLLITE_TIMEOUT_SECS=30     # Timeout
SKILLLITE_AUTO_APPROVE=false  # Auto-approve dangerous operations
SKILLLITE_NO_SANDBOX=false    # Disable sandbox
```

Environment variable keys are defined in `skilllite-core/config/env_keys.rs` with legacy compatibility. Config source priority: CLI/explicit args > env vars > .env file > defaults. See [ENV_REFERENCE](./ENV_REFERENCE.md#config-source-priority).

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

### 4. Mandatory Deny Paths (`skilllite-sandbox/seatbelt.rs`)

**Always-blocked sensitive files:**

| Category | Examples |
|----------|----------|
| Shell configs | `.bashrc`, `.zshrc`, `.profile` |
| Git configs | `.gitconfig`, `.git/hooks/*` |
| IDE configs | `.vscode/settings.json`, `.idea/*` |
| Package managers | `.npmrc`, `.pypirc`, `.cargo/config` |
| Security files | `.ssh/*`, `.gnupg/*`, `.aws/credentials` |
| AI/Agent configs | `.mcp.json`, `.claude/*`, `.cursor/*` |

### 5. Supply Chain Security (`skilllite-sandbox/security/dependency_audit/`)

Scans Skill dependencies for known vulnerabilities using OSV (Open Source Vulnerabilities) API. Requires `audit` feature.

### 6. Additional Protections

- **Bash Validator** (`bash_validator.rs`): Detects dangerous bash commands
- **File Move Protection** (`move_protection.rs`): Prevents malicious file overwrites of critical paths
- **User Authorization**: Level 3 runs a unified precheck (`SKILL.md` + entry script); if the precheck produces a review report (including medium script findings, SKILL.md alerts, or scan errors), the runner (CLI) or host (agent/MCP) requires explicit consent before execution. `SKILLLITE_AUTO_APPROVE` applies to the same gate for TTY/CLI runs.

### 7. Swarm HTTP API (`skilllite swarm`)

- **Default bind**: `127.0.0.1:<port>` (loopback only). Use `--listen 0.0.0.0:<port>` when other machines must connect.
- **mDNS**: Registration is skipped when the bind address is loopback-only (LAN mesh requires an all-interfaces listen address).
- **Authentication**: Optional shared secret `SKILLLITE_SWARM_TOKEN`. When set, `GET /status`, `GET /can-do`, and `POST /task` require `Authorization: Bearer <token>`. Peer forwarding and `delegate_to_swarm` send the same header if the variable is set. If you listen on all interfaces without a token, startup logs a warning.

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
rayon = "..."                                    # Thread pool (stdio_rpc, skilllite-commands scan)
walkdir = "2.5"                                  # Recursive dir walk (skilllite-commands scan)
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

1. Create a module under `skilllite-agent/extensions/`, implement `tool_definitions()` and execution logic
2. Register the tool in `extensions/registry.rs`
3. Do not wire new tools by editing `agent_loop/mod.rs` core orchestration; add capabilities via `extensions/`

### Supporting New Platform Sandboxes

1. Implement platform module under `crates/skilllite-sandbox/src/` (e.g., `landlock.rs`)
2. Select backend by platform in `runner.rs`
3. Control compilation via feature flags

---

## Important Notes

1. **Do not modify `.skills/` directory**: These are example Skills; users may have custom content
2. **Maintain backward compatibility**: API changes must consider existing users
3. **Security first**: Any sandbox-related changes require careful review
4. **Cross-platform support**: macOS, Linux, and Windows sandbox implementations differ and must be tested separately
5. **Feature flag discipline**: New modules should clearly belong to a specific feature to avoid unnecessary dependency inclusion

---

*Document version: 1.4.2*
*Last updated: 2026-04-08*
