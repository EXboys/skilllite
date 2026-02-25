# SkillLite 项目架构文档

> **说明**：本文档已同步至 v0.1.9 架构。Python SDK 为薄桥接层（~600 行），主要导出 `scan_code`、`execute_code`、`chat`、`run_skill`、`get_binary`，逻辑集中在 Rust 二进制。

## 📋 项目概述

**SkillLite** 是一个轻量级 AI Agent Skills 执行引擎，分为两层产品：

```
┌──────────────────────────────────────────────────────────┐
│  SkillLite Agent（上层产品）                               │
│  自有 agent 框架：chat, planning, memory, tools            │
│  定位：开箱即用的 AI 助手，Core 的最佳实践                  │
│  编译：skilllite（full binary）                            │
├──────────────────────────────────────────────────────────┤
│  SkillLite Core（底层引擎）                                │
│  沙箱执行 + 安全扫描 + Skills 管理 + MCP                    │
│  定位：可被任何 agent 框架集成的安全执行引擎                 │
│  编译：skilllite-sandbox（轻量 binary）                     │
└──────────────────────────────────────────────────────────┘
```

Agent 是 Core 的第一个客户，也是最好的参考实现。

### 核心特性

- **内置原生系统级沙箱**：使用 Rust 实现的原生系统级安全隔离
- **零依赖**：单一二进制文件，毫秒级冷启动
- **本地执行**：代码和数据永不离开本机
- **LLM 无关**：兼容所有 OpenAI API 格式的 LLM 提供商
- **跨平台**：支持 macOS (Seatbelt)、Linux (Namespace + Seccomp)、Windows (WSL2 Bridge)

### 技术栈

| 组件 | 技术 |
|------|------|
| 沙箱执行器 | Rust (skilllite 二进制) |
| Python SDK | Python 3.x (python-sdk) |
| macOS 沙箱 | Seatbelt (sandbox-exec) |
| Linux 沙箱 | Namespace + Seccomp (bubblewrap / firejail) |
| Windows 沙箱 | WSL2 桥接 |

### 核心场景

| 场景 | 说明 | 用户 |
|------|------|------|
| **被集成** | AI 框架需要安全执行不可信代码时，嵌入 SkillLite Core | 框架开发者、IDE 厂商 |
| **Skills 生态** | 标准化 AI 工具的打包（SKILL.md）、分发、安全执行 | Skills 开发者 |
| **安全合规** | 证明 AI 执行的代码不泄露数据、不破坏系统 | 企业 AI 团队 |
| **开箱即用** | `skilllite chat` 完整 agent 体验 | 终端开发者 |

---

## 🏗️ 项目结构

