# SkillLite 项目架构文档

> 本文档用于记录 SkillLite 项目的核心架构、设计理念和关键实现细节，方便后续 AI 进行快速重构和复刻。

## 📋 项目概述

**SkillLite** 是一个轻量级 AI Agent Skills 执行引擎，具有以下核心特性：

- **内置原生系统级沙箱**：使用 Rust 实现的原生系统级安全隔离
- **零依赖**：单一二进制文件，毫秒级冷启动
- **本地执行**：代码和数据永不离开本机
- **LLM 无关**：兼容所有 OpenAI API 格式的 LLM 提供商

### 技术栈

| 组件 | 技术 |
|------|------|
| 沙箱执行器 | Rust (skilllite 二进制) |
| Python SDK | Python 3.x (python-sdk) |
| macOS 沙箱 | Seatbelt (sandbox-exec) |
| Linux 沙箱 | Namespace + Seccomp |

---

## 🏗️ 项目结构

```
skillLite/
├── skilllite/                  # Rust 沙箱执行器 (核心)
│   ├── Cargo.toml             # Rust 依赖配置
│   └── src/
│       ├── main.rs            # CLI 入口 (chat/add/list/mcp/run/exec)
│       ├── cli.rs             # 命令行参数定义
│       ├── commands/           # 命令实现
│       ├── skill/             # Skill 元数据解析
│       ├── sandbox/           # 沙箱实现 (核心安全模块)
│       │   ├── executor.rs    # 沙箱执行器和安全级别
│       │   ├── macos.rs       # macOS Seatbelt 沙箱
│       │   ├── linux.rs       # Linux Namespace 沙箱
│       │   └── security/      # 安全扫描子模块
│       └── agent/             # Agent 循环 (chat 命令)
│
├── python-sdk/                 # Python SDK
│   └── skilllite/
│       ├── __init__.py         # 导出 chat, run_skill, scan_code, execute_code
│       ├── api.py              # chat, run_skill, scan_code, execute_code
│       ├── binary.py           # 二进制管理
│       ├── cli.py              # CLI 入口 (转发到 binary)
│       └── ipc.py              # IPC 客户端
│
├── langchain-skilllite/        # LangChain 适配器 (独立包)
├── benchmark/                  # 性能测试
│   ├── README.md               # 测试说明
│   ├── benchmark_runner.py     # 基准测试运行器
│   ├── benchmark_runner_v2.py  # 基准测试运行器 v2
│   ├── docker_vs.py            # Docker 对比测试
│   ├── pyodide_vs.py           # Pyodide 对比测试
│   ├── security_vs.py          # 安全性对比测试
│   ├── security_detailed_vs.py # 详细安全对比
│   └── srt_vs_skillbox_benchmark.py # SRT 对比测试
│
├── .skills/                    # Skills 目录 (示例技能)
│   ├── calculator/             # 计算器
│   ├── http-request/           # HTTP 请求
│   ├── text-processor/         # 文本处理
│   ├── data-analyzer/          # 数据分析
│   ├── nodejs-test/            # Node.js 测试
│   └── skill-creator/          # Skill 创建器
│
├── simple_demo.py              # 完整示例
├── simple_demo_v2.py           # 简化示例
├── simple_demo_minimal.py      # 最小示例
├── test_mcp_client.py          # MCP 客户端测试
├── test_mcp_interactive.py     # MCP 交互测试
│
├── CODE_OF_CONDUCT.md          # 行为准则
├── CONTRIBUTING.md             # 贡献指南
├── DOCUMENTATION_GUIDELINES.md # 文档规范
├── THIRD_PARTY_LICENSES.md     # 第三方许可证
└── README.md                   # 项目说明
```

---

## 🔐 核心模块详解

### 1. Rust 沙箱执行器 (skilllite)

#### 1.1 沙箱安全级别 (`sandbox/executor.rs`)

```rust
pub enum SandboxLevel {
    Level1,  // 无沙箱 - 直接执行，无隔离
    Level2,  // 仅沙箱隔离 (macOS Seatbelt / Linux namespace + seccomp)
    Level3,  // 沙箱隔离 + 静态代码扫描 (默认)
}
```

