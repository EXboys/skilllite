# SkillLite

[English](./README.md)


一个轻量级的 AI Agent Skills 执行引擎，支持与任意 OpenAI 兼容的 LLM 集成。

## 🎯 为什么选择 SkillLite？

| 特性 | SkillLite | Claude Code Sandbox | LangChain Sandbox | OpenAI Plugins | Semantic Kernel |
|------|------------|---------------------|-------------------|----------------|-----------------|
| **内置沙箱** | ✅ Rust 原生 | ✅ Node.js 原生 | ⚠️ Pyodide/Docker | ⚠️ 云端闭源 | ❌ 无（需 Azure） |
| **沙箱技术** | Seatbelt + Namespace | Seatbelt + bubblewrap | WebAssembly/Docker | 云端隔离 | - |
| **实现语言** | **Rust** (高性能) | Node.js/TypeScript | Python | - | C# |
| **本地执行** | ✅ | ✅ | ✅ | ❌ | ❌ |
| **零依赖** | ✅ 单二进制 | ❌ 需 Node.js | ❌ 需运行时 | ❌ | ❌ |
| **冷启动** | ⚡ 毫秒级 | 中等 | 🐢 秒级 | - | - |
| **LLM 无关** | ✅ 任意 LLM | ❌ 仅 Claude | ✅ | ❌ 仅 OpenAI | ✅ |
| **开源协议** | MIT | Apache 2.0 | MIT | 闭源 | MIT |

### 与 Claude Code Sandbox 的关系