```
skillLite/
├── skilllite/                     # Rust 沙箱执行器 (核心)
│   ├── Cargo.toml                 # Rust 依赖配置
│   └── src/
│       ├── main.rs                # CLI 入口 (~210 行，仅参数解析和命令分发)
│       ├── cli.rs                 # 命令行参数定义
│       ├── mcp.rs                 # MCP 协议服务器
│       ├── stdio_rpc.rs           # Stdio JSON-RPC 服务
│       ├── observability.rs       # 可观测性 (tracing)
│       ├── path_validation.rs     # 路径验证
│       │
│       ├── commands/              # 命令实现
│       │   ├── execute.rs         # run_skill, exec_script, bash_command
│       │   ├── scan.rs            # scan_skill
│       │   ├── security.rs        # security_scan, dependency_audit
│       │   ├── skill.rs           # add, remove, list, show
│       │   ├── ide.rs             # Cursor / OpenCode 集成
│       │   ├── init.rs            # 项目初始化
│       │   ├── quickstart.rs      # 快速开始 (agent feature)
│       │   ├── env.rs             # 环境管理 (clean)
│       │   ├── reindex.rs         # 重新索引 skills
│       │   └── planning_rules_gen.rs  # 规划规则生成
│       │
│       ├── config/                # 配置模块
│       │   ├── loader.rs          # 环境变量加载 + set_var 安全包装
│       │   ├── schema.rs          # 配置模式 (LlmConfig 等)
│       │   └── env_keys.rs        # 环境变量 key 常量
│       │
│       ├── env/                   # 运行时环境
│       │   └── builder.rs         # build_runtime_paths, ensure_environment
│       │
│       ├── skill/                 # Skill 元数据解析
│       │   ├── metadata.rs        # SKILL.md 解析
│       │   ├── schema.rs          # Skill 模式定义
│       │   ├── deps.rs            # 依赖管理
│       │   └── dependency_resolver.rs  # 依赖解析器
│       │
│       ├── sandbox/               # 沙箱实现 (核心安全模块)
│       │   ├── runner.rs          # SandboxLevel, SandboxConfig, ResourceLimits
│       │   ├── common.rs          # 跨平台通用功能 (内存监控等)
│       │   ├── macos.rs           # macOS Seatbelt 沙箱
│       │   ├── linux.rs           # Linux Namespace 沙箱
│       │   ├── windows.rs         # Windows WSL2 桥接
│       │   ├── seatbelt.rs        # Seatbelt profile 和强制拒绝规则
│       │   ├── seccomp.rs         # Linux Seccomp BPF 过滤器
│       │   ├── network_proxy.rs   # HTTP/SOCKS5 网络代理 (域名过滤)
│       │   ├── bash_validator.rs  # Bash 命令安全验证
│       │   ├── move_protection.rs # 文件移动保护
│       │   ├── log.rs             # 沙箱日志
│       │   └── security/          # 安全扫描子模块
│       │       ├── scanner.rs     # 静态代码扫描器
│       │       ├── rules.rs       # 安全规则定义和匹配
│       │       ├── types.rs       # 安全类型定义
│       │       ├── policy.rs      # 运行时安全策略
│       │       ├── default_rules.rs   # 默认规则实现
│       │       ├── default_rules.yaml # 可配置的规则文件
│       │       └── dependency_audit.rs # 供应链漏洞扫描 (OSV API)
│       │
│       ├── executor/              # 执行器模块 (executor feature)
│       │   ├── session.rs         # 会话管理
│       │   ├── transcript.rs      # 对话记录
│       │   ├── memory.rs          # 内存存储 (BM25 检索)
│       │   └── rpc.rs             # Executor RPC
│       │
│       └── agent/                 # Agent 循环 (agent feature)
│           ├── chat.rs            # CLI 聊天入口 (单次/REPL)
│           ├── agent_loop.rs      # Agent 主循环
│           ├── llm.rs             # LLM 客户端 (OpenAI/Claude)
│           ├── chat_session.rs    # 会话管理
│           ├── prompt.rs          # Prompt 构建
│           ├── skills.rs          # Skill 加载和管理
│           ├── rpc.rs             # Agent RPC (JSON-Lines 事件流)
│           ├── task_planner.rs    # 任务规划器
│           ├── planning_rules.rs  # 规划规则
│           ├── types.rs           # Agent 类型定义
│           ├── long_text/         # 长文本处理
│           │   ├── mod.rs
│           │   └── filter.rs
│           └── extensions/        # 工具扩展
│               ├── registry.rs    # 统一扩展注册表
│               ├── memory.rs      # 内存工具 (search/write/list)
│               └── builtin/       # 内置工具
│                   ├── file_ops.rs     # read_file, write_file, search_replace 等
│                   ├── run_command.rs  # run_command + 危险命令检测
│                   ├── output.rs      # write_output, list_output
│                   ├── preview.rs     # preview_server (内置 HTTP 服务)
│                   └── chat_data.rs   # chat_history, chat_plan
│
├── python-sdk/                    # Python SDK (薄桥接层)
│   ├── pyproject.toml             # 包配置 (v0.1.9, 零运行时依赖)
│   └── skilllite/
│       ├── __init__.py            # 导出 chat, run_skill, scan_code, execute_code
│       ├── api.py                 # 核心 API (subprocess 调用 skilllite 二进制)
│       ├── binary.py              # 二进制管理 (bundled/PATH 解析)
│       ├── cli.py                 # CLI 入口 (转发到 binary)
│       └── ipc.py                 # IPC 客户端
│
├── langchain-skilllite/           # LangChain 适配器 (独立包, v0.1.8)
│   └── langchain_skilllite/
│       ├── core.py                # SkillManager, SkillInfo
│       ├── tools.py               # SkillLiteTool, SkillLiteToolkit
│       └── callbacks.py           # 回调处理器
│
├── benchmark/                     # 性能测试
│   ├── benchmark_runner.py        # 性能基准 (冷启动/高并发)
│   ├── security_vs.py             # 安全性对比测试
│   └── security_detailed_vs.py    # 详细安全对比
│
├── .skills/                       # Skills 目录 (示例技能)
│   ├── agent-browser/             # 浏览器自动化
│   ├── calculator/                # 计算器
│   ├── csdn-article/             # CSDN 文章
│   ├── data-analysis/            # 数据分析
│   ├── frontend-design/          # 前端设计
│   ├── http-request/             # HTTP 请求
│   ├── nodejs-test/              # Node.js 测试
│   ├── skill-creator/            # Skill 创建器
│   ├── text-processor/           # 文本处理
│   ├── weather/                  # 天气查询
│   ├── writing-helper/           # 写作助手
│   └── xiaohongshu-writer/       # 小红书写作
│
├── tutorials/                     # 教程示例
├── test/                          # 集成测试
├── tests/                         # 额外测试
├── scripts/                       # 构建脚本
├── docs/                          # 文档 (中英文)
│   ├── zh/                        # 中文文档
│   └── en/                        # 英文文档
│
├── install.sh                     # Unix 安装脚本
├── install.ps1                    # Windows 安装脚本
├── simple_demo.py                 # 完整示例
└── README.md                      # 项目说明
```

