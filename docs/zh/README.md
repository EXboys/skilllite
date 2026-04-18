# SkillLite

[English](../../README.md)

**不知道该从哪开始？** 同一仓库提供多种入口，请先读 **[选择你的路径](./START_PATHS.md)**（桌面 / 沙箱与 MCP / 全栈），再读下文长文档。

**越用越强的 AI Agent 引擎——在安全沙箱约束下自进化。**

AI Agent 需要从经验中进化——学习更好的 prompt、积累记忆、从重复模式中生成新技能。但自进化天然有风险：进化出的代码可能是恶意的，进化出的规则可能导致越权。**SkillLite 用一个单二进制解决这个问题**：不可变的安全内核约束所有进化产物，Agent 越用越强，安全性不降级。零依赖、本地优先、LLM 无关。

[![Performance Benchmark Video](https://github.com/EXboys/skilllite/raw/main/docs/images/benchmark-en.gif)]

![Performance Benchmark Comparison](../images/benchmark-en.png)

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│  自进化引擎（Self-Evolving Engine）                            │
│                                                              │
│  不可变内核（编译进二进制，永不自修改）                          │
│  ├─ Agent 循环、LLM 调度、工具执行                             │
│  ├─ 配置、元数据、路径校验                                     │
│  └─ 进化引擎：反馈 → 反思 → 进化 → 质量校验                    │
│                                                              │
│  可进化数据（本地文件，越用越好）                               │
│  ├─ Prompts — system / planning / execution 提示词            │
│  ├─ Memory  — 任务模式、工具效果、失败教训                      │
│  └─ Skills  — 从重复模式自动生成的新技能                        │
│                         ▼                                    │
│          所有进化产物必须通过 ▼                                 │
├──────────────────────────────────────────────────────────────┤
│  安全沙箱（Security Sandbox）                                  │
│                                                              │
│  全生命周期防御：                                              │
│  ├─ 安装时：静态扫描 + LLM 分析 + 供应链审计                    │
│  ├─ 执行前：两阶段确认 + 完整性校验                             │
│  └─ 运行时：OS 原生隔离（Seatbelt / bwrap / seccomp）          │
│     ├─ 进程白名单、文件系统/网络/IPC 锁定                       │
│     └─ 资源限制（CPU / 内存 / fork / 文件大小）                 │
└──────────────────────────────────────────────────────────────┘
```

我们先做了生态中安全评分最高的沙箱（20/20 安全测试满分），然后意识到：真正的价值不只是安全执行——而是安全的**进化**。

**为什么两层缺一不可？** 没有安全的进化是鲁莽的——进化出的技能可能窃取数据或占满资源。没有进化的安全是静态的——Agent 永远不会进步。SkillLite 将两者焊死：进化引擎产出新 prompt、记忆和技能；沙箱层确保每一个进化产物都通过 L3 安全扫描 + OS 级隔离后才能执行。进化可审计、可回滚，且永不修改核心二进制。

| | **skilllite**（完整版） | **skilllite-sandbox**（轻量版） |
|---|---|---|
| 二进制大小 | ~6.2 MB | ~3.6 MB |
| 启动 RSS | ~4 MB | ~3.9 MB |
| Agent 模式 RSS | ~11 MB | — |
| 沙箱执行 RSS | ~11 MB | ~10 MB |

> macOS ARM64 release build 测量。沙箱 RSS 主要来自嵌入的 Python 进程。
>
> **用完整版还是只用沙箱？** `skilllite` 提供进化 + Agent + 沙箱。`skilllite-sandbox` 是独立二进制（或 MCP 服务器），**任何** Agent 框架都可以直接采纳——无需使用 SkillLite 的完整技术栈。

---

## 🧬 智能轴：进化不降低安全门槛

业界已有不少产品强调 **自适应** 或 **自进化** Agent。SkillLite 的表述更窄、也更耐看：**prompt、记忆、技能可以进化**，但仍走同一套 **不可变内核 + 全链路沙箱** —— 安装时扫描、执行前门控、OS 级隔离，以及阈值、backlog、回滚等治理。进化产物 **不设豁免**：与手动安装的技能一样，走 **同一套 L3 校验 + 沙箱**。

---

## 🔒 安全轴：全链路防御

多数沙箱方案只提供**运行时隔离**。SkillLite 在**技能全生命周期**防御——三层防线，一个二进制：

```
┌─────────────────────────────────────────────────┐
│ 第 1 层 — 安装时扫描                              │
│ ├─ 静态规则扫描（正则模式匹配）                    │
│ ├─ LLM 辅助分析（可疑 → 确认）                    │
│ └─ 供应链审计（PyPI / OSV 漏洞库）                 │
├─────────────────────────────────────────────────┤
│ 第 2 层 — 执行前授权                              │
│ ├─ 两阶段确认（扫描 → 用户确认 → 执行）            │
│ └─ 完整性校验（哈希篡改检测）                      │
├─────────────────────────────────────────────────┤
│ 第 3 层 — 运行时沙箱                              │
│ ├─ OS 原生隔离（Seatbelt / bwrap）               │
│ ├─ 进程执行白名单（仅允许解释器）                  │
│ ├─ 文件系统 / 网络 / IPC 锁定                     │
│ └─ 资源限制（rlimit CPU/内存/fork/文件大小）       │
└─────────────────────────────────────────────────┘
```

| 安全能力 | SkillLite | E2B | Docker | Claude SRT | Pyodide |
|---|:-:|:-:|:-:|:-:|:-:|
| **安装时扫描** | ✅ | — | — | — | — |
| **静态代码分析** | ✅ | — | — | — | — |
| **供应链审计** | ✅ | — | — | — | — |
| **进程执行白名单** | ✅ | — | — | — | — |
| **IPC / 内核锁定** | ✅ | — | — | — | — |
| **文件系统隔离** | ✅ | 部分 | 部分 | 部分 | ✅ |
| **网络隔离** | ✅ | ✅ | — | ✅ | ✅ |
| **资源限制** | ✅ | ✅ | 部分 | 部分 | 部分 |
| **运行时沙箱** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **零依赖安装** | ✅ | — | — | — | — |
| **离线可用** | ✅ | — | 部分 | ✅ | ✅ |

### 运行时安全评分（20 项测试）

| 平台 | 拦截 | 评分 |
|---|---|---|
| **SkillLite (Level 3)** | **20/20** | **100%** |
| Pyodide | 7/20 | 35% |
| Claude SRT | 7.5/20 | 37.5% |
| Docker (默认) | 2/20 | 10% |

<details>
<summary>完整 20 项安全测试明细</summary>

| 测试项 | SkillLite | Docker | Pyodide | Claude SRT |
|---|:-:|:-:|:-:|:-:|
| **文件系统** | | | | |
| 读取 /etc/passwd | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ❌ 放行 |
| 读取 SSH 私钥 | ✅ 拦截 | ✅ 拦截 | ✅ 拦截 | ✅ 拦截 |
| 写入 /tmp 目录 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ✅ 拦截 |
| 目录遍历 | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ❌ 放行 |
| 列出根目录 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| **网络** | | | | |
| 发送 HTTP 请求 | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ✅ 拦截 |
| DNS 查询 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ✅ 拦截 |
| 监听端口 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ✅ 拦截 |
| **进程** | | | | |
| 执行 os.system() | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| 执行 subprocess | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ❌ 放行 |
| 枚举进程 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ✅ 拦截 |
| 发送进程信号 | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ⚠️ 部分 |
| **资源限制** | | | | |
| 内存炸弹 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| Fork 炸弹 | ✅ 拦截 | ❌ 放行 | ✅ 拦截 | ❌ 放行 |
| CPU 密集计算 | ✅ 拦截 | ✅ 拦截 | ❌ 放行 | ✅ 拦截 |
| **代码注入** | | | | |
| 动态 import os | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| 使用 eval/exec | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| 修改内置函数 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| **信息泄露** | | | | |
| 读取环境变量 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |
| 获取系统信息 | ✅ 拦截 | ❌ 放行 | ❌ 放行 | ❌ 放行 |

```bash
# 复现：运行安全对比测试
cd benchmark && python3 security_vs.py
```

</details>

---

## ⚡ 性能

| 维度 | SkillLite | Docker | Pyodide | SRT |
|---|---|---|---|---|
| **热启动** | 40 ms | 194 ms | 672 ms | 596 ms |
| **冷启动** | 492 ms | 120s | ~5s | ~1s |
| **内存** | ~10 MB | ~100 MB | ~50 MB | ~84 MB |
| **部署** | 单二进制 | 需守护进程 | 需 Node.js | 需安装 |

> 比 Docker/SRT 快 **3-5 倍**，内存占用低 **10 倍**。

<details>
<summary>性能测试详情与命令</summary>

![Performance Benchmark Comparison](../images/benchmark-en.png)

```bash
cd benchmark/
python benchmark_runner.py --compare-levels --compare-ipc -n 100 -c 10

# 冷启动对比
python benchmark_runner.py --cold-start --compare-ipc

# 完整测试：冷启动 + 高并发
python benchmark_runner.py --cold-start --cold-iterations 20 --compare-levels --compare-ipc -o results.json
```

详见 [benchmark/README.md](../../benchmark/README.md)。

</details>

---

## 🎯 为什么选择 SkillLite？

**一句话**：其他 Agent 框架要么聪明但不安全，要么安全但不进化。SkillLite 两者兼具——Agent 越用越强，每一个进化产物都受 OS 级沙箱安全约束。

- **vs Agent 框架**（AutoGen、CrewAI、LangGraph）：它们提供编排能力但没有内置进化或沙箱。SkillLite 自主进化且安全执行。
- **vs 沙箱工具**（E2B、Docker、Claude SRT）：它们提供隔离但没有智能层。SkillLite 在其上增加了完整的 Agent 循环 + 自进化。
- **vs 进化平台**（OpenClaw Foundry、EvoAgentX）：它们支持进化但不对进化产物施加安全约束。SkillLite 对所有进化产物强制 L3 扫描 + OS 沙箱。

> Claude/Anthropic 的 [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing) 使用了**同样的底层沙箱技术**（Seatbelt + bubblewrap）。详见[架构对比分析](./CLAUDE-CODE-OPENCLAW-ARCHITECTURE-COMPARISON.md)。

---

## 🚀 快速开始

### 安装（推荐：pip）

```bash
pip install skilllite
skilllite init        # 沙箱二进制 + skills/ + 下载 skills
skilllite list        # 验证安装
```

**零配置快速开始**（自动检测 LLM、配置 skills、启动对话）：

```bash
skilllite quickstart
```

### 运行第一个示例

```python
from skilllite import chat

result = chat("帮我计算 15 乘以 27", skills_dir="skills")
print(result)
```

或使用 CLI：`skilllite chat`

### 环境配置

```bash
cp .env.example .env   # 编辑: BASE_URL, API_KEY, MODEL
```

| 文件 | 说明 |
|------|------|
| [.env.example](../../.env.example) | 快速开始模板 |
| [.env.example.full](../../.env.example.full) | 完整变量列表 |
| [ENV_REFERENCE.md](./ENV_REFERENCE.md) | 完整变量说明 |

> **平台支持**：macOS、Linux 和 Windows（通过 WSL2 桥接）。

---

## 📚 教程

| 教程 | 时长 | 说明 |
|------|------|------|
| [01. 基础用法](../../tutorials/01_basic) | 5 分钟 | 最简示例，一行执行 |
| [02. Skill 管理](../../tutorials/02_skill_management) | 10 分钟 | 创建和管理 skills |
| [03. Agentic Loop](../../tutorials/03_agentic_loop) | 15 分钟 | 多轮对话与工具调用 |
| [04. LangChain 集成](../../tutorials/04_langchain_integration) | 15 分钟 | LangChain 框架集成 |
| [05. LlamaIndex 集成](../../tutorials/05_llamaindex_integration) | 15 分钟 | RAG + skill 执行 |
| [06. MCP 服务器](../../tutorials/06_mcp_server) | 10 分钟 | Claude Desktop 集成 |
| [07. OpenCode 集成](../../tutorials/07_opencode_integration) | 10 分钟 | 一键 OpenCode 集成 |

👉 **[查看全部教程](../../tutorials/README.md)**

---

## 💡 用法

### 直接执行 Skill

```python
from skilllite import run_skill

result = run_skill("./skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

### Skills 仓库管理

```bash
skilllite add owner/repo                    # 添加 GitHub 仓库中的所有 skills
skilllite add owner/repo@skill-name         # 按名称添加指定 skill
skilllite add ./local-path                  # 从本地目录添加
skilllite list                              # 列出所有已安装 skills
skilllite remove <skill-name>               # 移除已安装的 skill
```

### 框架集成

```bash
pip install langchain-skilllite   # LangChain 适配器
```

```python
from langchain_skilllite import SkillLiteToolkit
from langgraph.prebuilt import create_react_agent

tools = SkillLiteToolkit.from_directory(
    "./skills",
    sandbox_level=3,  # 1=无沙箱, 2=仅沙箱, 3=沙箱+扫描
    confirmation_callback=lambda report, sid: input("继续? [y/N]: ").lower() == 'y'
)
agent = create_react_agent(ChatOpenAI(model="gpt-4"), tools)
```

详见 [05. LlamaIndex 集成](../../tutorials/05_llamaindex_integration/README.md)。

### 安全等级

| 等级 | 说明 |
|------|------|
| 1 | 无沙箱 — 直接执行 |
| 2 | 仅沙箱隔离 |
| 3 | 沙箱 + 静态安全扫描（高危问题需确认） |

### 支持的 LLM 提供商

| 提供商 | base_url |
|--------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen (通义千问) | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot (月之暗面) | `https://api.moonshot.cn/v1` |
| Ollama (本地) | `http://localhost:11434/v1` |

---

## 🛠️ 创建自定义 Skill

每个 Skill 是一个包含 `SKILL.md` 的目录：

```
my-skill/
├── SKILL.md           # Skill 元数据（必需）
├── scripts/main.py    # 入口脚本
├── references/        # 参考文档（可选）
└── assets/            # 资源文件（可选）
```

<details>
<summary>SKILL.md 示例</summary>

```markdown
---
name: my-skill
description: 我的自定义 Skill，用于处理某些任务。
license: MIT
compatibility: Requires Python 3.x with requests library, network access
metadata:
  author: your-name
  version: "1.0"
---

# My Skill

这是 Skill 的详细说明。

## 输入参数

- `query`: 输入查询字符串（必需）

## 输出格式

返回 JSON 格式结果。
```

> 依赖通过 `compatibility` 字段声明（而非 `requirements.txt`）。入口点自动检测（`main.py` > `main.js` > `main.ts` > `main.sh`）。

</details>

---

## 📦 Crates 模块化架构

SkillLite 是一个 Cargo workspace，由职责单一、可独立编译的 crate 组成。

```
skilllite/                         依赖流向
├── Cargo.toml                     ────────────────────────────
├── skilllite/  (主二进制)          skilllite (CLI 入口)
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
                                          skilllite-core (基础层)
```

| Crate | 职责 | 层 |
|-------|------|---|
| **skilllite-core** | 基础层 — 配置、技能元数据、路径校验、可观测性 | 共享 |
| **skilllite-sandbox** | **安全沙箱** — OS 原生隔离、静态扫描、供应链审计、资源限制。可独立交付为 `skilllite-sandbox` 二进制 | 🔒 沙箱 |
| **skilllite-evolution** | **自进化引擎** — 反馈收集 → 反思 → 进化 → 质量门 → 审计。驱动 prompt / memory / skill 进化 | 🧬 进化 |
| **skilllite-executor** | 会话管理 — 对话记录、记忆存储、向量搜索（可选） | Agent |
| **skilllite-agent** | LLM Agent 循环 — 多轮对话、工具编排、任务规划 | Agent |
| **skilllite-commands** | CLI 命令实现 — 将各 crate 组装进 `skilllite` 二进制 | CLI |
| **skilllite-swarm** | P2P 集群 — mDNS 发现、节点网格、分布式任务路由 | 网络 |
| **skilllite-artifact** | 按 run 的 artifact 存储 — 本地目录（agent 默认）、可选 HTTP（主二进制启用 `artifact_http` 时含 `artifact-serve`） | 存储 / HTTP |
| **skilllite-assistant** | 桌面应用 — Tauri 2 + React，独立 GUI | 应用 |

> **两个可独立交付的二进制**：`skilllite`（完整版：进化 + Agent + 沙箱）和 `skilllite-sandbox`（轻量版：仅沙箱 + MCP，~3.6 MB）。沙箱对 agent 和 evolution crate 零依赖——其他框架（LangChain、AutoGen、CrewAI 等）可通过 CLI、MCP 或 Rust crate 直接嵌入。

### SDK 与集成

- **python-sdk**（`pip install skilllite`）— 薄桥接层，零 PyPI 运行时依赖；含 `artifact_put` / `artifact_get`（HTTP，标准库）与二进制桥接 API
- **langchain-skilllite**（`pip install langchain-skilllite`）— LangChain / LangGraph 适配器

<details>
<summary>CLI 命令</summary>

| 命令 | 说明 |
|------|------|
| `skilllite init` | 初始化项目（skills/ + 下载 skills + 依赖 + 审计） |
| `skilllite quickstart` | 零配置：检测 LLM、配置 skills、启动对话 |
| `skilllite chat` | 交互式 Agent 对话（或 `--message` 单次对话） |
| `skilllite add owner/repo` | 从 GitHub 添加 skills |
| `skilllite remove <name>` | 移除已安装的 skill |
| `skilllite list` | 列出已安装 skills |
| `skilllite show <name>` | 显示 skill 详情 |
| `skilllite run <dir> '<json>'` | 直接执行 skill |
| `skilllite scan <dir>` | 扫描 skill 安全性 |
| `skilllite evolution status` | 查看进化指标和历史 |
| `skilllite evolution backlog` | 查询进化提案 backlog（状态/风险/ROI/acceptance_status） |
| `skilllite evolution run` | 强制触发进化周期 |
| `skilllite mcp` | 启动 MCP 服务器（Cursor/Claude Desktop） |
| `skilllite serve` | 启动 IPC 守护进程（stdio JSON-RPC） |
| `skilllite artifact-serve` | 按 run 的 artifact HTTP 服务（监听需 `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`） |
| `skilllite init-cursor` | 初始化 Cursor IDE 集成 |
| `skilllite init-opencode` | 初始化 OpenCode 集成 |
| `skilllite clean-env` | 清理缓存的运行时环境 |
| `skilllite reindex` | 重新索引所有已安装 skills |

> 说明：在默认 `skills` 模式下，若 `skills/` 不存在但 `.skills/` 存在，SkillLite 会自动回退到 `.skills/`。
> 当 `skills/<name>` 与 `.skills/<name>` 同时存在时，命令会输出同名冲突告警，便于定位问题。
> 工作区技能发现还支持 `.agents/skills/` 与 `.claude/skills/` 作为规范搜索根目录。

</details>

<details>
<summary>从源码构建</summary>

### 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### 构建命令

| 包 | 二进制 | 命令 | 说明 |
|---|---|---|---|
| skilllite | **skilllite** | `cargo build -p skilllite` | **完整版**（进化 + Agent + 沙箱 + MCP；含 `artifact-serve` 代码，监听需 `SKILLLITE_ARTIFACT_SERVE_ALLOW=1`） |
| skilllite | **skilllite** | `cargo build -p skilllite --features memory_vector` | 完整版 **+ 向量记忆**搜索 |
| skilllite | **skilllite** | `cargo build -p skilllite --no-default-features` | 最小版：仅 run/exec/bash/scan |
| skilllite | **skilllite-sandbox** | `cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | 仅沙箱 + MCP |

### 安装到 `~/.cargo/bin/`

| 命令 | 获得 |
|---|---|
| `cargo install --path skilllite` | **skilllite** — 完整版 |
| `cargo install --path skilllite --features memory_vector` | **skilllite** — 完整版 + 向量记忆 |
| `cargo install --path skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary` | **skilllite-sandbox** — 仅沙箱 + MCP |

**默认 features** = `sandbox`、`audit`、`agent`、`swarm`、`artifact_http`。向量记忆（`memory_vector`）**不在**默认中。**`skilllite artifact-serve`** 默认编入二进制，但**未设置 `SKILLLITE_ARTIFACT_SERVE_ALLOW=1` 时不会监听**，避免误暴露。

</details>

<details>
<summary>OpenCode 集成</summary>

```bash
pip install skilllite
skilllite init-opencode   # 自动配置 OpenCode MCP
opencode
```

`init-opencode` 命令自动检测最佳启动方式、创建 `opencode.json`、发现项目中的技能。

</details>

<details>
<summary>桌面助手（skilllite-assistant）</summary>

**SkillLite Assistant** 是本机 **桌面应用**（Tauri 2 + React），底层仍是同一套 **`skilllite`** 引擎（`agent-rpc`）。**安装包**（dmg / msi / AppImage）见 [GitHub Releases](https://github.com/EXboys/skilllite/releases)（桌面构建可能比主产物稍晚出现）。首次分流说明：[选择你的路径 → 桌面](./START_PATHS.md#path-1-desktop)。

**设置**（齿轮）：**模型与 API**（界面语言、服务商、API Key、模型、可选 Base URL、Token 统计等）、**工作区与沙箱**、**Agent 预算**、**自进化**、**定时任务**、**卸载与数据**。

![SkillLite Assistant — 设置「模型与 API」](../images/assistant-settings-model-api.png)

Tauri 2 + React 桌面应用，位于 `crates/skilllite-assistant/`：

```bash
cd crates/skilllite-assistant
npm install
npm run tauri dev    # 开发模式（HMR）
npm run tauri build
```

聊天输入行为：`Enter` 发送；`Shift+Enter` 换行；**输入法组合输入（未上屏）**时回车交给输入法确认选词，不会误触发发送。
在 **设置 → 模型与 API** 保存时会合并当前模型、Base 与 Key 到本地列表；输入框底栏左侧可在已保存配置间切换。
**MiniMax M2 / M2.7**（OpenAI 兼容 `api.minimax.io`）：部分流式响应把增量正文放在 `choices[].delta.reasoning_details[].text`（字符串或 `{ "value": "…" }`），`delta.content` 可能一直为空；内置 agent 会转发这些片段。工具跑完后，「助手是否已有足够正文」的判断改为与界面相同的可见文本清洗（不再只看原始长度），避免长段仅含 thinking 时被误判为「已有正文」从而跳过工具摘要回退，第二轮问天气等场景不再容易出现空气泡。
顶栏 **IDE** 或**设置 → 工作区与沙箱**可开启**IDE 三栏主界面**（工作区文件树、编辑器、对话）；**三栏之间的竖线可拖拽**调宽（会记住宽度）。关闭后恢复右侧状态栏。文件树会跳过 `.DS_Store` / `Thumbs.db`；非 UTF‑8 文件在 IDE 文本区会显示中文说明而非底层解码英文报错。`read_file` 工具结果在对话中以约 **5 行可滚动**预览展示，**点击预览**可在已知路径时在 IDE 中打开该文件，否则打开全屏阅读；`list_directory` 为同样高度的可滚动目录树。详情见助手目录内 README。
任务计划、工具调用、**工具回复**与待处理的**执行确认**均位于可折叠的**内部步骤**时间线中（需要操作时会自动展开；在可信环境可在**设置 → Agent 预算**或输入框下开启**自动允许执行确认**）。
当工具结果被标记为 `partial_success` 或 `failure` 时，助手会弹出多选恢复项，其中包含 `【启动进化】`，用于将能力补齐请求写入受治理的进化 backlog。
默认系统/规划/执行提示词倾向于在缺少专用技能时仍通过**脚本、`run_command`、小技能**等方式推进浏览器/桌面类需求，而不是一句「做不到」结束。客户端升级若提高 prompt **种子版本**，下次启动时可能用新模板**覆盖** `~/.skilllite/chat/prompts/` 下与新版不一致的文件（含从旧版官方模板升级的情况）；若你改过这些文件，请先**自行备份**。
**卸载**：**设置 → 卸载与数据** 可选择仅卸载应用或一并删除本机助手数据（详见助手 README）。

**Windows**：后台子进程（内置引擎自检、Life Pulse、运行时探测、以及代理里通过 `run_command` 拉起的 shell）使用无控制台窗口方式创建，正常使用时不应再反复闪现空的「命令提示符」黑框。

详见 [crates/skilllite-assistant/README.md](../../crates/skilllite-assistant/README.md)。

</details>

---

## 🤝 上游贡献

SkillLite 的沙箱加固经验与方案已贡献至 [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw)（[issue #4812](https://github.com/zeroclaw-labs/zeroclaw/issues/4812)），并被采纳合并（[PR #4821](https://github.com/zeroclaw-labs/zeroclaw/pull/4821)），提升了其原生沙箱的安全性（seccomp、权限收敛、后端不可用时的失败关闭策略）。

---

## 📄 License

MIT — 第三方依赖详见 [THIRD_PARTY_LICENSES.md](../../THIRD_PARTY_LICENSES.md)。

## 📚 文档

- [选择你的路径](./START_PATHS.md) — 按目标分流（桌面 / MCP / 全栈）
- [快速入门](./GETTING_STARTED.md) — 安装和快速入门指南
- [更新日志](../../CHANGELOG.md) — 版本历史与升级说明
- [环境变量参考](./ENV_REFERENCE.md) — 完整环境变量说明
- [项目架构](./ARCHITECTURE.md) — 项目架构和设计
- [架构对比分析](./CLAUDE-CODE-OPENCLAW-ARCHITECTURE-COMPARISON.md) — Claude Code vs OpenClaw vs OpenCode vs SkillLite
- [贡献指南](./CONTRIBUTING.md) — 如何贡献代码