Claude/Anthropic 在 2025 年 10 月发布了 [Claude Code Sandbox](https://www.anthropic.com/engineering/claude-code-sandboxing)，采用了与 Claude Code Sandbox **相同的底层技术栈**：
- **macOS**: Seatbelt (sandbox-exec)
- **Linux**: bubblewrap + namespace

**关键差异**：

| 维度 | SkillLite | Claude Code Sandbox |
|------|------------|---------------------|
| **定位** | 通用 Skills 执行引擎 | Claude Code 专属功能 |
| **LLM 绑定** | ✅ 支持任意 LLM | ❌ 仅限 Claude |
| **实现语言** | **Rust** (更高性能、更小体积) | Node.js/TypeScript |
| **部署方式** | 单二进制，零依赖 | 依赖 Node.js  |
| **Skills 生态** | 独立 Skills 目录结构 | 依赖 MCP 协议 |
| **使用场景** | 任意 Agent 框架集成 | Claude Code 内部使用 |

> 💡 **总结**：Claude Code Sandbox 验证了"原生系统级沙箱"是 AI Agent 安全执行的正确方向。SkillLite 提供了一个 **LLM 无关、Rust 实现、更轻量** 的替代方案，适合需要集成多种 LLM 或追求极致性能的场景。

## 🔐 核心创新：原生系统级安全沙箱

SkillLite 使用 **Rust 实现的原生系统级沙箱**，而非 Docker 或 WebAssembly：

- **macOS**: 基于 Seatbelt (sandbox-exec) 的内核级隔离
- **Linux**: 基于 Namespace + Seccomp 的容器级隔离

### 与其他方案的本质区别

```
┌─────────────────────────────────────────────────────────────────┐
│  其他方案                                                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │   Docker    │  │   Pyodide   │  │  云端沙箱   │              │
│  │  (重量级)   │  │ (WebAssembly)│  │ (数据上传)  │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  SkillLite 方案                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │              Rust 原生系统级沙箱                             ││
│  │  • 直接调用操作系统安全机制（Seatbelt/Namespace）            ││
│  │  • 零外部依赖，单二进制文件                                  ││
│  │  • 毫秒级冷启动，生产级性能                                  ││
│  │  • 代码和数据永不离开本机                                    ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
```

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

## 🚀 快速开始

### 安装（推荐：pip）

```bash
# 安装 SkillLite SDK
pip install skilllite

# 安装沙箱二进制和初始化 skills 目录
skilllite init

# 验证安装
skilllite status
```

### Skills 仓库管理

```bash
# 从远程仓库添加 skills
skilllite add owner/repo                    # 添加 GitHub 仓库中的所有 skills
skilllite add owner/repo/skill-name         # 按路径添加指定 skill
skilllite add owner/repo@skill-name          # 按名称过滤添加
skilllite add https://github.com/owner/repo # 从完整 GitHub URL 添加
skilllite add ./local-path                  # 从本地目录添加
skilllite add owner/repo --list             # 列出可用 skills 但不安装
skilllite add owner/repo --force             # 强制覆盖已存在的 skills

# 管理已安装的 skills
skilllite list                              # 列出所有已安装 skills
skilllite remove <skill-name>                # 移除已安装的 skill
skilllite remove <skill-name> --force        # 无需确认直接移除
```

无需 Rust、Docker 或复杂配置。

> ⚠️ **平台支持**：目前仅支持 **macOS** 和 **Linux**，暂不支持 Windows。

### 环境配置

```bash
# 复制环境变量模板并填入 API 配置
cp .env.example .env
# 编辑 .env: BASE_URL, API_KEY, MODEL
```

| 文件 | 说明 |
|------|------|
| [.env.example](./.env.example) | 快速开始模板（5-8 个常用变量） |
| [.env.example.full](./.env.example.full) | 完整变量列表（高级用户） |
| [docs/zh/ENV_REFERENCE.md](./docs/zh/ENV_REFERENCE.md) | 完整变量说明、默认值、使用场景 |

### 运行示例

```bash
python3 simple_demo.py
```

## 📁 项目结构

```
skillLite/
├── skilllite/              # Rust 沙箱执行器（CLI: chat/add/list/mcp/run/exec）
├── python-sdk/             # Python SDK
│   └── skilllite/
│       ├── api.py         # chat, run_skill, scan_code, execute_code
│       ├── binary.py      # 二进制管理
│       ├── cli.py         # CLI 入口（转发到 binary）
│       └── ipc.py         # IPC 客户端
├── langchain-skilllite/    # LangChain 适配器（独立包）
├── .skills/                # Skills 目录
├── simple_demo.py          # 完整示例（使用 chat API）
└── tutorials/             # 教程
```

## 💡 使用方法

### 基础用法（chat API）

```python
from skilllite import chat

# 单次 Agent 对话（使用 .env 中的 API 配置）
result = chat("帮我计算 15 乘以 27", skills_dir=".skills")
print(result)
```

### 直接执行 Skill

```python
from skilllite import run_skill

result = run_skill("./.skills/calculator", '{"operation": "add", "a": 15, "b": 27}')
print(result["text"])
```

### 框架集成（LangChain / LlamaIndex）

如需与 LangChain 或 LlamaIndex Agent 集成，请使用对应适配器：

```bash
pip install langchain-skilllite   # LangChain
pip install skilllite[llamaindex] # LlamaIndex（可选）
```

详见 [04. LangChain 集成](./tutorials/04_langchain_integration) 和 [05. LlamaIndex 集成](./tutorials/05_llamaindex_integration)。

### 支持的 LLM 提供商

| 提供商 | base_url |
|--------|----------|
| OpenAI | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Qwen (通义千问) | `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| Moonshot (月之暗面) | `https://api.moonshot.cn/v1` |
| Ollama (本地) | `http://localhost:11434/v1` |

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
description: 我的自定义 Skill
version: 1.0.0
entry_point: scripts/main.py
---

# My Skill

这是 Skill 的详细说明...
```

## 📦 核心组件

- **skilllite**（Rust 二进制）- 沙箱执行器、CLI（chat/add/list/mcp/run/exec）、MCP 服务器
- **chat** - Python API，用于单次 Agent 对话
- **run_skill** / **execute_code** / **scan_code** - Python API，用于直接执行
- **langchain-skilllite** - LangChain 适配器（SkillLiteToolkit、SkillManager）

## 🔌 OpenCode 集成

SkillLite 可以作为 MCP (Model Context Protocol) 服务器集成到 [OpenCode](https://github.com/opencode-ai/opencode)，为其提供安全沙箱执行能力。

### 一键集成

```bash
# 安装 SkillLite（含 MCP 支持）
pip install skilllite[mcp]

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

### 可用的 MCP 工具

| 工具 | 描述 |
|------|------|
| `skilllite_list_skills` | 列出所有可用技能 |
| `skilllite_get_skill_info` | 获取技能详情和参数 |
| `skilllite_run_skill` | 执行预定义技能 |
| `skilllite_scan_code` | 扫描代码安全性 |
| `skilllite_execute_code` | 在安全沙箱中执行代码 |

### 安全特性

- **系统级沙箱**：macOS Seatbelt / Linux Namespace 隔离
- **安全扫描**：执行前静态分析代码
- **用户确认**：危险代码需要明确批准
- **Scan ID 验证**：防止扫描和执行之间代码被篡改

详细文档请参阅 [OpenCode 集成教程](./tutorials/07_opencode_integration/README.md)。

## 📄 License

MIT

本项目包含各种许可证的第三方依赖项。详见 [THIRD_PARTY_LICENSES.md](./THIRD_PARTY_LICENSES.md)。

## 📚 文档

- [快速入门](./docs/zh/GETTING_STARTED.md) - 安装和快速入门指南
- [环境变量参考](./docs/zh/ENV_REFERENCE.md) - 完整环境变量说明
- [项目架构](./docs/zh/ARCHITECTURE.md) - 项目架构和设计
- [贡献指南](./docs/zh/CONTRIBUTING.md) - 如何贡献代码