---

## 🔐 核心模块详解

### 1. Rust 三层架构

```
入口层 (CLI/MCP/stdio_rpc) → Agent → Executor → Sandbox → Core
Core 不依赖上层；Agent 是 Core 的客户，不是 Core 的一部分
```

**Feature Flags 控制编译**：

| Feature | 包含模块 | 编译目标 |
|---------|---------|---------|
| `sandbox` (默认) | sandbox, skill, config, env | 沙箱核心 |
| `audit` (默认) | dependency_audit (OSV API) | 供应链审计 |
| `executor` | session, transcript, memory | 会话管理 |
| `agent` (默认) | agent_loop, llm, chat, extensions | Agent 功能 |
| `sandbox_binary` | 仅 sandbox + core | skilllite-sandbox 轻量二进制 |
| `memory_vector` | sqlite-vec 向量检索 | 可选语义搜索 |

**编译目标**：
- `cargo build -p skilllite`：全量产品（chat/add/list/mcp/init 等）
- `cargo build --features sandbox_binary`：Core 引擎（run/exec/bash，无 agent）

### 2. 沙箱模块 (sandbox/)

#### 2.1 沙箱安全级别 (`sandbox/runner.rs`)

```rust
pub enum SandboxLevel {
    Level1,  // 无沙箱 - 直接执行，无隔离
    Level2,  // 仅沙箱隔离 (macOS Seatbelt / Linux namespace + seccomp)
    Level3,  // 沙箱隔离 + 静态代码扫描 (默认)
}
```

#### 2.2 SandboxConfig（解耦 sandbox ↔ skill）

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

sandbox 不再直接 `use crate::skill::*`，改为接收 `SandboxConfig`，由调用方从 `SkillMetadata` 构造。

#### 2.3 RuntimePaths（解耦 sandbox ↔ env）

```rust
pub struct RuntimePaths {
    pub python: PathBuf,
    pub node: PathBuf,
    pub node_modules: PathBuf,
    pub env_dir: PathBuf,
}
```

sandbox 不再 `use crate::env::builder::*`，改为接收 `RuntimePaths`，由 `env/builder.rs::build_runtime_paths()` 桥接构造。

