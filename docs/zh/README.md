# SkillLite

[English](../../README.md)

**一个轻量级的 AI Agent Skills 执行引擎，内置原生系统级沙箱，零依赖，本地执行。**

支持与任意 OpenAI 兼容的 LLM 集成。

## ⚡ 性能基准测试

查看 SkillLite 与其他沙箱方案的实时性能对比：

[![Performance Benchmark Video](https://github.com/EXboys/skilllite/raw/main/docs/images/benchmark-en.gif)]

![Performance Benchmark Comparison](../images/benchmark-en.png)

### 运行基准测试

```bash
# 从项目根目录执行
python benchmark/benchmark_runner.py --compare-levels --compare-ipc -n 100 -c 10

# 冷启动对比（输出 COLD START BENCHMARK COMPARISON 表格）
python benchmark/benchmark_runner.py --cold-start --compare-ipc

# 完整测试：冷启动 + 高并发
python benchmark/benchmark_runner.py --cold-start --cold-iterations 20 --compare-levels --compare-ipc -o results.json
```

详见 [benchmark/README.md](../../benchmark/README.md)。

## 🎯 为什么选择 SkillLite？

| 特性 | SkillLite | Claude Code Sandbox | Pyodide | OpenAI Plugins | Semantic Kernel |
|------|------------|---------------------|---------|----------------|-----------------|
| **内置沙箱** | ✅ Rust 原生 | ✅ Node.js 原生 | ⚠️ Pyodide/Docker | ⚠️ 云端闭源 | ❌ 无（需 Azure） |
| **沙箱技术** | Seatbelt + Namespace | Seatbelt + bubblewrap | WebAssembly/Docker | 云端隔离 | - |
| **实现语言** | **Rust** (高性能) | Node.js/TypeScript | Python | - | C# |
| **本地执行** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **零依赖** | ✅ 单二进制 | ❌ 需 Node.js | ❌ 需运行时 | ❌ | ❌ |
| **冷启动** | ⚡ 毫秒级 | 中等 | 🐢 秒级 | - | - |
| **LLM 无关** | ✅ 任意 LLM | ❌ 仅 Claude | ✅ | ❌ 仅 OpenAI | ✅ |
| **开源协议** | MIT | Apache 2.0 | MIT | 闭源 | MIT |

> **性能亮点**：SkillLite 比 Docker 和 SRT 快 **3-5 倍**，内存占用低 **10 倍**（~10MB vs ~100MB）。

## 🚀 快速开始

### 安装（推荐：pip）

```bash
# 安装 SkillLite SDK
pip install skilllite

# 初始化项目（沙箱二进制 + .skills/ + 从 EXboys/skilllite 下载 skills）
skilllite init

# 验证安装
skilllite list

```

### Skills 仓库管理

```bash
# 从远程仓库添加 skills
skilllite add owner/repo                    # 添加 GitHub 仓库中的所有 skills
skilllite add owner/repo/skill-name         # 按路径添加指定 skill
skilllite add owner/repo@skill-name         # 按名称过滤添加
skilllite add https://github.com/owner/repo # 从完整 GitHub URL 添加
skilllite add ./local-path                  # 从本地目录添加
skilllite add owner/repo --list             # 列出可用 skills 但不安装
skilllite add owner/repo --force            # 强制覆盖已存在的 skills

# 管理已安装的 skills
skilllite list                              # 列出所有已安装 skills
skilllite remove <skill-name>               # 移除已安装的 skill
skilllite remove <skill-name> --force       # 无需确认直接移除
```

无需 Rust、Docker 或复杂配置。

**零配置快速开始**（自动检测 LLM、配置 skills、启动对话）：

```bash
skilllite quickstart
```

> **平台支持**：macOS、Linux 和 Windows（通过 WSL2 桥接）。

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

### 运行第一个示例

```python
from skilllite import chat

# 使用 .env 配置 API，.skills 作为工具目录
result = chat("帮我计算 15 乘以 27", skills_dir=".skills")
print(result)
```

或使用 CLI 进行交互式对话：`skilllite chat`

### 环境配置

```bash
# 复制模板并填入 LLM API 凭证
cp .env.example .env
# 编辑 .env: BASE_URL, API_KEY, MODEL
```

| 文件 | 说明 |
|------|------|
| [.env.example](../../.env.example) | 快速开始模板（5-8 个常用变量） |
| [.env.example.full](../../.env.example.full) | 完整变量列表（高级用户） |
| [ENV_REFERENCE.md](./ENV_REFERENCE.md) | 完整变量说明、默认值、使用场景 |

👉 **[查看全部教程](../../tutorials/README.md)**

## 安全对比测试

除性能测试外，我们还提供安全对比测试，评估沙箱方案对恶意行为的防护能力。

### 运行安全测试

```bash
# 完整测试（SkillLite + Docker + Pyodide + Claude SRT）
python3 benchmark/security_vs.py

# 仅测试 SkillLite
python3 benchmark/security_vs.py --skip-docker --skip-pyodide --skip-claude-srt

# 输出 JSON 结果
python3 benchmark/security_vs.py --output security_results.json
```

## 综合对比摘要

| 维度 | SkillLite | Docker | Pyodide | SRT |
|------|----------|--------|---------|-----|
| **热启动延迟** | 40 ms | 194 ms | 672 ms | 596 ms |
| **冷启动延迟** | 492 ms | 120s | ~5s | ~1s |
| **内存占用** | 10 MB | ~100 MB | ~50 MB | 84 MB |
| **安全性** | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ |
| **部署复杂度** | 单二进制 | 需守护进程 | 需 Node.js | 需安装 |
| **平台支持** | macOS/Linux/Win(WSL2) | 全平台 | 全平台 | macOS/Linux |

### 与 Claude Code Sandbox 的关系

Claude/Anthropic 在 2025 年 10 月发布了 [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing)，采用了与 SkillLite **相同的底层技术栈**：
- **macOS**: Seatbelt (sandbox-exec)
- **Linux**: bubblewrap + namespace

### 安全特性

| 安全能力 | 说明 |
|---------|------|
| **进程隔离** | 每个 Skill 在独立进程中执行 |
| **文件系统隔离** | 仅可访问 Skill 目录和临时目录 |
| **网络隔离** | 默认禁用网络，可按需开启 |
| **资源限制** | CPU、内存、执行时间限制 |
| **权限最小化** | 遵循最小权限原则 |

## ✨ 特性

- **🔒 原生安全沙箱** - Rust 实现的系统级隔离，非 Docker/WebAssembly
- **⚡ 极致轻量** - 单二进制文件，毫秒级冷启动，零外部依赖
- **🏠 数据主权** - 纯本地执行，代码和数据永不离开本机
- **🔌 通用 LLM 支持** - 兼容所有 OpenAI API 格式的 LLM 提供商
- **📦 Skills 管理** - 自动发现、注册和管理 Skills
- **🧠 智能 Schema 推断** - 自动从 SKILL.md 和脚本代码推断输入参数 Schema
- **🔧 Tool Calls 处理** - 无缝处理 LLM 的工具调用请求
- **📄 丰富的上下文支持** - 支持 references、assets 等扩展资源

## 🛠️ 创建自定义 Skill

每个 Skill 是一个包含 `SKILL.md` 的目录：

```
my-skill/
├── SKILL.md           # Skill 元数据和说明（必需）
├── scripts/           # 脚本目录
│   └── main.py        # 入口脚本
├── references/        # 参考文档（可选）
└── assets/            # 资源文件（可选）
```

### SKILL.md 示例

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

> **注意**：依赖通过 `compatibility` 字段声明（而非 `requirements.txt`）。入口点自动检测（`main.py` > `main.js` > `main.ts` > `main.sh`）。

## 📦 核心组件

- **skilllite**（Rust 二进制）- 沙箱执行器、CLI、Agent 循环、MCP 服务器——单二进制包含一切
- **python-sdk**（`pip install skilllite`）- 薄桥接层（~600 行），零运行时依赖，通过 subprocess 调用 Rust 二进制
- **langchain-skilllite**（`pip install langchain-skilllite`）- LangChain 适配器（SkillLiteToolkit）

### 主要 CLI 命令

| 命令 | 说明 |
|------|------|
| `skilllite init` | 初始化项目（.skills/ + 下载 skills + 依赖 + 审计） |
| `skilllite quickstart` | 零配置：检测 LLM、配置 skills、启动对话 |
| `skilllite chat` | 交互式 Agent 对话（或 `--message` 单次对话） |
| `skilllite add owner/repo` | 从 GitHub 添加 skills |
| `skilllite remove <name>` | 移除已安装的 skill |
| `skilllite list` | 列出已安装 skills |
| `skilllite show <name>` | 显示 skill 详情 |
| `skilllite run <dir> '<json>'` | 直接执行 skill |
| `skilllite scan <dir>` | 扫描 skill 安全性 |
| `skilllite mcp` | 启动 MCP 服务器（Cursor/Claude Desktop） |
| `skilllite serve` | 启动 IPC 守护进程（stdio JSON-RPC） |
| `skilllite init-cursor` | 初始化 Cursor IDE 集成 |
| `skilllite init-opencode` | 初始化 OpenCode 集成 |
| `skilllite clean-env` | 清理缓存的运行时环境 |
| `skilllite reindex` | 重新索引所有已安装 skills |

## 🔌 OpenCode 集成

SkillLite 可以作为 MCP (Model Context Protocol) 服务器集成到 [OpenCode](https://github.com/opencode-ai/opencode)，为其提供安全沙箱执行能力。

### 一键集成

```bash
# 安装 SkillLite（MCP 服务器已内置）
pip install skilllite

# 一键初始化（自动检测最佳配置）
skilllite init-opencode

# 启动 OpenCode
opencode
```

`init-opencode` 命令会自动：
- 检测最佳启动方式（uvx、pipx、skilllite 或 python）
- 创建 `opencode.json` 配置文件
- 生成 `.opencode/skills/skilllite/SKILL.md` 使用说明
- 发现项目中的预定义技能

### 框架集成（LangChain / LlamaIndex）

如需与 LangChain 或 LlamaIndex Agent 集成，请使用对应适配器：

```bash
pip install langchain-skilllite   # LangChain
```

详见 [04. LangChain 集成](../../tutorials/04_langchain_integration) 和 [05. LlamaIndex 集成](../../tutorials/05_llamaindex_integration)。

### 支持的 LLM 提供商

| 提供商 | base_url |
|--------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen (通义千问) | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot (月之暗面) | `https://api.moonshot.cn/v1` |
| Ollama (本地) | `http://localhost:11434/v1` |

## 📄 License

MIT

本项目包含各种许可证的第三方依赖项。详见 [THIRD_PARTY_LICENSES.md](../../THIRD_PARTY_LICENSES.md)。

## 📚 文档

- [快速入门](./GETTING_STARTED.md) - 安装和快速入门指南
- [环境变量参考](./ENV_REFERENCE.md) - 完整环境变量说明
- [项目架构](./ARCHITECTURE.md) - 项目架构和设计
- [贡献指南](./CONTRIBUTING.md) - 如何贡献代码
