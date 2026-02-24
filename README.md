# SkillLite

[中文文档](./docs/zh/README.md)

**A lightweight AI Agent Skills secure engine with built-in native system-level sandbox, zero dependencies, and local execution.**

A lightweight AI Agent Skills secure execution engine that integrates with any OpenAI-compatible LLM.


## Architecture: Two Layers

```
┌────────────────────────────────────────────────────┐
│  Agent Layer (optional)                            │
│  Built-in chat, planning, memory, tools            │
│  Binary: skilllite (full)                          │
├────────────────────────────────────────────────────┤
│  Core Layer                                        │
│  Sandbox + security scan + skills management + MCP │
│  Binary: skilllite-sandbox (lightweight)           │
└────────────────────────────────────────────────────┘
```

## ⚡ Performance Benchmark

See SkillLite's performance compared to other sandbox solutions in real-time:

[![Performance Benchmark Video](https://github.com/EXboys/skilllite/raw/main/docs/images/benchmark-en.gif)]

![Performance Benchmark Comparison](./docs/images/benchmark-en.png)

### Running Benchmarks

```bash
# From project root
python benchmark/benchmark_runner.py --compare-levels --compare-ipc -n 100 -c 10

# Cold start comparison (outputs COLD START BENCHMARK COMPARISON table)
python benchmark/benchmark_runner.py --cold-start --compare-ipc

# Full test: cold start + high concurrency
python benchmark/benchmark_runner.py --cold-start --cold-iterations 20 --compare-levels --compare-ipc -o results.json
```

See [benchmark/README.md](./benchmark/README.md) for full documentation.


## 🎯 Why SkillLite?

| Feature | SkillLite | Claude Code Sandbox | Pyodide  | OpenAI Plugins | Semantic Kernel |
|---------|-----------|---------------------|-------------------|----------------|-----------------|
| **Built-in Sandbox** | ✅ Rust Native | ✅ Node.js Native | ⚠️ Pyodide/Docker | ⚠️ Cloud (Closed) | ❌ None (Azure) |
| **Sandbox Tech** | Seatbelt + Namespace | Seatbelt + bubblewrap | WebAssembly/Docker | Cloud Isolation | - |
| **Implementation** | **Rust** (High Perf) | Node.js/TypeScript | Python | - | C# |
| **Local Execution** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Zero Dependencies** | ✅ Single Binary | ❌ Needs Node.js | ❌ Needs Runtime | ❌ | ❌ |
| **Cold Start** | ⚡ Milliseconds | Medium | 🐢 Seconds | - | - |
| **LLM Agnostic** | ✅ Any LLM | ❌ Claude Only | ✅ | ❌ OpenAI Only | ✅ |
| **License** | MIT | Apache 2.0 | MIT | Closed | MIT |



> **Performance Highlights**: SkillLite achieves **3-5x faster** execution than Docker and SRT, with **10x lower memory footprint** (~10MB vs ~100MB).

## 🚀 Quick Start

### Installation (Recommended: pip)

```bash
# Install SkillLite SDK
pip install skilllite

# Initialize project (sandbox binary + .skills/ + download skills from EXboys/skilllite)
skilllite init

# Verify installation
skilllite list

```

### Skill Repository Management

```bash
# Add skills from remote repositories
skilllite add owner/repo                    # Add all skills from a GitHub repo
skilllite add owner/repo/skill-name         # Add a specific skill by path
skilllite add owner/repo@skill-name         # Add a specific skill by name filter
skilllite add https://github.com/owner/repo # Add from full GitHub URL
skilllite add ./local-path                  # Add from local directory
skilllite add owner/repo --list             # List available skills without installing
skilllite add owner/repo --force            # Force overwrite existing skills

# Manage installed skills
skilllite list                              # List all installed skills
skilllite remove <skill-name>               # Remove an installed skill
skilllite remove <skill-name> --force       # Remove without confirmation
```

That's it! No Rust, no Docker, no complex setup required.

**Zero-config quick start** (auto-detect LLM, setup skills, launch chat):

```bash
skilllite quickstart
```

> **Platform Support**: macOS, Linux, and Windows (via WSL2 Bridge).

## 📚 Tutorials

| Tutorial | Time | Description |
|----------|------|-------------|
| [01. Basic Usage](./tutorials/01_basic) | 5 min | Simplest examples, one-line execution |
| [02. Skill Management](./tutorials/02_skill_management) | 10 min | Create and manage skills |
| [03. Agentic Loop](./tutorials/03_agentic_loop) | 15 min | Multi-turn conversations and tool calls |
| [04. LangChain Integration](./tutorials/04_langchain_integration) | 15 min | Integration with LangChain framework |
| [05. LlamaIndex Integration](./tutorials/05_llamaindex_integration) | 15 min | RAG + skill execution |
| [06. MCP Server](./tutorials/06_mcp_server) | 10 min | Claude Desktop integration |
| [07. OpenCode Integration](./tutorials/07_opencode_integration) | 10 min | One-command OpenCode integration |

### Run Your First Example

```python
from skilllite import chat

# Uses .env for API config, .skills for tools
result = chat("Calculate 15 * 27", skills_dir=".skills")
print(result)
```

Or use the CLI for interactive chat: `skilllite chat`

### Environment Configuration

```bash
# Copy the template and fill in your LLM API credentials
cp .env.example .env
# Edit .env: BASE_URL, API_KEY, MODEL
```

| File | Description |
|------|-------------|
| [.env.example](./.env.example) | Quick start template (5-8 common variables) |
| [.env.example.full](./.env.example.full) | Full variable list (advanced users) |
| [docs/en/ENV_REFERENCE.md](./docs/en/ENV_REFERENCE.md) | Complete reference: defaults, usage scenarios |



👉 **[View All Tutorials](./tutorials/README.md)**

## Security Comparison Test

In addition to performance tests, we provide security comparison tests to evaluate the protection capabilities of sandbox solutions against malicious behavior.

### Test Dimensions

| Category | Test Item | Description |
|------|--------|------|
| **File System** | Read sensitive files | `/etc/passwd`, `~/.ssh/id_rsa` |
| | Write files | Try to create files outside sandbox |
| | Directory traversal | `../../../` path traversal attacks |
| **Network** | HTTP requests | External network access capability |
| | DNS queries | Domain name resolution capability |
| | Port listening | Open socket services |
| **Process** | System commands | `os.system()`, `subprocess` |
| | Process enumeration | View other process information |
| | Signal sending | Try to kill other processes |
| **Resource Limits** | Memory bomb | Infinite memory allocation |
| | Fork bomb | Infinite process creation |
| | CPU bomb | Infinite loop calculation |
| **Code Injection** | Dynamic import | `__import__`, `importlib` |
| | eval/exec | Dynamic code execution |

### Security Comparison 

| Test Item               |   SkillLite    |     Docker     |    Pyodide     |   Claude SRT   |
|----------------------|----------------|----------------|----------------|----------------|
| **File System** | | | | |
| Read /etc/passwd       |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| Read SSH private key    |      ✅ Blocked      |      ✅ Blocked      |      ✅ Blocked      |      ❌ Allowed      |
| Write to /tmp dir       |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Directory traversal     |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| List root directory     |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| **Network** | | | | |
| Send HTTP request       |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ✅ Blocked      |
| DNS query              |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Listen port             |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| **Process** | | | | |
| Execute os.system()    |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Execute subprocess     |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| Enumerate processes    |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ✅ Blocked      |
| Send process signal    |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |    ⚠️ Partially Blocked     |
| **Resource Limits** | | | | |
| Memory bomb             |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Fork bomb              |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |      ❌ Allowed      |
| CPU intensive compute  |      ✅ Blocked      |      ✅ Blocked      |      ❌ Allowed      |      ✅ Blocked      |
| **Code Injection** | | | | |
| Dynamic import os      |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Use eval/exec          |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Modify built-in funcs  |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| **Information Leakage** | | | | |
| Read environment vars  |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |
| Get system info        |      ✅ Blocked      |      ❌ Allowed      |      ❌ Allowed      |      ❌ Allowed      |

#### Security Scores

| Platform | Blocked | Partially Blocked | Allowed | Security Score |
|------|------|----------|------|----------|
| SkillLite | 18 | 0 | 2 | 90.0% |
| Docker | 2 | 0 | 18 | 10.0% |
| Pyodide | 7 | 0 | 13 | 35.0% |
| Claude SRT | 6 | 1 | 13 | 32.5% |

### Running Security Tests

```bash
# Complete test (SkillLite + Docker + Pyodide + Claude SRT)
python3 benchmark/security_vs.py

# Test SkillLite only
python3 benchmark/security_vs.py --skip-docker --skip-pyodide --skip-claude-srt

# Output JSON results
python3 benchmark/security_vs.py --output security_results.json
```

---

## Comprehensive Comparison Summary

| Dimension | SkillLite | Docker | Pyodide | SRT |
|------|----------|--------|---------|-----|
| **Warm Start Latency** | 40 ms | 194 ms | 672 ms | 596 ms |
| **Cold Start Latency** | 492 ms | 120s | ~5s | ~1s |
| **Memory Usage** | 10 MB | ~100 MB | ~50 MB | 84 MB |
| **Security** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **Deployment Complexity** | Single binary | Requires daemon | Requires Node.js | Requires installation |
| **Platform Support** | macOS/Linux/Win(WSL2) | All platforms | All platforms | macOS/Linux |

**Note**: gVisor runs ON TOP OF Docker (using `--runtime=runsc`), so its performance will always be worse than Docker. It's only useful for security isolation comparison, not performance benchmarking.

---

### Comparison with Claude Code Sandbox

Claude/Anthropic released [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing) in October 2025, using the **same underlying technology stack** as SkillLite:
- **macOS**: Seatbelt (sandbox-exec)
- **Linux**: bubblewrap + namespace


### Security Features

| Security Capability | Description |
|--------------------|-------------|
| **Process Isolation** | Each Skill runs in an independent process |
| **Filesystem Isolation** | Only Skill directory and temp directory accessible |
| **Network Isolation** | Network disabled by default, can be enabled on demand |
| **Resource Limits** | CPU, memory, execution time limits |
| **Least Privilege** | Follows the principle of least privilege |

## ✨ Features

- **🔒 Native Security Sandbox** - Rust-implemented system-level isolation, not Docker/WebAssembly
- **⚡ Ultra Lightweight** - Single binary, millisecond cold start, zero external dependencies
- **🏠 Data Sovereignty** - Pure local execution, code and data never leave your machine
- **🔌 Universal LLM Support** - Compatible with all OpenAI API format LLM providers
- **📦 Skills Management** - Auto-discovery, registration, and management of Skills
- **🧠 Smart Schema Inference** - Automatically infer input parameter Schema from SKILL.md and script code
- **🔧 Tool Calls Handling** - Seamlessly handle LLM tool call requests
- **📄 Rich Context Support** - Support for references, assets, and other extended resources



## 🔧 Alternative: Build from Source

<details>
<summary>Click to expand (for contributors or custom builds)</summary>

### Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Build & Install Commands (from repository root)

| Package | Binary | Command | Description |
|---------|--------|---------|-------------|
| skilllite | **skilllite** | `cargo build -p skilllite` | **Full** (Agent + Chat + MCP + sandbox + audit). No vector memory. |
| skilllite | **skilllite** | `cargo build -p skilllite --features memory_vector` | Full **+ vector memory** search |
| skilllite | **skilllite** | `cargo build -p skilllite --no-default-features` | Minimal: run/exec/bash/scan only, no Agent |
| skilllite | **skilllite-sandbox** | `cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | Sandbox + MCP only |

### Install (to `~/.cargo/bin/`)

| Command | What you get |
|---------|--------------|
| `cargo install --path skilllite` | **skilllite** — full (Agent + Chat + MCP + sandbox + audit). **Does NOT include vector memory.** |
| `cargo install --path skilllite --features memory_vector` | **skilllite** — full + vector memory search (semantic `memory_search`) |
| `cargo install --path skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | **skilllite-sandbox** — sandbox + MCP only, no Agent |

**Default features** = `sandbox`, `audit`, `agent`. Vector memory (`memory_vector`) is **not** in default.

### From `skilllite/` subdir

```bash
cd skilllite && cargo install --path .                              # full, no vector
cd skilllite && cargo install --path . --features memory_vector     # full + vector
```

### Script (avoids copy-paste issues)

If you get `command not found: cargo install` when pasting, the clipboard may have invisible Unicode. Use:

```bash
./scripts/install-from-source.sh
```

This installs full + vector memory. Run from repository root.

### Output paths

- `cargo install`: `~/.cargo/bin/skilllite` or `~/.cargo/bin/skilllite-sandbox`
- `cargo build`: `target/release/skilllite` or `target/release/skilllite-sandbox`

### Desktop Assistant (skilllite-assistant)

Tauri 2 + React Desktop，位于 `crates/skilllite-assistant/`：

```bash
cd crates/skilllite-assistant
npm install
npm run tauri dev    # dev mode（HMR）
npm run tauri build  
```

详见 [crates/skilllite-assistant/README.md](./crates/skilllite-assistant/README.md)。

### Project Structure (Cargo Workspace)

Standard Rust multi-crate layout; root `Cargo.toml` declares the workspace:

```
skilllite/
├── Cargo.toml              # [workspace] members
├── skilllite/              # Main binary (CLI entry point)
└── crates/
    ├── skilllite-assistant/ # Desktop app (Tauri + React)
    ├── skilllite-core/     # Config, skill metadata, path validation
    ├── skilllite-sandbox/  # Sandbox executor (independently deliverable)
    ├── skilllite-executor/ # Session, transcript, memory
    └── skilllite-agent/    # LLM Agent loop, tool extensions
```

Dependency direction: `skilllite → agent → sandbox + executor → core`. See [ARCHITECTURE.md](./docs/en/ARCHITECTURE.md).

</details>


## 💡 Usage

### Basic Usage (chat API)

```python
from skilllite import chat

# Single-shot agent chat (uses .env for API config)
result = chat("Calculate 15 times 27", skills_dir=".skills")
print(result)
```

### Direct Skill Execution

```python
from skilllite import run_skill

result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

### Framework Integration (LangChain / LlamaIndex)

For LangChain or LlamaIndex agents, use the dedicated adapters:

```bash
pip install langchain-skilllite   # LangChain
```

See [Framework Adapters](#framework-adapters) below.

### Supported LLM Providers

| Provider | base_url |
|----------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot | `https://api.moonshot.cn/v1` |
| Ollama (Local) | `http://localhost:11434/v1` |

## 🛠️ Create Custom Skill

Each Skill is a directory containing a `SKILL.md`:

```
my-skill/
├── SKILL.md           # Skill metadata and description (required)
├── scripts/           # Scripts directory
│   └── main.py        # Entry script
├── references/        # Reference documents (optional)
└── assets/            # Resource files (optional)
```

### SKILL.md Example

```markdown
---
name: my-skill
description: My custom Skill that does something useful.
license: MIT
compatibility: Requires Python 3.x with requests library, network access
metadata:
  author: your-name
  version: "1.0"
---

# My Skill

This is the detailed description of the Skill.

## Input Parameters

- `query`: Input query string (required)

## Output Format

Returns JSON result.
```

> **Note**: Dependencies are declared in the `compatibility` field (not `requirements.txt`). Entry point is auto-detected (`main.py` > `main.js` > `main.ts` > `main.sh`).

## Framework Adapters

SkillLite provides adapters for popular AI frameworks with security confirmation support.

### LangChain Integration

Install the [langchain-skilllite](https://pypi.org/project/langchain-skilllite/) package:

```bash
pip install langchain-skilllite
```

```python
from langchain_skilllite import SkillLiteToolkit
from langchain_openai import ChatOpenAI
from langgraph.prebuilt import create_react_agent

# Load skills as LangChain tools
tools = SkillLiteToolkit.from_directory("./skills")

# With security confirmation (sandbox_level=3)
def confirm_execution(report: str, scan_id: str) -> bool:
    print(report)
    return input("Continue? [y/N]: ").lower() == 'y'

tools = SkillLiteToolkit.from_directory(
    "./skills",
    sandbox_level=3,  # 1=no sandbox, 2=sandbox only, 3=sandbox+scan
    confirmation_callback=confirm_execution
)

# Use with LangChain agent
agent = create_react_agent(ChatOpenAI(model="gpt-4"), tools)
result = agent.invoke({"messages": [("user", "Calculate 15 * 27")]})
```

### LlamaIndex Integration

See [05. LlamaIndex Integration](./tutorials/05_llamaindex_integration/README.md) for setup and usage.

### Security Levels

| Level | Description |
|-------|-------------|
| 1 | No sandbox - direct execution |
| 2 | Sandbox isolation only |
| 3 | Sandbox + static security scan (requires confirmation for high-severity issues) |

## OpenCode Integration

SkillLite can be integrated with [OpenCode](https://github.com/opencode-ai/opencode) as an MCP (Model Context Protocol) server, providing secure sandbox execution capabilities.

### Quick Setup

```bash
# Install SkillLite (MCP server is built-in)
pip install skilllite

# One-command setup for OpenCode
skilllite init-opencode

# Start OpenCode
opencode
```

The `init-opencode` command automatically:
- Detects the best way to start the MCP server (uvx, pipx, skilllite, or python)
- Creates `opencode.json` with optimal configuration
- Generates `.opencode/skills/skilllite/SKILL.md` with usage instructions
- Discovers your pre-defined skills


## 📦 Core Components

- **skilllite** (Rust binary) - Sandbox executor, CLI, Agent loop, MCP server — single binary with everything (workspace: `skilllite/` + `crates/*`)
- **python-sdk** (`pip install skilllite`) - Thin bridge (~600 lines), zero runtime deps, calls Rust binary via subprocess
- **langchain-skilllite** (`pip install langchain-skilllite`) - LangChain adapter (SkillLiteToolkit)

### Key CLI Commands

| Command | Description |
|--------|-------------|
| `skilllite init` | Initialize project (.skills/ + download skills + dependencies + audit) |
| `skilllite quickstart` | Zero-config: detect LLM, setup skills, launch chat |
| `skilllite chat` | Interactive agent chat (or `--message` for single-shot) |
| `skilllite add owner/repo` | Add skills from GitHub |
| `skilllite remove <name>` | Remove an installed skill |
| `skilllite list` | List installed skills |
| `skilllite show <name>` | Show skill details |
| `skilllite run <dir> '<json>'` | Execute a skill directly |
| `skilllite scan <dir>` | Scan skill for security issues |
| `skilllite mcp` | Start MCP server (Cursor/Claude Desktop) |
| `skilllite serve` | Start IPC daemon (stdio JSON-RPC) |
| `skilllite init-cursor` | Initialize Cursor IDE integration |
| `skilllite init-opencode` | Initialize OpenCode integration |
| `skilllite clean-env` | Clean cached runtime environments |
| `skilllite reindex` | Re-index all installed skills |

## 📄 License

MIT

This project includes third-party dependencies with various licenses. See [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) for details.

## 📚 Documentation

- [Getting Started](./docs/en/GETTING_STARTED.md) - Installation and quick start guide
- [Environment Variables Reference](./docs/en/ENV_REFERENCE.md) - Complete env var documentation
- [Architecture](./docs/en/ARCHITECTURE.md) - Project architecture and design
- [Contributing Guide](./docs/en/CONTRIBUTING.md) - How to contribute