#### 2.4 资源限制 (`sandbox/runner.rs`)

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,   // 默认 256MB
    pub timeout_secs: u64,    // 默认 30 秒
}
```

**环境变量**：
- `SKILLBOX_MAX_MEMORY_MB`: 最大内存限制
- `SKILLBOX_TIMEOUT_SECS`: 执行超时
- `SKILLBOX_SANDBOX_LEVEL`: 沙箱级别 (1/2/3)
- `SKILLBOX_AUTO_APPROVE`: 自动批准危险操作

#### 2.5 macOS 沙箱实现 (`sandbox/macos.rs`)

**核心技术**: 使用 macOS 的 `sandbox-exec` 和 Seatbelt 配置文件

**执行流程**：
1. 检查是否禁用沙箱 (`SKILLBOX_NO_SANDBOX`)
2. 启动网络代理（如果启用网络且有域名白名单）
3. 生成 Seatbelt 配置文件（限制文件系统、网络访问）
4. 使用 `sandbox-exec` 启动子进程
5. 监控内存使用和执行时间
6. 超限时终止进程

#### 2.6 Linux 沙箱实现 (`sandbox/linux.rs`)

**沙箱工具优先级**：bubblewrap (bwrap) → firejail → 报错

**Bubblewrap 隔离**：
- `--unshare-all`：取消共享所有命名空间
- 最小文件系统挂载（只读 /usr, /lib, /bin）
- Skill 目录只读挂载
- 网络隔离（默认 `--unshare-net`，启用时 `--share-net` 通过代理过滤）
- Seccomp BPF 过滤器阻止 AF_UNIX socket 创建

#### 2.7 Windows 沙箱实现 (`sandbox/windows.rs`)

通过 WSL2 桥接实现沙箱功能。

#### 2.8 网络代理 (`sandbox/network_proxy.rs`)

提供 HTTP 和 SOCKS5 代理，用于域名白名单过滤。当 skill 声明了网络访问但限制了出站域名时，代理会拦截非白名单请求。

#### 2.9 静态代码扫描 (`sandbox/security/`)

安全扫描模块包含以下文件：

| 文件 | 职责 |
|------|------|
| `scanner.rs` | 扫描器主逻辑 (ScriptScanner) |
| `rules.rs` | 安全规则定义和匹配 |
| `types.rs` | 安全类型定义 |
| `policy.rs` | 运行时安全策略 (路径/进程/网络) |
| `default_rules.rs` | 默认规则实现 |
| `default_rules.yaml` | 可配置的规则文件 |
| `dependency_audit.rs` | 供应链漏洞扫描 (OSV API, 需要 audit feature) |

**安全问题类型** (`security/types.rs`)：
```rust
pub enum SecurityIssueType {
    FileOperation,      // 文件操作
    NetworkRequest,     // 网络请求
    CodeInjection,      // 代码注入 (eval, exec)
    MemoryBomb,         // 内存炸弹
    ProcessExecution,   // 进程执行
    SystemAccess,       // 系统访问
    DangerousModule,    // 危险模块导入
}

pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

#### 2.10 其他安全模块

| 模块 | 职责 |
|------|------|
| `bash_validator.rs` | Bash 命令安全验证，检测危险命令 |
| `move_protection.rs` | 文件移动保护，防止恶意文件覆盖 |
| `seatbelt.rs` | macOS 强制拒绝路径和 Seatbelt profile 生成 |

---

### 3. 执行器模块 (executor/)

> 需要 `executor` feature，提供会话管理和持久化能力。

| 模块 | 职责 |
|------|------|
| `session.rs` | 会话生命周期管理 |
| `transcript.rs` | 对话记录持久化 |
| `memory.rs` | 内存存储 (BM25 检索，可选 sqlite-vec 向量搜索) |
| `rpc.rs` | Executor RPC 接口 |

**数据存储路径**：`~/.skilllite/`（聊天记录、会话数据、内存索引）

---

### 4. Agent 模块 (agent/)

> 需要 `agent` feature（默认启用），提供完整的 AI Agent 功能。

#### 4.1 核心模块