**核心逻辑**：
- `Level1`: 直接执行脚本，仅有资源限制（内存、超时）
- `Level2`: 使用系统级沙箱隔离（macOS 用 Seatbelt，Linux 用 Namespace）
- `Level3`: 在 Level2 基础上增加静态代码安全扫描，发现危险操作需用户授权

#### 1.2 资源限制 (`sandbox/executor.rs`)

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,   // 默认 512MB
    pub timeout_secs: u64,    // 默认 30 秒
}
```

**环境变量**：
- `SKILLBOX_MAX_MEMORY_MB`: 最大内存限制
- `SKILLBOX_TIMEOUT_SECS`: 执行超时
- `SKILLBOX_SANDBOX_LEVEL`: 沙箱级别 (1/2/3)
- `SKILLBOX_AUTO_APPROVE`: 自动批准危险操作

#### 1.3 macOS 沙箱实现 (`sandbox/macos.rs`)

**核心技术**: 使用 macOS 的 `sandbox-exec` 和 Seatbelt 配置文件

**关键函数**：
```rust
pub fn execute_with_limits(
    skill_dir: &Path,
    env_path: &Path,
    metadata: &SkillMetadata,
    input_json: &str,
    limits: ResourceLimits,
) -> Result<ExecutionResult>
```

**执行流程**：
1. 检查是否禁用沙箱 (`SKILLBOX_NO_SANDBOX`)
2. 启动网络代理（如果启用网络且有域名白名单）
3. 生成 Seatbelt 配置文件（限制文件系统、网络访问）
4. 使用 `sandbox-exec` 启动子进程
5. 监控内存使用和执行时间
6. 超限时终止进程

**Seatbelt 配置生成**：
```rust
fn execute_with_sandbox(...) -> Result<ExecutionResult> {
    // 检查是否有通配符 "*" (允许所有域名)
    let has_wildcard = metadata.network.outbound.iter()
        .any(|d| d.trim() == "*");
    
    // 启动网络代理（如果需要过滤）
    let proxy_manager = if metadata.network.enabled && !has_wildcard {
        let proxy_config = ProxyConfig::with_allowed_domains(
            metadata.network.outbound.clone()
        );
        ProxyManager::new(proxy_config)?.start()?
    };
    
    // 生成沙箱配置文件
    let profile_content = generate_sandbox_profile_with_proxy(
        skill_dir, env_path, metadata, work_dir,
        proxy_manager.http_port(),
        proxy_manager.socks5_port(),
    )?;
    
    // 使用 sandbox-exec 执行
    let mut cmd = Command::new("sandbox-exec");
    cmd.args(["-f", &profile_path]);
    cmd.arg(&executable);
    cmd.args(&args);
}
```

**内存监控**：
```rust
fn get_process_memory(pid: u32) -> Option<u64> {
    // 使用 ps 命令获取 RSS (常驻内存)
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output().ok()?;
    // RSS 单位是 KB，转换为 bytes
}
```

#### 1.4 静态代码扫描 (`sandbox/security/`)

安全扫描模块已重构为独立子模块，包含以下文件：

| 文件 | 职责 |
|------|------|
| `scanner.rs` | 扫描器主逻辑 |
| `rules.rs` | 安全规则定义和匹配 |
| `types.rs` | 安全类型定义 |
| `default_rules.rs` | 默认规则实现 |
| `default_rules.yaml` | 可配置的规则文件 |

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

**Python 危险模式检测**：
- `eval()`, `exec()`, `compile()` → Critical
- `subprocess`, `os.system` → High
- `pickle.loads`, `yaml.unsafe_load` → High
- `__import__`, `importlib.import_module` → Critical
- 大数组分配 (`[0] * 10^7`) → High

**规则配置** (`default_rules.yaml`)：
支持通过 YAML 文件自定义安全规则，无需重新编译

#### 1.5 SKILL.md 元数据解析 (`skill/metadata.rs`)

**支持两种格式**：

1. **传统格式** (YAML front matter):
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
| `compatibility` | 否 | 环境要求，最多 500 字符（用于推断网络权限、语言和 Python/Node 依赖） |
| `metadata` | 否 | 额外元数据（author、version 等） |
| `allowed-tools` | 否 | 预批准的工具列表（实验性） |

**从 `compatibility` 字段推断配置**：

1. **网络权限**：
   - 包含 "network"、"internet"、"http"、"api"、"web" → 启用网络访问

2. **语言检测**：
   - 包含 "Python" → language = python
   - 包含 "Node" 或 "JavaScript" → language = node
   - 包含 "bash" 或 "shell" → language = bash

3. **依赖管理**（自动从 compatibility 提取已知包名）：
   - Python 包：requests、pandas、numpy、scipy、matplotlib、flask、django、fastapi 等
   - Node 包：axios、express、lodash、moment、cheerio、puppeteer 等
   - 示例：`compatibility: Requires Python 3.x with requests, pandas` → 自动安装 requests 和 pandas

**自动检测入口点**：
```rust
fn detect_entry_point(skill_dir: &Path) -> Option<String> {
    // 优先级: main.py > main.js > main.ts > main.sh
    // 然后: index.* > run.* > entry.* > app.* > cli.*
    // 最后: 如果只有一个脚本文件，使用它
}
```

---

### 2. Python SDK (python-sdk)

#### 2.1 SkillManager (`core/manager.py`)

**设计模式**: Facade 模式，组合多个子组件

```python
class SkillManager:
    def __init__(self, skills_dir, ...):
        self._executor = SkillExecutor(...)      # 执行器
        self._registry = SkillRegistry()          # 注册表
        self._tool_builder = ToolBuilder(...)     # 工具定义生成
        self._prompt_builder = PromptBuilder(...) # 提示词生成
        self._handler = ToolCallHandler(...)      # 工具调用处理
