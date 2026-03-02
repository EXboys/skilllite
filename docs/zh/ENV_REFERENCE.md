# SkillLite 环境变量参考

本文档列出 SkillLite 支持的所有环境变量，包含默认值、类型说明及使用场景。

- **快速开始**：只需配置 `BASE_URL`、`API_KEY`、`MODEL` 即可运行
- **完整模板**：见 [.env.example.full](../../.env.example.full)

---

## 推荐变量与兼容别名

优先使用 `SKILLLITE_*` 作为主变量；兼容 `OPENAI_*`、`BASE_URL` 等业界通用名；`SKILLBOX_*`、`AGENTSKILL_*` 已废弃，建议迁移。

| 推荐变量 | 兼容别名（fallback 顺序） | 说明 |
|----------|---------------------------|------|
| `SKILLLITE_API_BASE` | `OPENAI_API_BASE`, `OPENAI_BASE_URL`, `BASE_URL` | LLM API 地址 |
| `SKILLLITE_API_KEY` | `OPENAI_API_KEY`, `API_KEY` | API 密钥 |
| `SKILLLITE_MODEL` | `OPENAI_MODEL`, `MODEL` | 模型名称 |
| `SKILLLITE_AUDIT_LOG` | `SKILLBOX_AUDIT_LOG` | 审计日志路径 |
| `SKILLLITE_QUIET` | `SKILLBOX_QUIET` | 静默模式 |
| `SKILLLITE_CACHE_DIR` | `SKILLBOX_CACHE_DIR`, `AGENTSKILL_CACHE_DIR` | 技能环境缓存目录 |

**废弃说明**：`SKILLBOX_*`、`AGENTSKILL_*` 将在后续大版本中移除，请迁移至 `SKILLLITE_*` 对应变量。

---

## 按场景分层

| 层级 | 变量数量 | 说明 |
|------|----------|------|
| **必需** | 3 | `BASE_URL`、`API_KEY`、`MODEL`（或 `SKILLLITE_*` 等价） |
| **常用** | 5–8 | `SKILLS_DIR`、`ALLOW_NETWORK`、`EXECUTION_TIMEOUT`、`SKILLBOX_SANDBOX_LEVEL`、`ENABLE_SANDBOX` |
| **高级** | 15–20 | 长文本、规划、审计、资源限制等，按需配置 |
| **内部** | 其余 | 子进程/沙箱内部使用，一般不需用户配置 |

- **`.env.example`**：仅包含「必需 + 常用」
- **`.env.example.full`**：完整变量列表 + 层级注释

---

## LLM API 配置 <small>[必需]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_API_BASE` | string | - | **推荐**。LLM API 地址；兼容 `OPENAI_API_BASE`、`OPENAI_BASE_URL`、`BASE_URL` |
| `SKILLLITE_API_KEY` | string | - | **推荐**。API 密钥；兼容 `OPENAI_API_KEY`、`API_KEY` |
| `SKILLLITE_MODEL` | string | `deepseek-chat` | **推荐**。模型名称；兼容 `OPENAI_MODEL`、`MODEL` |
| `SKILLLITE_MAX_TOKENS` | int | `8192` | LLM 单次输出 token 上限；增大可减少 write_output 截断（部分 API 如 Claude 支持更高） |

**使用场景**：所有调用 LLM 的场景均需配置。支持 OpenAI 兼容 API 的任意提供商（DeepSeek、Qwen、Ollama 等）。若出现 `Recovered truncated JSON for write_output` 警告，可尝试增大 `SKILLLITE_MAX_TOKENS`。

---

## Skills 与输出 <small>[常用]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLS_DIR` | string | `./.skills` | Skills 目录路径，支持相对/绝对路径 |
| `SKILLLITE_SKILLS_DIR` | string | - | 同上（别名） |
| `SKILLLITE_SKILLS_REPO` | string | `EXboys/skilllite` | `skilllite init` 在 `.skills/` 为空时下载 skills 的 GitHub 仓库（如 `owner/repo`），可自定义 |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace_root}/output` | 输出目录，用于报告、图片等 |
| `SKILLBOX_SKILLS_ROOT` | string | 当前工作目录 | 沙箱内 skill 路径的根目录（**废弃**，暂无 SKILLLITE 替代） |

---

## 网络配置 <small>[常用]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ALLOW_NETWORK` | bool | `False` | 是否允许 Skill 访问网络 |
| `SKILLBOX_ALLOW_NETWORK` | bool | - | 同上（沙箱内部使用，**废弃**） |
| `NETWORK_TIMEOUT` | int | `30` | 网络请求超时（秒） |