| 模块 | 职责 |
|------|------|
| `chat.rs` | CLI 聊天入口（单次 `--message` / 交互式 REPL） |
| `agent_loop.rs` | Agent 主循环（LLM 调用 → 工具执行 → 结果返回） |
| `llm.rs` | LLM HTTP 客户端（支持 OpenAI 兼容 API 和 Claude Native API，流式/非流式） |
| `chat_session.rs` | 聊天会话管理 |
| `prompt.rs` | 系统提示词构建 |
| `skills.rs` | Skill 加载和工具定义生成 |
| `rpc.rs` | Agent RPC 服务器（JSON-Lines 事件流协议） |
| `task_planner.rs` | 任务规划器 |
| `planning_rules.rs` | 规划规则配置 |
| `types.rs` | Agent 类型定义 |

#### 4.2 长文本处理 (`long_text/`)

自动检测和处理超长文本输出，避免 LLM 上下文溢出。

#### 4.3 工具扩展系统 (`extensions/`)

**注册表模式**（编译时注册）：

```rust
registry.register(builtin::file_ops::tools());
registry.register(builtin::run_command::tools());
registry.register(memory::tools());
// 新增工具 = 加一行注册，不改 agent_loop
```

**内置工具** (`extensions/builtin/`)：

| 文件 | 工具 |
|------|------|
| `file_ops.rs` | read_file, write_file, search_replace, list_directory, file_exists |
| `run_command.rs` | run_command（带危险命令检测和用户确认） |
| `output.rs` | write_output, list_output |
| `preview.rs` | preview_server（内置 HTTP 文件服务器） |
| `chat_data.rs` | chat_history, chat_plan, update_task_plan |

**内存工具** (`extensions/memory.rs`)：

| 工具 | 说明 |
|------|------|
| `memory_search` | 搜索历史对话记忆 |
| `memory_write` | 写入新记忆 |
| `memory_list` | 列出所有记忆 |

---

### 5. MCP 模块 (mcp.rs)

**MCP (Model Context Protocol) 服务器**：JSON-RPC 2.0 over stdio

**提供 5 个工具**：

| 工具 | 说明 |
|------|------|
| `list_skills` | 列出所有已安装的 skills |
| `get_skill_info` | 获取 skill 详细信息 |
| `run_skill` | 执行 skill（带安全扫描两阶段确认） |
| `scan_code` | 扫描代码安全性 |
| `execute_code` | 执行代码（带安全扫描两阶段确认） |

**两阶段确认机制**：先扫描（scan），用户确认后再执行（confirm）。扫描结果缓存 TTL 300 秒。

---

### 6. Stdio RPC 模块 (stdio_rpc.rs)

**技能执行 stdio RPC**：JSON-RPC 2.0 over stdio（一行一个请求）

使用 rayon 线程池处理并发请求，支持方法：`run`, `exec`, `bash`, `scan`, `validate`, `info` 等。

与 `agent::rpc` 分离——后者专用于 Agent Chat 流式事件。

---

### 7. Python SDK (python-sdk)

> **说明**：Python SDK 为薄桥接层（~600 行），零运行时依赖，通过 subprocess 调用 skilllite 二进制完成所有操作。

**模块与职责**：

| 模块 | 职责 |
|------|------|
| `api.py` | `scan_code`、`execute_code`、`chat`、`run_skill`，通过 subprocess 调用 skilllite 二进制 |
| `binary.py` | 二进制管理：`get_binary`、bundled/PATH 解析 |
| `cli.py` | CLI 入口，转发到 binary |
| `ipc.py` | IPC 客户端，与 `skilllite serve` 守护进程通信 |

**导出 API**：`scan_code`、`execute_code`、`chat`、`run_skill`、`get_binary`

**程序化 Agent**：使用 `skilllite chat --message` 或 `api.chat()` 调用 Rust Agent 循环。

---

### 8. LangChain 集成 (langchain-skilllite)

> 独立包 `pip install langchain-skilllite`（v0.1.8）

| 模块 | 职责 |
|------|------|
| `core.py` | SkillManager, SkillInfo — Skill 扫描和管理 |
| `tools.py` | SkillLiteTool, SkillLiteToolkit — LangChain 工具适配 |
| `callbacks.py` | 回调处理器 |