```

**核心方法**：
- `scan_directory(directory)`: 扫描目录注册 Skills
- `get_tools()`: 获取 OpenAI 格式工具定义
- `handle_tool_calls(response)`: 处理 LLM 工具调用
- `get_system_prompt_context()`: 生成系统提示词

#### 2.2 SkillRegistry (`core/registry.py`)

**职责**: Skill 注册、发现和查找

```python
class SkillRegistry:
    def __init__(self):
        self._skills: Dict[str, SkillInfo] = {}
        self._multi_script_tools: Dict[str, Dict] = {}  # 多脚本工具
        self._analyzed_skills: Set[str] = set()
```

**多脚本工具支持**：
- 一个 Skill 可以有多个脚本入口
- 工具名格式: `skill-name:script-name`
- 例如: `skill-creator:init-skill`, `skill-creator:package-skill`

#### 2.3 ToolBuilder (`core/tool_builder.py`)

**职责**: 生成 LLM 工具定义

**渐进式披露策略**：
1. 工具定义只包含 name 和 description
2. 使用灵活的 schema 接受任意参数
3. 调用时注入完整 SKILL.md 内容

**Argparse 解析**：
```python
def _parse_argparse_schema(self, script_code: str) -> Dict:
    # 从 Python 脚本中提取 argparse 参数定义
    # 转换为 JSON Schema 格式
```

#### 2.4 ToolCallHandler (`core/handler.py`)

**职责**: 解析和执行 LLM 工具调用

```python
class ToolCallHandler:
    def execute(self, skill_name, input_data, ...):
        # 检查是否是多脚本工具
        tool_info = self._registry.get_multi_script_tool_info(skill_name)
        if tool_info:
            # 执行特定脚本
            return self._executor.execute(..., entry_point=tool_info["script_path"])
        # 常规 Skill 执行
        return self._executor.execute(...)
```

#### 2.5 AgenticLoop (`core/loops.py`)

**职责**: 处理 LLM-工具交互循环

**支持的 API 格式**：
```python
class ApiFormat(Enum):
    OPENAI = "openai"           # OpenAI 兼容格式
    CLAUDE_NATIVE = "claude_native"  # Claude 原生格式
```

**任务规划系统**：
```python
def _generate_task_list(self, user_message: str) -> List[Dict]:
    # 使用 LLM 分析用户需求
    # 判断是否需要工具
    # 生成任务列表