**使用场景**：使用需要联网的 Skill（如天气、HTTP 请求）时，设置 `ALLOW_NETWORK=True`。

---

## 沙箱与安全 <small>[常用]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ENABLE_SANDBOX` | bool | `true` | 是否启用沙箱 |
| `SKILLBOX_SANDBOX_LEVEL` | int | `3` | 沙箱级别（见下表） |
| `SKILLBOX_ALLOW_PLAYWRIGHT` | bool | `false` | 为使用 Playwright 的 Skill 跳过沙箱 |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | 在子进程中运行 read_file/write_file 以隔离 |
| `SKILLBOX_AUTO_APPROVE` | bool | `false` | 自动批准 L3 安全提示（不推荐） |
| `SKILLLITE_TRUST_BYPASS_CONFIRM` | bool | `false` | 允许 Community/Unknown 信任等级 Skill 无需确认即可执行（仅 CLI/Python；MCP 使用 `confirmed` 参数） |

**沙箱级别说明**：

| 级别 | 说明 |
|------|------|
| 1 | 无沙箱，完全信任 |
| 2 | 沙箱 + 允许 .env/git/venv/cache/Playwright（宽松） |
| 3 | 扫描 + 确认，确认后等同 L2（默认） |

**使用场景**：
- 沙箱不可用时：`SKILLBOX_SANDBOX_LEVEL=1`
- Skill 卡住时：`SKILLBOX_DEBUG=1` 查看进度

---

## 资源限制 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `EXECUTION_TIMEOUT` | int | `120` | 单次执行超时（秒） |
| `SKILLBOX_TIMEOUT_SECS` | int | - | 同上（别名） |
| `MAX_MEMORY_MB` | int | `256` | 最大内存（MB） |
| `SKILLBOX_MAX_MEMORY_MB` | int | - | 同上（别名） |

**使用场景**：依赖较多的 Skill（如 xiaohongshu-writer）建议 `EXECUTION_TIMEOUT=300`。

---

## 长文本与摘要 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_CHUNK_SIZE` | int | `6000` | 分块大小（约 1.5k tokens/chunk） |
| `SKILLLITE_HEAD_CHUNKS` | int | `3` | 头尾摘要的头块数 |
| `SKILLLITE_TAIL_CHUNKS` | int | `3` | 头尾摘要的尾块数 |
| `SKILLLITE_MAX_OUTPUT_CHARS` | int | `8000` | 摘要最大输出长度（约 2k tokens） |
| `SKILLLITE_SUMMARIZE_THRESHOLD` | int | `15000` | 超过此长度用摘要，否则截断 |
| `SKILLLITE_TOOL_RESULT_MAX_CHARS` | int | `8000` | Agent 循环中单次工具结果最大字符数 |

**使用场景**：处理超长上下文时按需调整，一般无需修改。

---

## 会话与压缩 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_COMPACTION_THRESHOLD` | int | `16` | 对话历史超过此消息数时触发压缩（约 8 轮） |
| `SKILLLITE_COMPACTION_KEEP_RECENT` | int | `10` | 压缩后保留的最近消息数 |
| `SKILLLITE_MEMORY_FLUSH_ENABLED` | bool | `true` | 是否启用 pre-compaction 记忆自动写入（OpenClaw 风格） |
| `SKILLLITE_MEMORY_FLUSH_THRESHOLD` | int | `12` | 达到此消息数时触发记忆 flush（低于压缩阈值可更早触发） |

**使用场景**：若希望更早触发压缩，可降低 `COMPACTION_THRESHOLD`（如 `12`）；若压缩过于频繁可适当提高。`/compact` 命令可手动触发压缩，不受阈值限制。

**记忆自动触发**：启用 `enable_memory` 时，当对话达到 `MEMORY_FLUSH_THRESHOLD`（默认 12 条消息，约 6 轮）会自动运行一次静默 turn，提醒模型将重要内容写入 `memory/YYYY-MM-DD.md`。若记忆触发过少，可降低 `MEMORY_FLUSH_THRESHOLD`（如 `8` 或 `6`）。

---

## 规划与规则 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_COMPACT_PLANNING` | bool | 自动 | 1=紧凑，0=完整。未设置时：仅 claude/gpt-4/gpt-5/gemini-2 用紧凑；deepseek/qwen/7b/ollama 等用完整 |

