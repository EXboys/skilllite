# SkillLite

[中文文档](./README_CN.md)

**The only lightweight AI Agent Skills engine with built-in native system-level sandbox, zero dependencies, and local execution.**

A lightweight AI Agent Skills execution engine that integrates with any OpenAI-compatible LLM.

## 🎯 Why SkillLite?

| Feature | SkillLite | Claude Code Sandbox | LangChain Sandbox | OpenAI Plugins | Semantic Kernel |
|---------|-----------|---------------------|-------------------|----------------|-----------------|
| **Built-in Sandbox** | ✅ Rust Native | ✅ Node.js Native | ⚠️ Pyodide/Docker | ⚠️ Cloud (Closed) | ❌ None (Azure) |
| **Sandbox Tech** | Seatbelt + Namespace | Seatbelt + bubblewrap | WebAssembly/Docker | Cloud Isolation | - |
| **Implementation** | **Rust** (High Perf) | Node.js/TypeScript | Python | - | C# |
| **Local Execution** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Zero Dependencies** | ✅ Single Binary | ❌ Needs Node.js | ❌ Needs Runtime | ❌ | ❌ |
| **Cold Start** | ⚡ Milliseconds | Medium | 🐢 Seconds | - | - |
| **LLM Agnostic** | ✅ Any LLM | ❌ Claude Only | ✅ | ❌ OpenAI Only | ✅ |
| **License** | MIT | Apache 2.0 | MIT | Closed | MIT |


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

**Key Differences**:

| Aspect | SkillLite | Claude Code Sandbox |
|--------|-----------|---------------------|
| **Purpose** | General Skills Execution Engine | Claude Code Exclusive |
| **LLM Binding** | ✅ Any LLM | ❌ Claude Only |
| **Implementation** | **Rust** (Higher Performance, Smaller Size) | Node.js/TypeScript |
| **Deployment** | Single Binary, Zero Dependencies | Requires Node.js Runtime |
| **Skills Ecosystem** | Independent Skills Directory | Depends on MCP Protocol |
| **Use Case** | Any Agent Framework Integration | Claude Code Internal Use |

> 💡 **Summary**: Claude Code Sandbox validates that "native system-level sandbox" is the right direction for AI Agent secure execution. SkillLite provides an **LLM-agnostic, Rust-implemented, lighter-weight** alternative for scenarios requiring multi-LLM integration or maximum performance.



## 🔐 Core Innovation: Native System-Level Security Sandbox

SkillLite uses a **Rust-implemented native system-level sandbox**, not Docker or WebAssembly:

- **macOS**: Kernel-level isolation based on Seatbelt (sandbox-exec)
- **Linux**: Container-level isolation based on Namespace + Seccomp

### Fundamental Difference from Other Solutions

```
┌─────────────────────────────────────────────────────────────────┐
│  Other Solutions                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │   Docker    │  │   Pyodide   │  │ Cloud Sandbox│              │
│  │ (Heavyweight)│  │ (WebAssembly)│  │(Data Upload) │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  SkillLite Solution                                              │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │           Rust Native System-Level Sandbox                   ││
│  │  • Direct OS security mechanisms (Seatbelt/Namespace)        ││
│  │  • Zero external dependencies, single binary                 ││
│  │  • Millisecond cold start, production-grade performance      ││
│  │  • Code and data never leave your machine                    ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

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

## 🚀 Quick Start

### 1. Install Rust Sandbox Executor

This project uses a Rust-written isolated sandbox to securely execute Skills scripts. You need to install Rust and compile the sandbox first.

> ⚠️ **Platform Support**: Currently only supports **macOS** and **Linux**. Windows is not supported yet.

#### Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Reload environment variables after installation
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Compile the Sandbox Executor

```bash
# Enter Rust project directory and compile
cd skillbox
cargo build --release

# Optional: Install to system path (recommended)
cargo install --path .

# Verify installation
skillbox --help
```

After compilation, the `skillbox` binary will be located at:
- If using `cargo install`: `~/.cargo/bin/skillbox`
- If only `cargo build`: `skillbox/target/release/skillbox`

### 2. Environment Configuration

```bash
# Copy environment variable template
cp .env.example .env

# Edit .env with your API configuration
# BASE_URL=https://api.deepseek.com/v1
# API_KEY=your_api_key_here
# MODEL=deepseek-chat
```

### 3. Run Example

```bash
python3 simple_demo.py
```

## 📁 Project Structure

```
skillLite/
├── skillbox/              # Rust sandbox executor
├── skilllite/             # Python SDK
│   └── skilllite/
│       ├── manager.py     # SkillManager core manager
│       ├── executor.py    # Skill executor
│       ├── loops.py       # Agentic Loop implementation
│       ├── tools.py       # Tool definitions
│       └── ...
├── .skills/               # Skills directory
│   ├── calculator/        # Calculator Skill
│   ├── data-analyzer/     # Data Analysis Skill
│   ├── http-request/      # HTTP Request Skill
│   ├── text-processor/    # Text Processing Skill
│   ├── weather/           # Weather Query Skill
│   └── writing-helper/    # Writing Assistant Skill
├── simple_demo.py         # Full example
├── simple_demo_v2.py      # Simplified example
└── simple_demo_minimal.py # Minimal example
```

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