**依赖**：`langchain-core>=0.3.0`, `skilllite>=0.1.8`

---

### 9. Skill 元数据解析 (`skill/`)

#### 9.1 SKILL.md 格式

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

**字段说明**（遵循 Claude Agent Skills 规范）：

| 字段 | 必需 | 说明 |
|------|------|------|
| `name` | 是 | 技能名称，最多 64 字符，仅小写字母、数字和连字符 |
| `description` | 是 | 技能描述，最多 1024 字符 |
| `license` | 否 | 许可证名称或引用 |
| `compatibility` | 否 | 环境要求，最多 500 字符（用于推断网络权限、语言和依赖） |
| `metadata` | 否 | 额外元数据（author、version 等） |
| `allowed-tools` | 否 | 预批准的工具列表（实验性） |

#### 9.2 从 `compatibility` 字段推断配置

1. **网络权限**：包含 "network"、"internet"、"http"、"api"、"web" → 启用网络访问
2. **语言检测**：Python / Node / JavaScript / bash / shell
3. **依赖管理**：自动从 compatibility 提取已知包名并安装

#### 9.3 自动检测入口点

```rust
fn detect_entry_point(skill_dir: &Path) -> Option<String> {
    // 优先级: main.py > main.js > main.ts > main.sh
    // 然后: index.* > run.* > entry.* > app.* > cli.*
    // 最后: 如果只有一个脚本文件，使用它
}
```

#### 9.4 依赖解析 (`dependency_resolver.rs`)

独立的依赖解析器，支持从 SKILL.md 和 compatibility 字段自动解析、安装 Python/Node 依赖。

---

## 🔄 执行流程

### 完整执行流程

```
用户输入
    ↓
skilllite chat / api.chat() / skilllite chat --message
    ↓
Rust Agent (skilllite 二进制)
    ↓
┌─────────────────────────────────────┐
│ 1. 生成系统提示词 (含 Skill 信息)    │
│ 2. 调用 LLM                         │
│ 3. 解析工具调用                      │
│ 4. 执行工具 (内置工具 / Skill)      │
│ 5. 返回结果给 LLM                   │
│ 6. 重复直到完成或达到最大迭代次数    │
└─────────────────────────────────────┘
    ↓
Rust Sandbox.execute()
    ↓
┌─────────────────────────────────────┐
│ 1. 解析 SKILL.md 元数据             │
│ 2. 设置运行时环境 (RuntimePaths)     │
│ 3. Level 3: 静态代码扫描            │
│ 4. Level 2+: 启动系统级沙箱         │
│ 5. 执行脚本                         │
│ 6. 监控资源使用                      │
│ 7. 返回结果                         │
└─────────────────────────────────────┘
    ↓
返回执行结果
```

### CLI 命令一览

```bash
# 执行类
skilllite run <skill_dir> '<input_json>'       # 运行 Skill
skilllite exec <skill_dir> <script> '<json>'   # 直接执行脚本
skilllite bash <skill_dir> '<command>'         # 执行 Bash 命令

# 扫描类
skilllite scan <skill_dir>                     # 扫描 Skill
skilllite validate <skill_dir>                 # 验证 Skill
skilllite info <skill_dir>                     # 显示 Skill 信息
skilllite security-scan <script_path>          # 安全扫描
skilllite dependency-audit <skill_dir>         # 供应链审计

# Agent 类 (agent feature)
skilllite chat                                 # 交互式聊天
skilllite chat --message "..."                 # 单次对话
skilllite quickstart                           # 快速开始
skilllite agent-rpc                            # Agent RPC 服务器

# 管理类
skilllite add <source>                         # 添加 Skill
skilllite remove <skill_name>                  # 移除 Skill
skilllite list                                 # 列出所有 Skills
skilllite show <skill_name>                    # 显示 Skill 详情
skilllite list-tools                           # 列出工具定义

# 服务类
skilllite serve                                # IPC daemon (stdio JSON-RPC)
skilllite mcp                                  # MCP 协议服务器

# IDE 集成
skilllite init-cursor                          # 初始化 Cursor 集成
skilllite init-opencode                        # 初始化 OpenCode 集成

# 维护类
skilllite init                                 # 项目初始化
skilllite clean-env                            # 清理缓存环境
skilllite reindex                              # 重新索引 Skills
```

