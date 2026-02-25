# SkillLite

[中文文档](./docs/zh/README.md)

**A lightweight AI Agent Skills secure engine with built-in native system-level sandbox, zero dependencies, and local execution.**

[![Performance Benchmark Video](https://github.com/EXboys/skilllite/raw/main/docs/images/benchmark-en.gif)]

![Performance Benchmark Comparison](./docs/images/benchmark-en.png)

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

| | **skilllite** (full) | **skilllite-sandbox** (lightweight) |
|---|---|---|
| Binary size | ~6.2 MB | ~3.6 MB |
| Startup RSS | ~4 MB | ~3.9 MB |
| Agent mode RSS (chat / agent-rpc) | ~11 MB | — |
| Sandbox execution RSS | ~11 MB | ~10 MB |

> Measured on macOS ARM64, release build. Sandbox RSS is dominated by the embedded Python process. The Agent layer adds memory only when chat, planning, or memory features are actively used.

---

## 🔒 Supply-Chain Defense: Full-Chain Security

> **Core principle: Scan before install, enforce after install. (先判毒，再落地；落地后防改)**

Most sandbox solutions only provide **runtime isolation** — a single layer. SkillLite is the first lightweight engine that defends across **the entire skill lifecycle**:

```
┌─────────────────────────────────────────────────┐
│ Layer 1 — Install-time Scanning                 │
│ ├─ Static rule scan (regex pattern matching)    │
│ ├─ LLM-assisted analysis (suspicious → confirm) │
│ └─ Supply-chain audit (PyPI / OSV vuln DB)      │
├─────────────────────────────────────────────────┤
│ Layer 2 — Pre-execution Authorization           │
│ ├─ Two-phase confirm (scan → user OK → run)     │
│ └─ Integrity check (hash tamper detection)      │
├─────────────────────────────────────────────────┤
│ Layer 3 — Runtime Sandbox                       │
│ ├─ OS-native isolation (Seatbelt / bwrap)       │
│ ├─ Process-exec whitelist (interpreter only)    │
│ ├─ Filesystem / network / IPC lockdown          │
│ └─ Resource limits (rlimit CPU/mem/fork/fsize)  │
└─────────────────────────────────────────────────┘
```

### Full-Chain Security Comparison

| Capability | SkillLite | E2B | Docker | Claude SRT | Pyodide |
|---|:-:|:-:|:-:|:-:|:-:|
| **Install-time scanning** | ✅ | — | — | — | — |
| **Static code analysis** | ✅ | — | — | — | — |
| **Supply-chain audit** | ✅ | — | — | — | — |
| **Process-exec whitelist** | ✅ | — | — | — | — |
| **IPC / kernel lockdown** | ✅ | — | — | — | — |
| **Filesystem isolation** | ✅ | partial | partial | partial | ✅ |
| **Network isolation** | ✅ | ✅ | — | ✅ | ✅ |
| **Resource limits** | ✅ | ✅ | partial | partial | partial |
| **Runtime sandbox** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Zero-dependency install** | ✅ | — | — | — | — |
| **Offline capable** | ✅ | — | partial | ✅ | ✅ |

> Other solutions focus on runtime isolation only. SkillLite adds install-time and pre-execution layers — three lines of defense in a single binary.

### Runtime Security Scores (20-item test suite)

| Platform | Blocked | Score |
|---|---|---|
| **SkillLite (Level 3)** | **20/20** | **100%** |
| Pyodide | 7/20 | 35% |
| Claude SRT | 7.5/20 | 37.5% |
| Docker (default) | 2/20 | 10% |

<details>
<summary>Full 20-item security test breakdown</summary>

| Test Item | SkillLite | Docker | Pyodide | Claude SRT |
|---|:-:|:-:|:-:|:-:|
| **File System** | | | | |
| Read /etc/passwd | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed |
| Read SSH private key | ✅ Blocked | ✅ Blocked | ✅ Blocked | ✅ Blocked |
| Write to /tmp dir | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked |
| Directory traversal | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed |
| List root directory | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| **Network** | | | | |
| Send HTTP request | ✅ Blocked | ❌ Allowed | ✅ Blocked | ✅ Blocked |
| DNS query | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked |
| Listen port | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked |
| **Process** | | | | |
| Execute os.system() | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| Execute subprocess | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed |
| Enumerate processes | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked |
| Send process signal | ✅ Blocked | ❌ Allowed | ✅ Blocked | ⚠️ Partially |
| **Resource Limits** | | | | |
| Memory bomb | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| Fork bomb | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed |
| CPU intensive compute | ✅ Blocked | ✅ Blocked | ❌ Allowed | ✅ Blocked |
| **Code Injection** | | | | |
| Dynamic import os | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| Use eval/exec | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| Modify built-in funcs | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| **Information Leakage** | | | | |
| Read environment vars | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |
| Get system info | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed |

```bash
# Reproduce: run security comparison tests
cd benchmark && python3 security_vs.py
```

</details>

---

## ⚡ Performance


| Dimension | SkillLite | Docker | Pyodide | SRT |
|---|---|---|---|---|
| **Warm Start** | 40 ms | 194 ms | 672 ms | 596 ms |
| **Cold Start** | 492 ms | 120s | ~5s | ~1s |
| **Memory** | ~10 MB | ~100 MB | ~50 MB | ~84 MB |
| **Deployment** | Single binary | Requires daemon | Requires Node.js | Requires installation |

> **3-5x faster** execution, **10x lower memory** footprint vs Docker/SRT.

<details>
<summary>Performance benchmark details & commands</summary>

![Performance Benchmark Comparison](./docs/images/benchmark-en.png)

```bash
cd benchmark/
python benchmark_runner.py --compare-levels --compare-ipc -n 100 -c 10

# Cold start comparison
python benchmark_runner.py --cold-start --compare-ipc

# Full test: cold start + high concurrency
python benchmark_runner.py --cold-start --cold-iterations 20 --compare-levels --compare-ipc -o results.json
```

See [benchmark/README.md](./benchmark/README.md) for full documentation.

</details>

---

## 🎯 Why SkillLite?

| Feature | SkillLite | Claude Code Sandbox | Pyodide | OpenAI Plugins | Semantic Kernel |
|---------|-----------|---------------------|---------|----------------|-----------------|
| **Built-in Sandbox** | ✅ Rust Native | ✅ Node.js Native | ⚠️ Docker | ⚠️ Cloud (Closed) | ❌ None |
| **Sandbox Tech** | Seatbelt + Namespace | Seatbelt + bubblewrap | WebAssembly/Docker | Cloud Isolation | — |
| **Supply-Chain Defense** | ✅ Full-chain | — | — | — | — |
| **Local Execution** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **Zero Dependencies** | ✅ Single Binary | ❌ Needs Node.js | ❌ Needs Runtime | ❌ | ❌ |
| **Cold Start** | ⚡ Milliseconds | Medium | 🐢 Seconds | — | — |
| **LLM Agnostic** | ✅ Any LLM | ❌ Claude Only | ✅ | ❌ OpenAI Only | ✅ |

> Claude/Anthropic's [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing) (Oct 2025) uses the **same underlying tech** (Seatbelt + bubblewrap) — SkillLite adds full-chain supply-chain defense on top.

---

## 🚀 Quick Start

### Installation (Recommended: pip)

```bash
pip install skilllite
skilllite init        # sandbox binary + .skills/ + download skills
skilllite list        # verify installation
```

**Zero-config quick start** (auto-detect LLM, setup skills, launch chat):

```bash
skilllite quickstart
```

### Run Your First Example

```python
from skilllite import chat

result = chat("Calculate 15 * 27", skills_dir=".skills")
print(result)
```

Or use the CLI: `skilllite chat`

### Environment Configuration

```bash
cp .env.example .env   # Edit: BASE_URL, API_KEY, MODEL
```

| File | Description |
|------|-------------|
| [.env.example](./.env.example) | Quick start template |
| [.env.example.full](./.env.example.full) | Full variable list |
| [docs/en/ENV_REFERENCE.md](./docs/en/ENV_REFERENCE.md) | Complete reference |

> **Platform Support**: macOS, Linux, and Windows (via WSL2 Bridge).

---

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

👉 **[View All Tutorials](./tutorials/README.md)**

---

## 💡 Usage

### Direct Skill Execution

```python
from skilllite import run_skill

result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

### Skill Repository Management

```bash
skilllite add owner/repo                    # Add all skills from a GitHub repo
skilllite add owner/repo@skill-name         # Add a specific skill by name
skilllite add ./local-path                  # Add from local directory
skilllite list                              # List all installed skills
skilllite remove <skill-name>               # Remove an installed skill
```

### Framework Integration

```bash
pip install langchain-skilllite   # LangChain adapter
```

```python
from langchain_skilllite import SkillLiteToolkit
from langgraph.prebuilt import create_react_agent

tools = SkillLiteToolkit.from_directory(
    "./skills",
    sandbox_level=3,  # 1=no sandbox, 2=sandbox only, 3=sandbox+scan
    confirmation_callback=lambda report, sid: input("Continue? [y/N]: ").lower() == 'y'
)
agent = create_react_agent(ChatOpenAI(model="gpt-4"), tools)
```

See [05. LlamaIndex Integration](./tutorials/05_llamaindex_integration/README.md) for LlamaIndex usage.

### Security Levels

| Level | Description |
|-------|-------------|
| 1 | No sandbox — direct execution |
| 2 | Sandbox isolation only |
| 3 | Sandbox + static security scan (requires confirmation for high-severity issues) |

### Supported LLM Providers

| Provider | base_url |
|----------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot | `https://api.moonshot.cn/v1` |
| Ollama (Local) | `http://localhost:11434/v1` |

---

## 🛠️ Create Custom Skill

Each Skill is a directory with a `SKILL.md`:

```
my-skill/
├── SKILL.md           # Skill metadata (required)
├── scripts/main.py    # Entry script
├── references/        # Reference documents (optional)
└── assets/            # Resource files (optional)
```

<details>
<summary>SKILL.md example</summary>

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

Detailed description of the Skill.

## Input Parameters

- `query`: Input query string (required)

## Output Format

Returns JSON result.
```

> Dependencies are declared in `compatibility` (not `requirements.txt`). Entry point is auto-detected (`main.py` > `main.js` > `main.ts` > `main.sh`).

</details>

---

## 📦 Core Components

- **skilllite** (Rust binary) — Sandbox executor, CLI, Agent loop, MCP server — single binary
- **python-sdk** (`pip install skilllite`) — Thin bridge (~600 lines), zero runtime deps
- **langchain-skilllite** (`pip install langchain-skilllite`) — LangChain adapter

<details>
<summary>CLI Commands</summary>

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

</details>

<details>
<summary>Build from Source</summary>

### Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Build & Install Commands (from repository root)

| Package | Binary | Command | Description |
|---------|--------|---------|-------------|
| skilllite | **skilllite** | `cargo build -p skilllite` | **Full** (Agent + Chat + MCP + sandbox + audit) |
| skilllite | **skilllite** | `cargo build -p skilllite --features memory_vector` | Full **+ vector memory** search |
| skilllite | **skilllite** | `cargo build -p skilllite --no-default-features` | Minimal: run/exec/bash/scan only |
| skilllite | **skilllite-sandbox** | `cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | Sandbox + MCP only |

### Install (to `~/.cargo/bin/`)

| Command | What you get |
|---------|--------------|
| `cargo install --path skilllite` | **skilllite** — full |
| `cargo install --path skilllite --features memory_vector` | **skilllite** — full + vector memory |
| `cargo install --path skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | **skilllite-sandbox** — sandbox + MCP only |

**Default features** = `sandbox`, `audit`, `agent`. Vector memory (`memory_vector`) is **not** in default.

### Project Structure (Cargo Workspace)

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

<details>
<summary>OpenCode Integration</summary>

```bash
pip install skilllite
skilllite init-opencode   # Auto-configure OpenCode MCP
opencode
```

The `init-opencode` command automatically detects the best way to start the MCP server, creates `opencode.json`, and discovers your skills.

</details>

<details>
<summary>Desktop Assistant (skilllite-assistant)</summary>

Tauri 2 + React Desktop, located at `crates/skilllite-assistant/`:

```bash
cd crates/skilllite-assistant
npm install
npm run tauri dev    # dev mode (HMR)
npm run tauri build
```

See [crates/skilllite-assistant/README.md](./crates/skilllite-assistant/README.md).

</details>

---

## 📄 License

MIT — See [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) for third-party details.

## 📚 Documentation

- [Getting Started](./docs/en/GETTING_STARTED.md) — Installation and quick start guide
- [Environment Variables Reference](./docs/en/ENV_REFERENCE.md) — Complete env var documentation
- [Architecture](./docs/en/ARCHITECTURE.md) — Project architecture and design
- [Contributing Guide](./docs/en/CONTRIBUTING.md) — How to contribute
