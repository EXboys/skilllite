# SkillLite 环境变量参考

本文档列出 SkillLite 支持的所有环境变量，包含默认值、类型说明及使用场景。

- **快速开始**：只需配置 `BASE_URL`、`API_KEY`、`MODEL` 即可运行
- **完整模板**：见 [.env.example.full](../../.env.example.full)

---

## LLM API 配置

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `BASE_URL` | string | - | **必需**。LLM API 地址，如 `https://api.deepseek.com/v1` |
| `API_KEY` | string | - | **必需**。LLM API 密钥 |
| `MODEL` | string | `deepseek-chat` | 模型名称 |
| `SKILLLITE_MAX_TOKENS` | int | `8192` | LLM 单次输出 token 上限；增大可减少 write_output 截断（部分 API 如 Claude 支持更高） |

**使用场景**：所有调用 LLM 的场景均需配置。支持 OpenAI 兼容 API 的任意提供商（DeepSeek、Qwen、Ollama 等）。若出现 `Recovered truncated JSON for write_output` 警告，可尝试增大 `SKILLLITE_MAX_TOKENS`。

---

## Skills 与输出

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLS_DIR` | string | `./.skills` | Skills 目录路径，支持相对/绝对路径 |
| `SKILLLITE_SKILLS_DIR` | string | - | 同上（别名） |
| `SKILLLITE_SKILLS_REPO` | string | `EXboys/skilllite` | `skilllite init` 在 `.skills/` 为空时下载 skills 的 GitHub 仓库（如 `owner/repo`），可自定义 |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace_root}/output` | 输出目录，用于报告、图片等 |
| `SKILLBOX_SKILLS_ROOT` | string | 当前工作目录 | 沙箱内 skill 路径的根目录，skill_dir 必须在其下 |

---

## 网络配置

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ALLOW_NETWORK` | bool | `False` | 是否允许 Skill 访问网络 |
| `SKILLBOX_ALLOW_NETWORK` | bool | - | 同上（沙箱内部使用） |
| `NETWORK_TIMEOUT` | int | `30` | 网络请求超时（秒） |

**使用场景**：使用需要联网的 Skill（如天气、HTTP 请求）时，设置 `ALLOW_NETWORK=True`。

---

## 沙箱与安全

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ENABLE_SANDBOX` | bool | `true` | 是否启用沙箱 |
| `SKILLBOX_SANDBOX_LEVEL` | int | `3` | 沙箱级别（见下表） |
| `SKILLBOX_ALLOW_PLAYWRIGHT` | bool | `false` | 为使用 Playwright 的 Skill 跳过沙箱 |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | 在子进程中运行 read_file/write_file 以隔离 |
| `SKILLBOX_AUTO_APPROVE` | bool | `false` | 自动批准 L3 安全提示（不推荐） |

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

## 资源限制

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `EXECUTION_TIMEOUT` | int | `120` | 单次执行超时（秒） |
| `SKILLBOX_TIMEOUT_SECS` | int | - | 同上（别名） |
| `MAX_MEMORY_MB` | int | `512` | 最大内存（MB） |
| `SKILLBOX_MAX_MEMORY_MB` | int | - | 同上（别名） |

**使用场景**：依赖较多的 Skill（如 xiaohongshu-writer）建议 `EXECUTION_TIMEOUT=300`。

---

## 长文本与摘要

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

## 规划与规则

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_PLANNING_RULES_PATH` | string | - | 自定义 planning_rules.json 路径 |

---

## 可观测性与审计

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_AUDIT_LOG` | string | - | 审计日志路径（确认→执行→命令） |
| `SKILLBOX_AUDIT_LOG` | string | - | 同上（别名） |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | 安全事件日志（拦截、scan_high 等） |
| `SKILLBOX_LOG_LEVEL` | string | `info` | Rust 日志级别：trace\|debug\|info\|warn\|error |
| `SKILLBOX_LOG_JSON` | bool | `false` | 是否输出 JSON 格式日志 |

---

## 调试与高级

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLBOX_DEBUG` | bool | `false` | 设为 `1` 打印沙箱调试信息 |
| `SKILLBOX_USE_IPC` | bool | 自动 | 是否使用 IPC 模式（通常更快） |
| `SKILLLITE_PATH` | string | - | skilllite 二进制路径 |
| `SKILLBOX_BINARY_PATH` | string | - | 同上（别名） |
| `SKILLBOX_CACHE_DIR` | string | - | 沙箱缓存目录 |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | 技能环境缓存目录（Python venv / Node），`skilllite env clean` 清理此目录；兼容 `AGENTSKILL_CACHE_DIR` |
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