```

**核心原则**：
- 简单任务直接由 LLM 完成，不使用工具
- 只有真正需要外部能力时才规划工具调用

#### 2.6 Tools 协议适配 (`core/tools.py`)

**工具定义**：
```python
@dataclass
class ToolDefinition:
    name: str
    description: str
    input_schema: Dict[str, Any]
    
    def to_openai_format(self) -> Dict:
        return {"type": "function", "function": {...}}
    
    def to_claude_format(self) -> Dict:
        return {"name": ..., "description": ..., "input_schema": ...}
```

**工具调用请求**：
```python
@dataclass
class ToolUseRequest:
    id: str
    name: str
    input: Dict[str, Any]
    
    @classmethod
    def parse_from_openai_response(cls, response) -> List["ToolUseRequest"]:
        # 解析 OpenAI 格式响应
    
    @classmethod
    def parse_from_claude_response(cls, response) -> List["ToolUseRequest"]:
        # 解析 Claude 格式响应
```

#### 2.7 SkillRunner (`quick.py`)

**职责**: 一行代码运行 Skills

```python
class SkillRunner:
    def __init__(self, base_url=None, api_key=None, model=None, ...):
        # 自动加载 .env 配置
        # 懒加载 OpenAI client 和 SkillManager
    
    def run(self, user_message: str) -> str:
        # 创建 AgenticLoop
        # 执行并返回结果
```

**配置优先级**：
1. 构造函数参数
2. 环境变量
3. .env 文件
4. 默认值

---

## 🔄 执行流程

### 完整执行流程

```
用户输入
    ↓
SkillRunner.run()
    ↓
AgenticLoop.run()
    ↓
┌─────────────────────────────────────┐
│ 1. 生成系统提示词 (含 Skill 信息)    │
│ 2. 调用 LLM                         │
│ 3. 解析工具调用                      │
│ 4. 执行工具 (SkillExecutor)         │
│ 5. 返回结果给 LLM                   │
│ 6. 重复直到完成或达到最大迭代次数    │
└─────────────────────────────────────┘
    ↓
SkillExecutor.execute()
    ↓
调用 skilllite 二进制
    ↓
┌─────────────────────────────────────┐
│ Rust Sandbox:                       │
│ 1. 解析 SKILL.md 元数据             │
│ 2. 设置虚拟环境                      │
│ 3. Level 3: 静态代码扫描            │
│ 4. Level 2+: 启动沙箱               │
│ 5. 执行脚本                         │
│ 6. 监控资源使用                      │
│ 7. 返回结果                         │
└─────────────────────────────────────┘
    ↓
返回执行结果
```

### Skill 执行命令

```bash
# 运行 Skill
skilllite run <skill_dir> '<input_json>' [options]

# 直接执行脚本
skilllite exec <skill_dir> <script_path> '<input_json>' [options]

# 扫描 Skill
skilllite scan <skill_dir>

# 验证 Skill
skilllite validate <skill_dir>

# 安全扫描
skilllite security-scan <script_path> [options]
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

## 使用方法

输入城市名称，返回当前天气信息。

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
SKILLBOX_MAX_MEMORY_MB=512    # 内存限制
SKILLBOX_TIMEOUT_SECS=30      # 超时时间
SKILLBOX_AUTO_APPROVE=false   # 自动批准危险操作
SKILLBOX_NO_SANDBOX=false     # 禁用沙箱

