# SkillLite

<div align="center">

[![CI](https://github.com/EXboys/skilllite/actions/workflows/ci.yml/badge.svg)](https://github.com/EXboys/skilllite/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![GitHub Stars](https://img.shields.io/github/stars/EXboys/skilllite)](https://github.com/EXboys/skilllite/stargazers)
[![PyPI](https://img.shields.io/pypi/v/skilllite)](https://pypi.org/project/skilllite/)

</div>

---

<div align="center">

### 📖 Documentation | 文档

| English | 中文 |
|---------|------|
| **[📗 English Docs](./README.md)** · [Pick your path](./docs/en/START_PATHS.md) · [Getting Started](./docs/en/GETTING_STARTED.md) · [Architecture](./docs/en/ARCHITECTURE.md) · [Env Reference](./docs/en/ENV_REFERENCE.md) · [Contributing](./docs/en/CONTRIBUTING.md) | **[📘 中文文档](./docs/zh/README.md)** · [选择你的路径](./docs/zh/START_PATHS.md) · [入门指南](./docs/zh/GETTING_STARTED.md) · [架构说明](./docs/zh/ARCHITECTURE.md) · [环境变量](./docs/zh/ENV_REFERENCE.md) · [贡献指南](./docs/zh/CONTRIBUTING.md) |

</div>

---

### Pick your path

One repository ships **several entry points**. Start with the guide that matches your goal (same anchor IDs in EN/ZH pages):

| Goal | First stop |
|------|----------------|
| **Desktop GUI** — local assistant, skills, optional IDE layout | [Path 1 — Desktop](./docs/en/START_PATHS.md#path-1-desktop) · [路径 1 — 桌面](./docs/zh/START_PATHS.md#path-1-desktop) |
| **Sandbox / MCP** — isolated skills in Cursor, OpenCode, or your own stack | [Path 2 — Sandbox & MCP](./docs/en/START_PATHS.md#path-2-sandbox-mcp) · [路径 2 — 沙箱与 MCP](./docs/zh/START_PATHS.md#path-2-sandbox-mcp) |
| **Full stack** — `skilllite` CLI, Python SDK, evolution | [Path 3 — Full stack](./docs/en/START_PATHS.md#path-3-fullstack) · [路径 3 — 全栈](./docs/zh/START_PATHS.md#path-3-fullstack) |

---

**A lightweight secure Self-evolution engine built in Rust, featuring a built-in native system-level sandbox, zero dependencies, and fully local execution.**

We built the most secure skill sandbox in the ecosystem (20/20 security score). Then we realized: the real value isn't just safe execution — it's safe **evolution**.


[![Performance Benchmark Video](https://github.com/EXboys/skilllite/raw/main/docs/images/benchmark-en.gif)]

![Performance Benchmark Comparison](./docs/images/benchmark-en.png)

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Self-Evolving Engine（自进化引擎）                             │
│                                                              │
│  Immutable Core (compiled into binary, never self-modifies)  │
│  ├─ Agent loop, LLM orchestration, tool execution            │
│  ├─ Config, metadata, path validation                        │
│  └─ Evolution engine: feedback → reflect → evolve → verify   │
│                                                              │
│  Evolvable Data (local files, auto-improves over use)        │
│  ├─ Prompts   — system / planning / execution prompts        │
│  ├─ Memory    — task patterns, tool effects, failure lessons  │
│  └─ Skills    — auto-generated skills from repeated patterns  │
│                         ▼                                    │
│          all evolved artifacts must pass ▼                    │
├──────────────────────────────────────────────────────────────┤
│  Security Sandbox（安全沙箱）                                  │
│                                                              │
│  Full-chain defense across the entire skill lifecycle:       │
│  ├─ Install-time: static scan + LLM analysis + supply-chain  │
│  ├─ Pre-execution: two-phase confirm + integrity check       │
│  └─ Runtime: OS-native isolation (Seatbelt / bwrap / seccomp)│
│     ├─ Process-exec whitelist, FS / network / IPC lockdown   │
│     └─ Resource limits (CPU / mem / fork / fsize)            │
└──────────────────────────────────────────────────────────────┘
```


**Why two layers, not one?** Evolution without safety is reckless — evolved skills could exfiltrate data or consume unbounded resources. Safety without evolution is static — the Agent never improves. SkillLite welds them together: the evolution engine produces new prompts, memory, and skills; the sandbox layer ensures every evolved artifact passes L3 security scanning + OS-level isolation before execution. Evolution is auditable, rollbackable, and never modifies the core binary.


|                                   | **skilllite** (Evolution) | **skilllite-sandbox** (lightweight) |
| --------------------------------- | ------------------------- | ----------------------------------- |
| Binary size                       | ~6.2 MB                   | ~3.6 MB                             |
| Startup RSS                       | ~4 MB                     | ~3.9 MB                             |
| Agent mode RSS (chat / agent-rpc) | ~11 MB                    | —                                   |
| Sandbox execution RSS             | ~11 MB                    | ~10 MB                              |


> Measured on macOS ARM64, release build. Sandbox RSS is dominated by the embedded Python process.
>
> **Use the full stack or just the sandbox**: `skilllite` gives you evolution + agent + sandbox. `skilllite-sandbox` is a standalone binary (or MCP server) that **any** agent framework can adopt — no need to buy into the full SkillLite stack.

---

## 🧬 Intelligence: evolution without lowering the safety bar

Many products now advertise **self-adapting** or **self-evolving** agents. SkillLite’s stance is narrower and easier to keep accurate over time: **prompts, memory, and skills may evolve**, but they stay behind the **same immutable core and full-chain sandbox** as everything else — install-time scan, pre-execution gates, OS-level isolation, and governance (thresholds, backlog, rollback). Evolved artifacts are **not exempt**: they pass the **same L3 checks + sandbox** as manually installed skills.

---

## 🔒 Security: Full-Chain Defense

Most sandbox solutions only provide **runtime isolation**. SkillLite defends across **the entire skill lifecycle** — three layers in a single binary:

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


| Capability                  | SkillLite | E2B     | Docker  | Claude SRT | Pyodide |
| --------------------------- | --------- | ------- | ------- | ---------- | ------- |
| **Install-time scanning**   | ✅         | —       | —       | —          | —       |
| **Static code analysis**    | ✅         | —       | —       | —          | —       |
| **Supply-chain audit**      | ✅         | —       | —       | —          | —       |
| **Process-exec whitelist**  | ✅         | —       | —       | —          | —       |
| **IPC / kernel lockdown**   | ✅         | —       | —       | —          | —       |
| **Filesystem isolation**    | ✅         | partial | partial | partial    | ✅       |
| **Network isolation**       | ✅         | ✅       | —       | ✅          | ✅       |
| **Resource limits**         | ✅         | ✅       | partial | partial    | partial |
| **Runtime sandbox**         | ✅         | ✅       | ✅       | ✅          | ✅       |
| **Zero-dependency install** | ✅         | —       | —       | —          | —       |
| **Offline capable**         | ✅         | —       | partial | ✅          | ✅       |


### Runtime Security Scores (20-item test suite)


| Platform                | Blocked   | Score    |
| ----------------------- | --------- | -------- |
| **SkillLite (Level 3)** | **20/20** | **100%** |
| Pyodide                 | 7/20      | 35%      |
| Claude SRT              | 7.5/20    | 37.5%    |
| Docker (default)        | 2/20      | 10%      |


<details>
<summary>Full 20-item security test breakdown</summary>

| Test Item               | SkillLite | Docker    | Pyodide   | Claude SRT   |
| ----------------------- | --------- | --------- | --------- | ------------ |
| **File System**         |           |           |           |              |
| Read /etc/passwd        | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed    |
| Read SSH private key    | ✅ Blocked | ✅ Blocked | ✅ Blocked | ✅ Blocked    |
| Write to /tmp dir       | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked    |
| Directory traversal     | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed    |
| List root directory     | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| **Network**             |           |           |           |              |
| Send HTTP request       | ✅ Blocked | ❌ Allowed | ✅ Blocked | ✅ Blocked    |
| DNS query               | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked    |
| Listen port             | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked    |
| **Process**             |           |           |           |              |
| Execute os.system()     | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| Execute subprocess      | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed    |
| Enumerate processes     | ✅ Blocked | ❌ Allowed | ❌ Allowed | ✅ Blocked    |
| Send process signal     | ✅ Blocked | ❌ Allowed | ✅ Blocked | ⚠️ Partially |
| **Resource Limits**     |           |           |           |              |
| Memory bomb             | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| Fork bomb               | ✅ Blocked | ❌ Allowed | ✅ Blocked | ❌ Allowed    |
| CPU intensive compute   | ✅ Blocked | ✅ Blocked | ❌ Allowed | ✅ Blocked    |
| **Code Injection**      |           |           |           |              |
| Dynamic import os       | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| Use eval/exec           | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| Modify built-in funcs   | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| **Information Leakage** |           |           |           |              |
| Read environment vars   | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |
| Get system info         | ✅ Blocked | ❌ Allowed | ❌ Allowed | ❌ Allowed    |


```bash
# Reproduce: run security comparison tests
cd benchmark && python3 security_vs.py
```

</details>

---

## ⚡ Performance


| Dimension      | SkillLite     | Docker          | Pyodide          | SRT                   |
| -------------- | ------------- | --------------- | ---------------- | --------------------- |
| **Warm Start** | 40 ms         | 194 ms          | 672 ms           | 596 ms                |
| **Cold Start** | 492 ms        | 120s            | ~5s              | ~1s                   |
| **Memory**     | ~10 MB        | ~100 MB         | ~50 MB           | ~84 MB                |
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

**In one sentence**: Other agent frameworks are smart but unsafe, or safe but static. SkillLite is both — an Agent that gets measurably better over time, with every evolved artifact security-constrained by an OS-native sandbox.

- **vs Agent frameworks** (AutoGen, CrewAI, LangGraph): They provide orchestration but no built-in evolution or sandbox. SkillLite evolves autonomously and executes safely.
- **vs Sandbox tools** (E2B, Docker, Claude SRT): They provide isolation but no intelligence layer. SkillLite adds a full agent loop + self-evolution on top.
- **vs Evolution platforms** (OpenClaw Foundry, EvoAgentX): They enable evolution but without security constraints on evolved artifacts. SkillLite enforces L3 scanning + OS sandbox on everything it evolves.

> Claude/Anthropic's [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing) uses the **same underlying sandbox tech** (Seatbelt + bubblewrap). See [Architecture Comparison](./docs/zh/CLAUDE-CODE-OPENCLAW-ARCHITECTURE-COMPARISON.md) for a detailed side-by-side analysis.

---

## 🚀 Quick Start

### Installation (Recommended: pip)

```bash
pip install skilllite
skilllite init        # sandbox binary + skills/ + download skills
skilllite list        # verify installation
```

**Zero-config quick start** (auto-detect LLM, setup skills, launch chat):

```bash
skilllite quickstart
```

### Run Your First Example

```python
from skilllite import chat

result = chat("Calculate 15 * 27", skills_dir="skills")
print(result)
```

Or use the CLI: `skilllite chat`

### Environment Configuration

```bash
cp .env.example .env   # Edit: BASE_URL, API_KEY, MODEL
```


| File                                                   | Description          |
| ------------------------------------------------------ | -------------------- |
| [.env.example](./.env.example)                         | Quick start template |
| [.env.example.full](./.env.example.full)               | Full variable list   |
| [docs/en/ENV_REFERENCE.md](./docs/en/ENV_REFERENCE.md) | Complete reference   |


> **Platform Support**: macOS, Linux, and Windows (via WSL2 Bridge).

---

## 📚 Tutorials


| Tutorial                                                            | Time   | Description                             |
| ------------------------------------------------------------------- | ------ | --------------------------------------- |
| [01. Basic Usage](./tutorials/01_basic)                             | 5 min  | Simplest examples, one-line execution   |
| [02. Skill Management](./tutorials/02_skill_management)             | 10 min | Create and manage skills                |
| [03. Agentic Loop](./tutorials/03_agentic_loop)                     | 15 min | Multi-turn conversations and tool calls |
| [04. LangChain Integration](./tutorials/04_langchain_integration)   | 15 min | Integration with LangChain framework    |
| [05. LlamaIndex Integration](./tutorials/05_llamaindex_integration) | 15 min | RAG + skill execution                   |
| [06. MCP Server](./tutorials/06_mcp_server)                         | 10 min | Claude Desktop integration              |
| [07. OpenCode Integration](./tutorials/07_opencode_integration)     | 10 min | One-command OpenCode integration        |


👉 **[View All Tutorials](./tutorials/README.md)**

---

## Evolution Arena (Evotown)

[Evotown](./evotown/) is an evolution testing platform that puts evolution engines (e.g. SkillLite) in a controlled environment for **evolution effect validation**.

![Evotown — Evolution Arena](./docs/images/Evotown.png)

---

## 💡 Usage

### Direct Skill Execution

```python
from skilllite import run_skill

result = run_skill("./skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
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


| Level | Description                                                                     |
| ----- | ------------------------------------------------------------------------------- |
| 1     | No sandbox — direct execution                                                   |
| 2     | Sandbox isolation only                                                          |
| 3     | Sandbox + static security scan (requires confirmation for high-severity issues) |


### Supported LLM Providers


| Provider       | base_url                                            |
| -------------- | --------------------------------------------------- |
| OpenAI         | `https://api.openai.com/v1`                         |
| DeepSeek       | `https://api.deepseek.com/v1`                       |
| Qwen           | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot       | `https://api.moonshot.cn/v1`                        |
| Ollama (Local) | `http://localhost:11434/v1`                         |


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

## 📦 Crates Architecture

SkillLite is a Cargo workspace of focused, composable crates. Each crate has a single responsibility and can be compiled independently.

```
skilllite/                         Dependency Flow
├── Cargo.toml                     ────────────────────────────
├── skilllite/  (main binary)      skilllite (CLI entry point)
│                                    ├── skilllite-commands
└── crates/                          │     ├── skilllite-evolution ──┐
    ├── skilllite-core/              │     ├── skilllite-sandbox ────┤
    ├── skilllite-sandbox/           │     └── skilllite-agent (opt) │
    ├── skilllite-evolution/         ├── skilllite-agent             │
    ├── skilllite-executor/          │     ├── skilllite-evolution   │
    ├── skilllite-agent/             │     ├── skilllite-sandbox     │
    ├── skilllite-commands/          │     └── skilllite-executor    │
    ├── skilllite-swarm/             ├── skilllite-swarm             │
    ├── skilllite-artifact/          ├── skilllite-artifact          │
    └── skilllite-assistant/         └───────────┬──────────────────┘
                                          skilllite-core (foundation)
```


| Crate                   | Role                                                                                                                                                                               | Layer        |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| **skilllite-core**      | Foundation — config, skill metadata, path validation, observability                                                                                                                | Shared       |
| **skilllite-sandbox**   | **Security Sandbox** — OS-native isolation (Seatbelt / bwrap / seccomp), static scan, supply-chain audit, resource limits. Independently deliverable as `skilllite-sandbox` binary | 🔒 Sandbox   |
| **skilllite-evolution** | **Self-Evolving Engine** — feedback collection → reflection → evolution → quality gate → audit. Drives prompt / memory / skill evolution                                           | 🧬 Evolution |
| **skilllite-executor**  | Session management — transcript logging, memory storage, vector search (opt)                                                                                                       | Agent        |
| **skilllite-agent**     | LLM Agent loop — multi-turn chat, tool orchestration, planning                                                                                                                     | Agent        |
| **skilllite-commands**  | CLI command implementations — wires crates into `skilllite` binary                                                                                                                 | CLI          |
| **skilllite-swarm**     | P2P mesh — mDNS discovery, peer routing, distributed task dispatch                                                                                                                 | Network      |
| **skilllite-artifact**  | Run-scoped artifact storage — local dir (agent default), optional HTTP server/client (`skilllite artifact-serve` when `artifact_http` is enabled)                                   | Storage / HTTP |
| **skilllite-assistant** | Desktop app — Tauri 2 + React, standalone GUI                                                                                                                                      | App          |


> **Two independently deliverable binaries**: `skilllite` (full: evolution + agent + sandbox) and `skilllite-sandbox` (lightweight: sandbox + MCP only, ~3.6 MB). The sandbox has zero dependency on the agent or evolution crates — other frameworks (LangChain, AutoGen, CrewAI, etc.) can embed it directly via CLI, MCP, or as a Rust crate.

### SDK & Integrations

- **python-sdk** (`pip install skilllite`) — Thin bridge (~770 lines of Python under `python-sdk/skilllite/`), zero runtime deps
- **langchain-skilllite** (`pip install langchain-skilllite`) — LangChain / LangGraph adapter

<details>
<summary>CLI Commands</summary>

| Command                        | Description                                                            |
| ------------------------------ | ---------------------------------------------------------------------- |
| `skilllite init`               | Initialize project (skills/ + download skills + dependencies + audit) |
| `skilllite quickstart`         | Zero-config: detect LLM, setup skills, launch chat                     |
| `skilllite chat`               | Interactive agent chat (or `--message` for single-shot)                |
| `skilllite add owner/repo`     | Add skills from GitHub                                                 |
| `skilllite remove <name>`      | Remove an installed skill                                              |
| `skilllite list`               | List installed skills                                                  |
| `skilllite show <name>`        | Show skill details                                                     |
| `skilllite run <dir> '<json>'` | Execute a skill directly                                               |
| `skilllite scan <dir>`         | Scan skill for security issues                                         |
| `skilllite evolution status`   | View evolution metrics and history                                     |
| `skilllite evolution backlog`  | Query backlog proposals (status/risk/ROI/acceptance_status)            |
| `skilllite evolution run`      | Force-trigger evolution cycle                                          |
| `skilllite mcp`                | Start MCP server (Cursor/Claude Desktop)                               |
| `skilllite serve`              | Start IPC daemon (stdio JSON-RPC)                                      |
| `skilllite artifact-serve`     | Run-scoped artifact HTTP server (bind requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`) |
| `skilllite init-cursor`        | Initialize Cursor IDE integration                                      |
| `skilllite init-opencode`      | Initialize OpenCode integration                                        |
| `skilllite clean-env`          | Clean cached runtime environments                                      |
| `skilllite reindex`            | Re-index all installed skills                                          |

> Note: for default `skills` mode, if `skills/` is missing and `.skills/` exists, SkillLite automatically falls back to `.skills/`.
> When both `skills/<name>` and `.skills/<name>` exist, commands print a duplicate-name warning to improve observability.
> Workspace skill discovery also supports `.agents/skills/` and `.claude/skills/` as canonical search roots.

</details>


<details>
<summary>Build from Source</summary>

### Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Build & Install Commands (from repository root)


| Package   | Binary                | Command                                                                                            | Description                                  |
| --------- | --------------------- | -------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| skilllite | **skilllite**         | `cargo build -p skilllite`                                                                         | **Full** (Evolution + Agent + Sandbox + MCP; includes `artifact-serve` code, bind requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`) |
| skilllite | **skilllite**         | `cargo build -p skilllite --features memory_vector`                                                | Full **+ vector memory** search              |
| skilllite | **skilllite**         | `cargo build -p skilllite --no-default-features`                                                   | Minimal: run/exec/bash/scan only             |
| skilllite | **skilllite-sandbox** | `cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | Sandbox + MCP only                           |


### Install (to `~/.cargo/bin/`)


| Command                                                                                                  | What you get                               |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `cargo install --path skilllite`                                                                         | **skilllite** — full                       |
| `cargo install --path skilllite --features memory_vector`                                                | **skilllite** — full + vector memory       |
| `cargo install --path skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | **skilllite-sandbox** — sandbox + MCP only |


**Default features** = `sandbox`, `audit`, `agent`, `swarm`, `artifact_http`. Vector memory (`memory_vector`) is **not** in default. **`skilllite artifact-serve`** is compiled in but **refuses to bind** unless **`SKILLLITE_ARTIFACT_SERVE_ALLOW=1`** is set (reduces accidental exposure).

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

**SkillLite Assistant** is the native **desktop app** (Tauri 2 + React) on top of the same **`skilllite`** engine (`agent-rpc`). **Installers** (dmg / msi / AppImage) ship on [GitHub Releases](https://github.com/EXboys/skilllite/releases) (desktop builds may appear shortly after the main release artifacts). First-time orientation: [Pick your path → Desktop](./docs/en/START_PATHS.md#path-1-desktop).

**Settings** (gear icon): **Model & API** (UI language, provider, API key, model, optional base URL, token stats), **Workspace & sandbox**, **Agent budget**, **Evolution**, **Schedule**.

![SkillLite Assistant — Settings, Model & API tab](./docs/images/assistant-settings-model-api.png)

Tauri 2 + React Desktop, located at `crates/skilllite-assistant/`:

```bash
cd crates/skilllite-assistant
npm install
npm run tauri dev    # dev mode (HMR)
npm run tauri build
```

Chat input behavior: `Enter` inserts a new line; submit by clicking the `Send` button.
Optional **IDE** three-pane layout (toolbar **IDE** or **Settings → Workspace & sandbox → IDE three-pane main window**): workspace file tree, editor, and chat; **Sessions** tab keeps the session list. **Drag the vertical splitters** between panes to resize (widths persist). Heavy folders like `node_modules` / `target` are skipped; macOS `.DS_Store` and Windows `Thumbs.db` are omitted from the file tree; non‑UTF‑8 files show a clear in‑editor error instead of a raw decode failure. Sensitive paths match write rules (e.g. `.env` blocked). Turn off IDE layout to bring back the right-hand status panel. The center editor supports **Markdown preview** (with an edit tab) and **read-only preview** for common **images** and **videos** via the Tauri asset protocol. In chat, `read_file` results use a short (~5-line) scrollable preview; **click the preview** to open the file in IDE layout when a path is known, or the fullscreen reader otherwise. `list_directory` output uses the same-height scrollable tree preview.
Pending **execution confirmations** live inside the collapsible **internal steps** timeline (it auto-expands when action is required; you can enable **auto-allow execution confirmations** in **Settings → Agent budget** or under the input on trusted machines).
For tool outcomes marked as `partial_success` or `failure`, the assistant shows a multi-option recovery prompt, including `【启动进化】` to enqueue a governed capability-evolution backlog proposal.
Default agent prompts bias toward **implementing** missing capabilities (scripts, `run_command`, small skills) instead of flatly refusing browser/desktop-style requests when no dedicated skill exists. App upgrades that bump the prompt **seed version** may **overwrite** `~/.skilllite/chat/prompts/*.md` when the on-disk file differs from the new bundled template (including upgrades from older stock); **back up** those files if you customized them.
**Uninstall**: **Settings → Workspace & sandbox** includes **Uninstall & data** (remove the app only or wipe local assistant data; see the assistant README).

**Windows**: background subprocesses (bundled engine checks, Life Pulse, runtime probes, and agent `run_command` shells) are started with `CREATE_NO_WINDOW`, so you should not see empty Command Prompt windows flashing during normal use.

See [crates/skilllite-assistant/README.md](./crates/skilllite-assistant/README.md).

</details>

---

## 🤝 Upstream Contributions

SkillLite's sandbox hardening experience was contributed to [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) via [issue #4812](https://github.com/zeroclaw-labs/zeroclaw/issues/4812) and adopted in [PR #4821](https://github.com/zeroclaw-labs/zeroclaw/pull/4821), improving its native sandbox security posture (seccomp, capability dropping, fail-closed backend selection).

---

## 📄 License

MIT — See [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md) for third-party details.

## 📚 Documentation

- [Pick your path](./docs/en/START_PATHS.md) — Desktop vs sandbox/MCP vs full stack (start here if unsure)
- [Getting Started](./docs/en/GETTING_STARTED.md) — Installation and quick start guide
- [Changelog](./CHANGELOG.md) — Version history and upgrade notes
- [Environment Variables Reference](./docs/en/ENV_REFERENCE.md) — Complete env var documentation
- [Architecture](./docs/en/ARCHITECTURE.md) — Project architecture and design
- [Architecture Comparison](./docs/zh/CLAUDE-CODE-OPENCLAW-ARCHITECTURE-COMPARISON.md) — Claude Code vs OpenClaw vs OpenCode vs SkillLite（中文）
- [Contributing Guide](./docs/en/CONTRIBUTING.md) — How to contribute

