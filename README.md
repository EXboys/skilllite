# SkillLite

[中文文档](./README_CN.md)

**The  lightweight AI Agent Skills engine with built-in native system-level sandbox, zero dependencies, and local execution.**

A lightweight AI Agent Skills execution engine that integrates with any OpenAI-compatible LLM.


## ⚡ Performance Benchmark

See SkillLite's performance compared to other sandbox solutions in real-time:

| [![Performance Benchmark Video](https://img.youtube.com/vi/yfTxjCj8I4Q/0.jpg)](https://www.youtube.com/watch?v=yfTxjCj8I4Q) | ![Performance Benchmark Comparison](./docs/images/benchmark-en.png) |
|:---:|:---:|
| [Video Demo](https://www.youtube.com/watch?v=yfTxjCj8I4Q) | Static Image |


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

# Install the sandbox binary and skills directory and  skills files
skilllite init

# Verify installation
skilllite status

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

> ⚠️ **Platform Support**: macOS and Linux only. Windows is not supported yet.

## 📚 Tutorials

| Tutorial | Time | Description |
|----------|------|-------------|
| [01. Basic Usage](./tutorials/01_basic) | 5 min | Simplest examples, one-line execution |
| [02. Skill Management](./tutorials/02_skill_management) | 10 min | Create and manage skills |
| [03. Agentic Loop](./tutorials/03_agentic_loop) | 15 min | Multi-turn conversations and tool calls |
| [04. LangChain Integration](./tutorials/04_langchain_integration) | 15 min | Integration with LangChain framework |
| [05. LlamaIndex Integration](./tutorials/05_llamaindex_integration) | 15 min | RAG + skill execution |
| [06. MCP Server](./tutorials/06_mcp_server) | 10 min | Claude Desktop integration |
| [07. OpenCode Integration](./tutorials/07_opencode_integration) | 5 min | One-command OpenCode integration |

### Run Your First Example

```python
from skilllite import SkillRunner

runner = SkillRunner()
result = runner.run("Calculate 15 * 27")
print(result)
```

### Environment Configuration

```bash
# Create .env file with your LLM API configuration
cat > .env << EOF
BASE_URL=https://api.deepseek.com/v1
API_KEY=your_api_key_here
MODEL=deepseek-chat
EOF
```



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

| Test Item               |    SkillBox    |     Docker     |    Pyodide     |   Claude SRT   |
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
| SkillBox | 18 | 0 | 2 | 90.0% |
| Docker | 2 | 0 | 18 | 10.0% |
| Pyodide | 7 | 0 | 13 | 35.0% |
| Claude SRT | 6 | 1 | 13 | 32.5% |

### Running Security Tests

```bash
# Complete test (SkillBox + Docker + Pyodide)
python3 benchmark/security_vs.py

# Test SkillBox only
python3 benchmark/security_vs.py --skip-docker --skip-pyodide

# Output JSON results
python3 benchmark/security_vs.py --output security_results.json
```

---

## Comprehensive Comparison Summary

| Dimension | SkillBox | Docker | Pyodide | SRT |
|------|----------|--------|---------|-----|
| **Warm Start Latency** | 40 ms | 194 ms | 672 ms | 596 ms |
| **Cold Start Latency** | 492 ms | 120s | ~5s | ~1s |
| **Memory Usage** | 10 MB | ~100 MB | ~50 MB | 84 MB |
| **Security** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **Deployment Complexity** | Single binary | Requires daemon | Requires Node.js | Requires installation |
| **Platform Support** | macOS/Linux | All platforms | All platforms | macOS/Linux |

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

### Compile the Sandbox Executor

```bash
cd skillbox
cargo build --release
cargo install --path .
skillbox --help
```

After compilation, the binary will be at:
- `cargo install`: `~/.cargo/bin/skillbox`
- `cargo build`: `skillbox/target/release/skillbox`

</details>


## 💡 Usage

### Basic Usage

```python
from openai import OpenAI
from skilllite import SkillManager

# Initialize OpenAI-compatible client
client = OpenAI(base_url="https://api.deepseek.com/v1", api_key="your_key")

# Initialize SkillManager
manager = SkillManager(
    skills_dir="./.skills",
    llm_client=client,
    llm_model="deepseek-chat"
)

# Get tool definitions (OpenAI format)
tools = manager.get_tools()

# Call LLM
response = client.chat.completions.create(
    model="deepseek-chat",
    tools=tools,
    messages=[{"role": "user", "content": "Calculate 15 times 27"}]
)

# Handle tool calls
if response.choices[0].message.tool_calls:
    results = manager.handle_tool_calls(response)
```

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
description: My custom Skill
version: 1.0.0
entry_point: scripts/main.py
---

# My Skill

This is the detailed description of the Skill...
```

## Framework Adapters

SkillLite provides adapters for popular AI frameworks with security confirmation support.

### LangChain Integration

```python
from skilllite import SkillManager
from skilllite.core.adapters.langchain import SkillLiteToolkit

manager = SkillManager(skills_dir="./skills")

# Basic usage
tools = SkillLiteToolkit.from_manager(manager).get_tools()

# With security confirmation (sandbox_level=3)
def confirm_execution(report: str, scan_id: str) -> bool:
    print(report)
    return input("Continue? [y/N]: ").lower() == 'y'

tools = SkillLiteToolkit.from_manager(
    manager,
    sandbox_level=3,  # 1=no sandbox, 2=sandbox only, 3=sandbox+scan
    confirmation_callback=confirm_execution
).get_tools()

# Use with LangChain agent
from langchain.agents import AgentExecutor, create_openai_tools_agent
agent = create_openai_tools_agent(llm, tools, prompt)
```

### LlamaIndex Integration

```python
from skilllite import SkillManager
from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

manager = SkillManager(skills_dir="./skills")

# Basic usage
tool_spec = SkillLiteToolSpec.from_manager(manager)
tools = tool_spec.to_tool_list()

# With security confirmation
def confirm(report: str, scan_id: str) -> bool:
    print(report)
    return input("Continue? [y/N]: ").lower() == 'y'

tool_spec = SkillLiteToolSpec.from_manager(
    manager,
    sandbox_level=3,
    confirmation_callback=confirm
)

# Use with LlamaIndex agent
from llama_index.core.agent import ReActAgent
agent = ReActAgent.from_tools(tools, llm=llm)
```

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
# Install with MCP support
pip install skilllite[mcp]

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

- **SkillManager** - Manages Skill discovery, registration, and execution
- **SkillInfo** - Single Skill information encapsulation
- **AgenticLoop** - Automated Agent loop execution
- **ToolDefinition** - OpenAI-compatible tool definition
- **SchemaInferrer** - Smart parameter Schema inference

## 📄 License

MIT

This project includes third-party dependencies with various licenses. See [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) for details.

## 📚 Documentation

- [Getting Started](./docs/en/GETTING_STARTED.md) - Installation and quick start guide
- [Architecture](./docs/en/ARCHITECTURE.md) - Project architecture and design
- [Contributing Guide](./docs/en/CONTRIBUTING.md) - How to contribute