# SDK 配置
ALLOW_NETWORK=false           # 允许网络访问
ENABLE_SANDBOX=true           # 启用沙箱
EXECUTION_TIMEOUT=120         # 执行超时
MAX_MEMORY_MB=512             # 最大内存
```

---

## 🛡️ 安全机制

### 1. 沙箱隔离

**macOS (Seatbelt)**:
- 文件系统隔离：只能访问 Skill 目录和临时目录
- 网络隔离：默认禁用，可按域名白名单开启
- 进程隔离：每个 Skill 独立进程

**Linux (Namespace + Seccomp)**:
- Mount namespace：隔离文件系统视图
- PID namespace：隔离进程空间
- Network namespace：隔离网络
- Seccomp BPF：限制系统调用（阻止 AF_UNIX socket 创建）
- 支持工具：bubblewrap (bwrap) 或 firejail

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

### 4. 强制拒绝路径 (`sandbox/security/rules.rs`)

**始终阻止写入的敏感文件**：

| 类别 | 文件示例 |
|------|----------|
| Shell 配置 | `.bashrc`, `.zshrc`, `.profile` |
| Git 配置 | `.gitconfig`, `.git/hooks/*` |
| IDE 配置 | `.vscode/settings.json`, `.idea/*` |
| 包管理器 | `.npmrc`, `.pypirc`, `.cargo/config` |
| 安全文件 | `.ssh/*`, `.gnupg/*`, `.aws/credentials` |
| AI/Agent 配置 | `.mcp.json`, `.claude/*`, `.cursor/*` |

**强制拒绝目录**：
```rust
pub const MANDATORY_DENY_DIRECTORIES: &[&str] = &[
    ".ssh", ".gnupg", ".aws", ".kube", ".docker",
    ".git/hooks", ".vscode", ".idea", ".claude", ".cursor",
];
```

### 5. 用户授权

Level 3 发现 Critical/High 级别问题时：
1. 显示安全警告
2. 列出发现的问题
3. 请求用户确认
4. 用户拒绝则阻止执行

---

## 📝 重构指南

### 如果需要重构 Rust 沙箱

1. **保持 CLI 接口兼容**：
   - `run`, `exec`, `scan`, `validate`, `info`, `security-scan` 命令
   - 参数格式保持一致

2. **保持输出格式**：
   - 成功时输出 JSON 到 stdout
   - 错误信息输出到 stderr

3. **安全级别逻辑**：
   - Level 1: 无沙箱
   - Level 2: 仅沙箱隔离
   - Level 3: 沙箱 + 代码扫描

### 如果需要重构 Python SDK

1. **保持 core 模块接口**：
   - `SkillManager` 是主入口
   - `get_tools()` 返回 OpenAI 格式
   - `handle_tool_calls()` 处理响应

2. **保持 SkillRunner 简洁**：
   - 一行代码运行
   - 自动加载配置

3. **保持工具协议适配**：
   - 支持 OpenAI 和 Claude 格式
   - 双向转换

---

## 🔗 依赖关系

### Rust 依赖 (Cargo.toml)

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }  # CLI 解析
serde = { version = "1", features = ["derive"] } # 序列化
serde_yaml = "0.9"                               # YAML 解析
serde_json = "1.0"                               # JSON 解析
tempfile = "3.10"                                # 临时文件
anyhow = "1.0"                                   # 错误处理
regex = "1.10"                                   # 正则表达式

# Linux 特定
[target.'cfg(target_os = "linux")'.dependencies]
nix = { version = "0.29", features = ["process", "mount", "sched", "signal"] }
libc = "0.2"

# macOS 特定
[target.'cfg(target_os = "macos")'.dependencies]
nix = { version = "0.29", features = ["process", "signal"] }
```

### Python 依赖

```
openai>=1.0.0      # LLM 客户端
pyyaml>=6.0        # YAML 解析
```

---

## 📌 注意事项

1. **不要修改 `.skills/` 目录**：这是示例 Skills，用户可能有自定义内容

2. **core 模块是受保护的**：修改前需要明确授权

3. **保持向后兼容**：API 变更需要考虑现有用户

4. **安全第一**：任何涉及沙箱的修改都需要仔细审查

5. **跨平台支持**：macOS 和 Linux 的沙箱实现不同，需要分别测试

---

## 🔌 内置工具

### 文件操作工具 (`builtin_tools.py`)

SDK 提供了四个内置文件操作工具，可在 AgenticLoop 中使用：

| 工具名 | 描述 | 参数 |
|--------|------|------|
| `read_file` | 读取文件内容 | `file_path`: 文件路径 |
| `write_file` | 写入/创建文件 | `file_path`: 文件路径, `content`: 内容 |
| `list_directory` | 列出目录内容 | `directory_path`: 目录路径, `recursive`: 是否递归 |
| `file_exists` | 检查文件是否存在 | `file_path`: 文件路径 |

**使用方式**：
```python
from skilllite import get_builtin_file_tools, execute_builtin_file_tool

# 获取工具定义
tools = get_builtin_file_tools()

# 执行工具
result = execute_builtin_file_tool("read_file", {"file_path": "test.txt"})
```

---

## 🔄 AgenticLoop 详解

### 核心执行流程

```python
class AgenticLoop:
    def run(self, user_message: str) -> Any:
        # 1. 任务规划（可选）
        if self.enable_task_planning:
            self.task_list = self._generate_task_list(user_message)
        
        # 2. 迭代执行
        for iteration in range(self.max_iterations):
            # 调用 LLM
            response = self.client.chat.completions.create(...)
            
            # 无工具调用则检查任务完成
            if not message.tool_calls:
                if self._check_all_tasks_completed():
                    return response
            
            # 渐进式披露：首次调用时注入 SKILL.md
            skill_docs = self._get_skill_docs_for_tools(message.tool_calls)
            if skill_docs:
                messages.append({"role": "system", "content": skill_docs})
                continue  # 让 LLM 重新调用
            
            # 执行工具
            tool_results = self.manager.handle_tool_calls(response)
            
            # 更新任务进度
            if self.enable_task_planning:
                self._update_task_list(completed_id)
```

### 任务规划系统

**核心原则**：最小化工具使用
- 简单任务（写作、翻译、问答）直接由 LLM 完成，返回空任务列表
- 只有真正需要外部能力时才规划工具调用

**任务列表格式**：
```json
[
  {"id": 1, "description": "任务描述", "tool_hint": "建议工具", "completed": false},
  {"id": 2, "description": "任务描述", "tool_hint": "file_operation", "completed": false}
]
```

### 渐进式披露

**策略**：工具定义只包含 name 和 description，完整 SKILL.md 在首次调用时注入

**实现**：
```python
def _get_skill_docs_for_tools(self, tool_calls):
    # 跟踪已文档化的 Skills，避免重复
    if not hasattr(self, '_documented_skills'):
        self._documented_skills = set()
    
    for tc in tool_calls:
        tool_name = tc.function.name
        if tool_name in self._documented_skills:
            continue
        
        skill_info = self.manager.get_skill(tool_name)
        if skill_info:
            full_content = skill_info.get_full_content()
            # 注入完整文档
            self._documented_skills.add(tool_name)
```

---

## 🐧 Linux 沙箱详解

### 沙箱工具优先级

```rust
fn execute_with_seccomp(...) -> Result<ExecutionResult> {
    // 1. 优先使用 bubblewrap (bwrap)
    if let Some(bwrap) = which_bwrap() {
        return execute_with_bwrap(...);
    }
    
    // 2. 回退到 firejail
    if let Some(firejail) = which_firejail() {
        return execute_with_firejail(...);
    }
    
    // 3. 无可用工具则报错
    anyhow::bail!("No sandbox tool available")
}
```

### Bubblewrap (bwrap) 配置

```rust
fn execute_with_bwrap(...) -> Result<ExecutionResult> {
    let mut cmd = Command::new(bwrap);
    
    // 基础隔离
    cmd.args(["--unshare-all"]);      // 取消共享所有命名空间
    cmd.args(["--die-with-parent"]);  // 父进程死亡时终止
    
    // 挂载最小文件系统
    cmd.args(["--ro-bind", "/usr", "/usr"]);
    cmd.args(["--ro-bind", "/lib", "/lib"]);
    cmd.args(["--ro-bind", "/bin", "/bin"]);
    
    // Skill 目录只读挂载
    cmd.args(["--ro-bind", &skill_dir, &skill_dir]);
    
    // 工作目录读写挂载
    cmd.args(["--bind", &work_dir, "/tmp"]);
    
    // 创建最小 /dev 和 /proc
    cmd.args(["--dev", "/dev"]);
    cmd.args(["--proc", "/proc"]);
    
    // 网络隔离
    if metadata.network.enabled {
        cmd.args(["--share-net"]);  // 共享网络（通过代理过滤）
    } else {
        cmd.args(["--unshare-net"]); // 完全隔离网络
    }
    
    // 敏感目录使用 tmpfs 隐藏
    for dir in MANDATORY_DENY_DIRECTORIES {
        cmd.args(["--tmpfs", &dir]);
    }
    
    // 应用 seccomp 过滤器
    cmd.args(["--seccomp", "3", &filter_path]);
}
```

### Seccomp BPF 过滤器

**目的**：阻止 Unix 域 socket 创建

```rust
// seccomp.rs
fn build_unix_socket_filter() -> Vec<SockFilter> {
    vec![
        // 加载系统调用号
        SockFilter::new(BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_NR),
        
        // 如果不是 socket()，允许
        SockFilter::new(BPF_JMP | BPF_JEQ | BPF_K, 0, 3, SYS_SOCKET),
        
        // 加载第一个参数 (domain/family)
        SockFilter::new(BPF_LD | BPF_W | BPF_ABS, 0, 0, SECCOMP_DATA_ARGS),
        
        // 如果是 AF_UNIX，返回 EPERM
        SockFilter::new(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, AF_UNIX),
        SockFilter::new(BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ERRNO | EPERM),
        
        // 允许其他所有
        SockFilter::new(BPF_RET | BPF_K, 0, 0, SECCOMP_RET_ALLOW),
    ]
}
```

**支持架构**：x86_64 和 aarch64

### 内存监控 (Linux)

```rust
fn wait_with_timeout_linux(child, timeout_secs, memory_limit_bytes) {
    loop {
        // 检查进程是否退出
        match child.try_wait() { ... }
        
        // 检查超时
        if start.elapsed() > timeout { ... }
        
        // 从 /proc/<pid>/status 读取内存使用
        if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", child.id())) {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    // 解析 RSS 值 (单位 KB)
                    let rss_bytes = rss_kb * 1024;
                    if rss_bytes > memory_limit_bytes {
                        child.kill();
                        return Err("memory_limit");
                    }
                }
            }
        }
        
        thread::sleep(check_interval);
    }
}
```

---

## 📊 数据流图

### 工具调用数据流

```
用户消息
    ↓
AgenticLoop._generate_task_list()
    ↓ (LLM 分析)
任务列表 [{id, description, tool_hint, completed}]
    ↓
AgenticLoop._run_openai() / _run_claude_native()
    ↓
┌─────────────────────────────────────────────────┐
│ 迭代循环                                         │
│                                                 │
│  LLM 响应                                       │
│      ↓                                          │
│  tool_calls?                                    │
│      ↓ Yes                                      │
│  _get_skill_docs_for_tools()                    │
│      ↓ (首次调用注入 SKILL.md)                   │
│  ToolCallHandler.handle_tool_calls()            │
│      ↓                                          │
│  SkillExecutor.execute()                        │
│      ↓                                          │
│  skilllite run/exec                              │
│      ↓                                          │
│  沙箱执行 → 结果                                 │
│      ↓                                          │
│  ToolResult → 添加到 messages                   │
│      ↓                                          │
│  _update_task_list()                            │
│      ↓                                          │
│  继续迭代或完成                                  │
└─────────────────────────────────────────────────┘
    ↓
最终响应
```

### Skill 元数据解析流程

```
SKILL.md 文件
    ↓
parse_skill_metadata()
    ↓
┌─────────────────────────────────────────────────┐
│ 1. 提取 YAML front matter (--- ... ---)         │
│ 2. 从 compatibility 字段解析网络权限和语言       │
│ 3. 自动检测 entry_point (从 scripts/ 目录)       │
│ 4. 自动检测 language (从入口点扩展名)            │
└─────────────────────────────────────────────────┘
    ↓
SkillMetadata {
    name, entry_point, language,
    description, compatibility, network
}
```

---

## 🧪 测试和验证

### 验证 Skill 结构

```bash
# 验证 Skill 元数据和入口点
skilllite validate ./.skills/calculator

# 查看 Skill 信息
skilllite info ./.skills/calculator

# 安全扫描脚本
skilllite security-scan ./.skills/calculator/scripts/main.py
```

### 测试执行

```bash
# 运行 Skill
skilllite run ./.skills/calculator '{"operation": "add", "a": 1, "b": 2}'

# 直接执行脚本
skilllite exec ./.skills/skill-creator scripts/init_skill.py '{"name": "test"}'
```

---

---

## 🆕 新增模块说明

### Rust sandbox/common.rs

跨平台通用功能模块，提取 macOS 和 Linux 共享的代码：

```rust
// 资源限制常量
pub const DEFAULT_MAX_MEMORY_MB: u64 = 512;
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_FILE_SIZE_LIMIT_MB: u64 = 10;
pub const DEFAULT_MAX_PROCESSES: u64 = 50;
pub const MEMORY_CHECK_INTERVAL_MS: u64 = 100;

// 跨平台内存监控
pub fn get_process_memory(pid: u32) -> Option<u64>;

// 统一的进程等待和资源监控
pub fn wait_with_timeout(
    child: &mut Child,
    timeout_secs: u64,
    memory_limit_bytes: u64,
) -> Result<(String, String, i32, bool, Option<String>)>;
```

### Rust sandbox/seatbelt.rs

安全策略和强制拒绝路径定义模块：

**强制拒绝的文件类别**：
- `MANDATORY_DENY_SHELL_CONFIGS`: Shell 配置文件 (`.bashrc`, `.zshrc` 等)
- `MANDATORY_DENY_GIT_CONFIGS`: Git 配置和钩子
- `MANDATORY_DENY_IDE_CONFIGS`: IDE 配置文件
- `MANDATORY_DENY_PACKAGE_CONFIGS`: 包管理器配置
- `MANDATORY_DENY_SECURITY_FILES`: 安全敏感文件 (SSH, AWS 等)
- `MANDATORY_DENY_AGENT_CONFIGS`: AI/Agent 配置文件

**核心函数**：
```rust
// 获取所有强制拒绝规则
pub fn get_mandatory_deny_rules() -> Vec<MandatoryDenyRule>;

// 生成 macOS Seatbelt 拒绝模式
pub fn generate_seatbelt_mandatory_deny_patterns() -> Vec<String>;
```

### Python SDK sandbox/config.py

沙箱配置管理模块，提供 `SandboxConfig` 数据类：

```python
@dataclass
class SandboxConfig:
    binary_path: Optional[str] = None      # skilllite 二进制路径
    cache_dir: Optional[str] = None        # 虚拟环境缓存目录
    allow_network: bool = False            # 允许网络访问
    enable_sandbox: bool = True            # 启用沙箱保护
    execution_timeout: int = 120           # 执行超时 (秒)
    max_memory_mb: int = 512               # 内存限制 (MB)
    sandbox_level: str = "3"               # 沙箱级别 (1/2/3)
    auto_install: bool = False             # 自动安装二进制
    auto_approve: bool = False             # 自动批准安全提示
```

**配置优先级**：构造函数参数 > 环境变量 > 默认值

**支持的环境变量**：
- `SKILLBOX_BINARY_PATH`, `SKILLBOX_CACHE_DIR`
- `SKILLBOX_SANDBOX_LEVEL`, `SKILLBOX_MAX_MEMORY_MB`, `SKILLBOX_TIMEOUT_SECS`
- `SKILLBOX_ALLOW_NETWORK`, `SKILLBOX_ENABLE_SANDBOX`, `SKILLBOX_AUTO_APPROVE`

### Python SDK sandbox/utils.py

CLI 参数转换工具：

```python
def convert_json_to_cli_args(
    input_data: Dict[str, Any],
    positional_keys: set = None
) -> List[str]:
    """
    将 JSON 输入转换为命令行参数列表
    
    示例:
        >>> convert_json_to_cli_args({"name": "test", "verbose": True, "count": 5})
        ['test', '--verbose', '--count', '5']
    """
```

**转换规则**：
- 位置参数：`skill_name`, `name`, `input` 等 → 直接作为值
- 命名参数：`path` → `--path value`
- 布尔标志：`True` → `--flag`，`False` → 省略
- 数组：`["a", "b"]` → `--key a,b`

---

*文档版本: 1.1.0*
*最后更新: 2026-01-31*