规划规则定义在 `planning_rules.rs` 中，无需外部 JSON 配置。

---

## 进化引擎 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_EVOLUTION` | string | `1` | 进化模式：`1`/`true` 全部启用，`0`/`false` 禁用，`prompts`/`memory`/`skills` 仅启用对应维度 |
| `SKILLLITE_MAX_EVOLUTIONS_PER_DAY` | int | `20` | 每日进化次数上限 |
| `SKILLLITE_EVOLUTION_INTERVAL_SECS` | int | `1800` | **A9** 周期性触发间隔（秒）。每 30 分钟触发一次进化，即使用户活跃也会在后台进化 |
| `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` | int | `10` | **A9** 决策数触发阈值。当未处理决策数 ≥ 此值时触发进化 |

**进化触发策略（A9）**：周期性触发（每 30 分钟）+ 决策数触发（每 N 条 decisions），即使用户持续交互也能在后台进化。

**Skill 生成失败**：若出现 `Failed to parse skill generation JSON: EOF`，多为 LLM 输出被截断。可增大 `SKILLLITE_MAX_TOKENS`（如 16384）后重试。

**需审核 Skill（L4 未通过）**：网络请求类 Skill 可能因 L4 安全扫描未通过而保存为 draft。`skilllite evolution status` 会显示 `(需审核)`。人工在 SKILL.md 的 front matter 中补充 `compatibility: Requires Python 3.x, network access` 后，执行 `skilllite evolution confirm <name>` 即可加入。

---

## 可观测性与审计 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_AUDIT_LOG` | string | - | 审计日志路径（确认→执行→命令） |
| `SKILLBOX_AUDIT_LOG` | string | - | 同上（**废弃**，请改用 `SKILLLITE_AUDIT_LOG`） |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | 安全事件日志（拦截、scan_high 等） |
| `SKILLLITE_LOG_LEVEL` | string | `info` | Rust 日志级别（**推荐**）；兼容 `SKILLBOX_LOG_LEVEL`（**废弃**） |
| `SKILLLITE_LOG_JSON` | bool | `false` | 是否输出 JSON 格式日志；兼容 `SKILLBOX_LOG_JSON`（**废弃**） |

---

## A11 高危工具确认 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_HIGH_RISK_CONFIRM` | string | `write_key_path,run_command,network` | 逗号分隔：需发消息确认的高危操作。注：.env、.key、.git/config 等配置和密码文件读取已直接拒绝 |

---

## 调试与高级 <small>[高级/内部]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLBOX_DEBUG` | bool | `false` | 设为 `1` 打印沙箱调试信息 |
| `SKILLBOX_USE_IPC` | bool | 自动 | 是否使用 IPC 模式（通常更快） |
| `SKILLLITE_PATH` | string | - | skilllite 二进制路径 |
| `SKILLBOX_BINARY_PATH` | string | - | 同上（**废弃**，请改用 `SKILLLITE_PATH`） |
| `SKILLBOX_CACHE_DIR` | string | - | 沙箱缓存目录（**废弃**，请改用 `SKILLLITE_CACHE_DIR`） |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | 技能环境缓存目录（Python venv / Node），`skilllite env clean` 清理此目录；兼容 `SKILLBOX_CACHE_DIR`、`AGENTSKILL_CACHE_DIR`（**废弃**） |
| `SKILLBOX_IPC_POOL_SIZE` | int | `10` | IPC 连接池大小 |
| `MCP_SANDBOX_TIMEOUT` | int | `30` | MCP 沙箱超时（秒） |

---

## 按场景推荐配置

### 快速体验（最小配置）

```bash
BASE_URL=https://api.deepseek.com/v1
API_KEY=your_key
MODEL=deepseek-chat
```

### 使用联网 Skill（如天气、HTTP）

```bash
# 在最小配置基础上增加
ALLOW_NETWORK=True
```

### 依赖较多的 Skill（如 xiaohongshu-writer）

```bash
# 在常用配置基础上增加
EXECUTION_TIMEOUT=300
```

### 沙箱不可用 / 调试

```bash
SKILLBOX_SANDBOX_LEVEL=1
# 或
SKILLBOX_DEBUG=1
```

### 生产环境审计

```bash
SKILLLITE_AUDIT_LOG=~/.skilllite/audit/audit.jsonl
SKILLLITE_SECURITY_EVENTS_LOG=~/.skilllite/audit/security.jsonl
```