---

## 📦 Skill 结构

### 标准 Skill 目录结构

```
my-skill/
├── SKILL.md           # 必需：元数据和说明文档（包含依赖声明）
├── scripts/           # 脚本目录
│   └── main.py        # 入口脚本
├── references/        # 可选：参考文档
│   └── api-docs.md
└── assets/            # 可选：资源文件
    └── config.json
```

> **注意**：Python 依赖不再使用 `requirements.txt`，而是通过 `SKILL.md` 的 `compatibility` 字段声明。

### SKILL.md 完整示例

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

查询指定城市的天气信息。

## 输入参数

- `city`: 城市名称 (必需)

## 输出格式

返回 JSON 格式的天气数据。
```

---

## 🔧 关键配置

### 环境变量

```bash
# LLM 配置
BASE_URL=https://api.deepseek.com/v1
API_KEY=your_api_key
MODEL=deepseek-chat

# 沙箱配置
SKILLBOX_SANDBOX_LEVEL=3      # 1/2/3
SKILLBOX_MAX_MEMORY_MB=256    # 内存限制
SKILLBOX_TIMEOUT_SECS=30      # 超时时间
SKILLBOX_AUTO_APPROVE=false   # 自动批准危险操作
SKILLBOX_NO_SANDBOX=false     # 禁用沙箱
```

环境变量 key 定义在 `config/env_keys.rs`，支持 legacy 兼容。配置加载优先级：构造函数参数 > 环境变量 > .env 文件 > 默认值。

---

## 🛡️ 安全机制

### 1. 沙箱隔离

**macOS (Seatbelt)**:
- 文件系统隔离：只能访问 Skill 目录和临时目录
- 网络隔离：默认禁用，可按域名白名单开启（通过 network_proxy）
- 进程隔离：每个 Skill 独立进程

**Linux (Namespace + Seccomp)**:
- Mount namespace：隔离文件系统视图
- PID namespace：隔离进程空间
- Network namespace：隔离网络
- Seccomp BPF：限制系统调用（阻止 AF_UNIX socket 创建）
- 支持工具：bubblewrap (bwrap) 或 firejail

**Windows (WSL2 Bridge)**:
- 通过 WSL2 桥接至 Linux 沙箱

### 2. 静态代码扫描

**检测项**:
- 代码注入：`eval()`, `exec()`, `__import__()`
- 进程执行：`subprocess`, `os.system`
- 不安全反序列化：`pickle.loads`, `yaml.unsafe_load`
- 内存炸弹：大数组分配、无限循环
- 系统访问：环境变量、用户信息

### 3. 资源限制

- 内存限制：通过 RSS 监控，超限终止
- 时间限制：超时自动终止
- 进程数限制：防止 fork 炸弹

### 4. 强制拒绝路径 (`sandbox/seatbelt.rs`)

**始终阻止写入的敏感文件**：

| 类别 | 文件示例 |
|------|----------|
| Shell 配置 | `.bashrc`, `.zshrc`, `.profile` |
| Git 配置 | `.gitconfig`, `.git/hooks/*` |
| IDE 配置 | `.vscode/settings.json`, `.idea/*` |
| 包管理器 | `.npmrc`, `.pypirc`, `.cargo/config` |
| 安全文件 | `.ssh/*`, `.gnupg/*`, `.aws/credentials` |
| AI/Agent 配置 | `.mcp.json`, `.claude/*`, `.cursor/*` |

### 5. 供应链安全 (`security/dependency_audit.rs`)

使用 OSV (Open Source Vulnerabilities) API 扫描 Skill 依赖中的已知漏洞，需要 `audit` feature。

### 6. 其他保护

- **Bash 验证器** (`bash_validator.rs`)：检测危险 bash 命令
- **文件移动保护** (`move_protection.rs`)：防止恶意文件覆盖关键路径
- **用户授权**：Level 3 发现 Critical/High 级别问题时，请求用户确认后才执行

---

## 🔗 依赖关系

### Rust 依赖 (Cargo.toml)

```toml
[dependencies]
# 核心
clap = { version = "4", features = ["derive"] }  # CLI 解析
serde = { version = "1", features = ["derive"] } # 序列化
serde_yaml = "0.9"                               # YAML 解析
serde_json = "1.0"                               # JSON 解析
anyhow = "1.0"                                   # 错误处理
thiserror = "..."                                # 类型化错误
regex = "1.10"                                   # 正则表达式
tempfile = "3.10"                                # 临时文件
sha2 = "..."                                     # SHA 哈希
tracing = "..."                                  # 结构化日志
chrono = "..."                                   # 时间处理
rayon = "..."                                    # 线程池
zip = "..."                                      # ZIP 解压

# 可选 (feature-gated)
tokio = { ..., optional = true }                 # 异步运行时 (agent)
reqwest = { ..., optional = true }               # HTTP 客户端 (agent)
rusqlite = { ..., optional = true }              # SQLite (executor)
ureq = { ..., optional = true }                  # HTTP (audit)
sqlite-vec = { ..., optional = true }            # 向量搜索 (memory_vector)

# 平台特定
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.29", features = ["process", "mount", "sched", "signal"] }
libc = "0.2"

[target.'cfg(target_os = "macos")'.dependencies]
nix = { version = "0.29", features = ["process", "signal"] }
```

### Python SDK

零运行时依赖，通过打包的 skilllite 二进制完成所有操作。

---

## 🏛️ 防腐化原则

### 依赖规则

```
入口层(CLI/MCP/stdio_rpc) → Agent → Executor → Sandbox → Core
Core 不依赖上层；Agent 是 Core 的客户，不是 Core 的一部分
```

### 接口优先

- Sandbox 只依赖 `SandboxConfig` struct，不依赖 `SkillMetadata` 具体类型
- 新能力通过「注册」接入，禁止 `if tool_name == "xxx"` 硬编码

### 依赖纪律

| 层级 | 允许 | 禁止 |
|------|------|------|
| Core | serde, anyhow, regex, dirs | tokio, reqwest, rusqlite |
| Sandbox | core, tempfile, nix | tokio, reqwest |
| Executor | core, rusqlite | tokio |
| Agent | 全部 | — |

---

## 📝 重构指南

### 如果需要重构 Rust 沙箱

1. **保持 CLI 接口兼容**：`run`, `exec`, `scan`, `validate`, `info`, `security-scan`, `bash` 命令
2. **保持输出格式**：成功时输出 JSON 到 stdout，错误信息输出到 stderr
3. **安全级别逻辑**：Level 1 无沙箱 / Level 2 仅隔离 / Level 3 隔离+扫描
4. **解耦约定**：通过 `SandboxConfig` 和 `RuntimePaths` 传参，不直接依赖上层模块

### 如果需要添加新工具

1. 在 `agent/extensions/` 下创建模块，实现 `tool_definitions()` 和执行逻辑
2. 在 `extensions/registry.rs` 中注册工具
3. 不修改 `agent_loop.rs`

### 如果需要支持新平台沙箱

1. 在 `sandbox/` 下实现平台模块（如 `landlock.rs`）
2. 在 `sandbox/runner.rs` 中按平台选择后端
3. 通过 feature flag 控制编译

---

## 📌 注意事项

1. **不要修改 `.skills/` 目录**：这是示例 Skills，用户可能有自定义内容
2. **保持向后兼容**：API 变更需要考虑现有用户
3. **安全第一**：任何涉及沙箱的修改都需要仔细审查
4. **跨平台支持**：macOS、Linux、Windows 的沙箱实现不同，需要分别测试
5. **Feature Flag 纪律**：新模块应明确属于哪个 feature，避免不必要的依赖引入

---

*文档版本: 1.3.0*
*最后更新: 2026-02-21*
