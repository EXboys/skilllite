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
| `SKILLLITE_AUDIT_LOG` | （旧：`SKILLBOX_AUDIT_LOG`） | 审计日志路径 |
| `SKILLLITE_QUIET` | （旧：`SKILLBOX_QUIET`） | 静默模式 |
| `SKILLLITE_CACHE_DIR` | （旧：`SKILLBOX_CACHE_DIR`、`AGENTSKILL_CACHE_DIR`） | 技能环境缓存目录 |

**废弃说明**：`SKILLBOX_*`、`AGENTSKILL_*` 将在后续大版本中移除，请迁移至 `SKILLLITE_*` 对应变量。

---

## 配置来源优先级

同一变量若存在多处配置，按以下优先级解析（高 → 低）：

| 优先级 | 来源 | 说明 |
|--------|------|------|
| 1 | **CLI / 显式参数** | 命令行传入（如 `--message`）、quickstart 交互输入、桌面端设置覆盖 |
| 2 | **环境变量** | 进程启动前已设置的 `export VAR=value` |
| 3 | **.env 文件** | 工作区或当前目录下的 `.env`，`load_dotenv` 加载且**不覆盖**已存在的 env |
| 4 | **默认值** | 代码中的 fallback（如 `LlmConfig::from_env()` 的默认） |

**示例**：若 `.env` 中有 `MODEL=deepseek-chat`，但用户通过桌面端设置选择了 `gpt-4`，则最终使用 `gpt-4`（CLI/显式 > .env）。

### 界面语言（聊天 / 定时任务）

| 变量 | 取值 | 默认 | 说明 |
|------|------|------|------|
| `SKILLLITE_UI_LOCALE` | `zh`、`en` | 未设置则不注入 | 桌面应用会根据「设置 → 界面语言」写入子进程；`AgentConfig` 会将其合并进 system prompt 附加段，使模型默认用对应语言说明。**`skilllite schedule tick` 与交互式聊天**在设置此变量后行为一致。CLI 用户也可手动 `export SKILLLITE_UI_LOCALE=zh`。 |

---

## 按场景分层

| 层级 | 变量数量 | 说明 |
|------|----------|------|
| **必需** | 3 | `BASE_URL`、`API_KEY`、`MODEL`（或 `SKILLLITE_*` 等价） |
| **常用** | 5–8 | `SKILLS_DIR`、`ALLOW_NETWORK`、`EXECUTION_TIMEOUT`、`SKILLLITE_SANDBOX_LEVEL`、`ENABLE_SANDBOX` |
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
| `SKILLS_DIR` | string | `./skills` | Skills 目录路径，支持相对/绝对路径（兼容 `./.skills`） |
| `SKILLLITE_SKILLS_DIR` | string | - | 同上（别名） |
| `SKILLLITE_SKILLS_REPO` | string | `EXboys/skilllite` | `skilllite init` 在 `skills/` 为空时下载 skills 的 GitHub 仓库（如 `owner/repo`），可自定义 |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace}/output` | 输出目录（`write_output`、截图等）。未设置时 `workspace` 与 `SKILLLITE_WORKSPACE` 一致，缺省为**当前工作目录**（桌面聊天子进程的 cwd 为所选工程根） |
| （内部） | string | 当前工作目录 | 沙箱内 skill 路径根目录；旧变量 `SKILLBOX_SKILLS_ROOT`（暂无 SKILLLITE 命名） |

---

## 定时任务 `schedule tick` <small>[可选]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_SCHEDULE_ENABLED` | bool | **视为 `false`（未设置时）** | **`skilllite schedule tick` 在会调用 LLM 时必须为 `1`/`true`**；未设置则跳过执行并打印提示。**`--dry-run` 不需要此变量**。 |

**使用场景**：工作区存在 `.skilllite/schedule.json` 且由 cron 调用 `tick` 时，在 crontab 或 `.env` 中设置 `SKILLLITE_SCHEDULE_ENABLED=1`，避免误配 cron 即自动消耗 API。

---

## 网络配置 <small>[常用]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `ALLOW_NETWORK` | bool | `False` | 是否允许 Skill 访问网络 |
| （内部） | bool | - | 同上（沙箱内部；旧 `SKILLBOX_ALLOW_NETWORK`） |
| `NETWORK_TIMEOUT` | int | `30` | 网络请求超时（秒） |

**使用场景**：使用需要联网的 Skill（如天气、HTTP 请求）时，设置 `ALLOW_NETWORK=True`。

---

## 沙箱与安全 <small>[常用]</small>

沙箱相关变量**统一走 config 层**（`SandboxEnvConfig::from_env()`）；config 接受 `SKILLLITE_*`（推荐）与旧名 `SKILLBOX_*`。

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_SANDBOX_LEVEL` | int | `3` | **推荐**。沙箱级别（1/2/3） |
| `SKILLLITE_NO_SANDBOX` | bool | `false` | 禁用沙箱（不推荐） |
| `SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK` | bool | `false` | **仅 Linux**。bwrap/firejail 缺失或执行失败时，允许退回到仅 PID/UTS/网络命名空间的弱隔离（**无** bwrap 级文件系统沙箱）；默认 `false` 为拒绝执行，与 Windows fail-closed 对齐。兼容 `SKILLBOX_ALLOW_LINUX_NAMESPACE_FALLBACK` |
| `SKILLLITE_ALLOW_PLAYWRIGHT` | bool | `false` | 为使用 Playwright 的 Skill 放宽沙箱 |
| `ENABLE_SANDBOX` | bool | `true` | 是否启用沙箱 |
| `SKILLLITE_AUTO_APPROVE` | bool | `false` | **推荐**。自动批准 L3 安全提示（不推荐） |
| `SKILLLITE_SCRIPT_ARGS` | string | - | 透传给脚本的额外参数 |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | 在子进程中运行 read_file/write_file 以隔离 |
| `SKILLLITE_TRUST_BYPASS_CONFIRM` | bool | `false` | 允许 Community/Unknown 信任等级 Skill 无需确认即可执行（仅 CLI/Python；MCP 使用 `confirmed` 参数） |

**沙箱级别说明**：

| 级别 | 说明 |
|------|------|
| 1 | 无沙箱，完全信任 |
| 2 | 沙箱 + 允许 .env/git/venv/cache/Playwright（宽松） |
| 3 | 扫描 + 确认，确认后等同 L2（默认） |

**使用场景**：
- 沙箱不可用时：`SKILLLITE_SANDBOX_LEVEL=1` 或 `SKILLLITE_NO_SANDBOX=1`（完全无隔离）
- Linux 无 bubblewrap 且仍需**有限**隔离时：可设 `SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK=1`（弱隔离，慎用）
- Skill 卡住时：`SKILLLITE_LOG_LEVEL=debug` 查看进度

---

## 资源限制 <small>[高级]</small>

沙箱资源限制**统一走 config**（`SandboxEnvConfig`）；仍接受旧名 `SKILLBOX_*`。

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_TIMEOUT_SECS` | int | `30` | **推荐**。沙箱执行超时（秒） |
| `SKILLLITE_MAX_MEMORY_MB` | int | `256` | **推荐**。沙箱最大内存（MB） |
| `EXECUTION_TIMEOUT` | int | `120` | 单次执行超时（秒） |
| `MAX_MEMORY_MB` | int | `256` | 最大内存（MB） |

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
| `SKILLLITE_READ_FILE_TOOL_RESULT_MAX_CHARS` | int | `786432` | 仅 `read_file`：工具结果在传入模型前的最大字节数（默认约 768KiB，超出则 head+tail 截断） |

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

**常用变量**（大多数场景只需这些）：

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_EVOLUTION` | string | `1` | 进化模式：`1`/`true` 全部启用，`0`/`false` 禁用，`prompts`/`memory`/`skills` 仅启用对应维度 |
| `SKILLLITE_MAX_EVOLUTIONS_PER_DAY` | int | `20` | 每日进化次数上限 |
| `SKILLLITE_EVOLUTION_INTERVAL_SECS` | int | `600` | **A9** 周期性检查间隔（秒）。默认 10 分钟；每次 tick 在调度判定「到期」时才 spawn `evolution run` |
| `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` | int | `10` | **A9** OR 条件：原始未进化决策行数（`evolved = 0`，含零 tool 行）≥ 此值则到期 |
| `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN` | int | `3` | **A9** 滑动窗口内「有意义」未进化决策的加权和阈值；失败/负反馈计 2，其余计 1 |
| `SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW` | int | `10` | **A9** 参与加权和的最近多少条有意义未进化决策 |
| `SKILLLITE_EVO_SWEEP_INTERVAL_SECS` | int | `86400` | **A9** 若距上次 `evolution_run` 超过此秒数且加权和 ≥ 1，则到期（低优先级补跑） |
| `SKILLLITE_EVO_MIN_RUN_GAP_SEC` | int | `0` | **A9** 两次自动进化运行之间的最短间隔（秒）；`0` 表示不限制 |
| `SKILLLITE_EVO_SHALLOW_PREFLIGHT` | bool | `1` | **运行** 为 `1` 时，若加权/未处理积压为空且技能目录与外部学习无需工作，则跳过快照与各 learner（减轻周期空跑；可能推迟一轮仅依赖「零积压 tick」的 **规则 retire**）。`0` 关闭 |
| `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS` | int | `10` | 构建 **active** 进化提案前，至少需要多少条稳定成功且未进化的决策（与 A9 是否 spawn 分开） |
| `SKILLLITE_EVOLUTION_SNAPSHOT_KEEP` | int | `10` | 每次进化后备份目录 `chat/prompts/_versions/<txn>/` 最多保留几个（按目录名排序删最旧）。设为 **`0` 表示不删除**，可长期本地溯源 prompt 版本，无需 Git；磁盘占用会随进化次数增长 |
| `SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK` | bool | `1` | 在启用 policy runtime 时，允许 coordinator 自动执行低风险提案 |
| `SKILLLITE_EVO_POLICY_RUNTIME_ENABLED` | bool | `1` | 启用 coordinator 的 policy runtime，对提案给出 `allow` / `ask` / `deny` 及可审计原因链 |
| `SKILLLITE_EVO_DENY_CRITICAL` | bool | `1` | policy runtime 默认拒绝 critical 风险提案（backlog 状态为 `policy_denied`） |
| `SKILLLITE_EVO_RISK_BUDGET_LOW_PER_DAY` | int | `5` | 低风险提案每日自动执行预算（`0` 表示不自动执行） |
| `SKILLLITE_EVO_RISK_BUDGET_MEDIUM_PER_DAY` | int | `0` | 中风险提案每日自动执行预算（`0` 表示仅入人工队列） |
| `SKILLLITE_EVO_RISK_BUDGET_HIGH_PER_DAY` | int | `0` | 高风险提案每日自动执行预算（`0` 表示仅入人工队列） |
| `SKILLLITE_EVO_RISK_BUDGET_CRITICAL_PER_DAY` | int | `0` | 极高风险提案每日自动执行预算（`0` 表示由策略拒绝/排队） |
| `SKILLLITE_EVO_PROFILE` | string | （不设） | 进化触发场景：`demo` 更频繁（演示/内测）、`default` 或不设与原有默认一致、`conservative` 更少（生产/省成本）。**不设或 `default` 时行为与之前完全一致。** |
| `SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS` | int | `3` | backlog 验收自动联动的指标窗口天数 |
| `SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE` | float | `0.70` | 验收阈值：窗口内 `first_success_rate` 最低要求 |
| `SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE` | float | `0.20` | 验收阈值：窗口内 `user_correction_rate` 最高允许值 |
| `SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE` | float | `0.20` | 验收阈值：窗口内回滚率（`auto_rollback / evolution_run`）最高允许值 |
| `SKILLLITE_SKILL_DEDUP_DESCRIPTION` | string | `1` | Skill 同轮去重：`0` 关闭描述相似度检查；非 `0` 时，若新 skill 的 description 与已有 pending 高度相似则跳过 |

**高级变量**（按需细调阈值；未设时由 `SKILLLITE_EVO_PROFILE` 或默认值决定）：

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_EVO_COOLDOWN_HOURS` | float | `1` | 上次进化后冷却时间（小时），此时间内不再次触发 |
| `SKILLLITE_EVO_RECENT_DAYS` | int | `7` | 统计决策的时间窗口（天） |
| `SKILLLITE_EVO_RECENT_LIMIT` | int | `100` | 时间窗口内最多取多少条决策参与统计 |
| `SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS` | int | `2` | 单条决策至少多少 tool 调用才计入「有意义」条数 |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS` | int | `3` | 技能进化：有意义决策数 ≥ 此值且（有失败或存在重复模式）才触发 |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY` | int | `3` | 记忆进化：有意义决策数 ≥ 此值才触发 |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS` | int | `5` | 规则进化：有意义决策数 ≥ 此值且（失败/重规划达标）才触发 |
| `SKILLLITE_EVO_FAILURES_MIN_PROMPTS` | int | `2` | 规则进化：失败次数 ≥ 此值才考虑规则进化 |
| `SKILLLITE_EVO_REPLANS_MIN_PROMPTS` | int | `2` | 规则进化：重规划次数 ≥ 此值才考虑规则进化 |
| `SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT` | int | `3` | 重复模式判定：同一模式出现次数 ≥ 此值且成功率达标才计为重复模式 |
| `SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE` | float | `0.8` | 重复模式判定：成功率 ≥ 此值（0~1） |

**进化触发策略（A9）**：由 `growth_schedule` 判定「到期」，满足 **任一** 即可：**周期**（`SKILLLITE_EVOLUTION_INTERVAL_SECS`，默认 10 分钟）、**加权信号**（窗口内加权和 ≥ `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN`，默认 3）、**原始积压**（未处理行数 ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`，默认 10）、**清扫**（长期无 `evolution_run` 且加权和 ≥ 1）。可用 `SKILLLITE_EVO_MIN_RUN_GAP_SEC` 限制连续自动运行间隔。**`ChatSession`** 在进程内跑定时与回合后触发；**桌面助手**由 **Life Pulse** 合并工作区与界面环境后 spawn `skilllite evolution run`。对话内 **不再** 因 partial_success / failure 弹出「启动进化」气泡；调度与右侧「自进化」面板一致。

**SkillLite Assistant（桌面）**：自进化详情里队列行的 **立即执行** 与聊天共用 **设置** 中的 API Key、模型与接口地址（`api_base`）。**Life Pulse** 在启动 `skilllite evolution run` / `skilllite schedule tick` 子进程时，会把工作区 `.env` 与当前 **设置** 里同步到后端的 LLM 相关项合并后注入子进程环境（与对话子进程规则一致），**一般无需再单独写 `.env` 里的 Key**；若你希望完全脱离应用、仅用命令行在同一工作区跑进化，仍可依赖 `.env` 或系统环境变量。

在 **设置 → 自进化** 中可填写应用内覆盖（检查间隔、决策数阈值、`SKILLLITE_EVO_PROFILE` 预设、`SKILLLITE_EVO_COOLDOWN_HOURS`），写入方式与上述合并规则相同，**优先于工作区 `.env` 中的同名变量**；留空则仍读 `.env` / 内置默认（保存设置后生效）。

**同轮 Skill 去重**：单次进化会先执行失败驱动生成、再执行成功驱动生成，两者都可能向 `_pending` 写入新 skill。为避免重复，写入前会做：① 同名跳过（已有同名 pending 则不再写入）；② 描述相似跳过（description 归一化后互为子串则跳过，可通过 `SKILLLITE_SKILL_DEDUP_DESCRIPTION=0` 关闭）。

**Skill 生成失败**：若出现 `Failed to parse skill generation JSON: EOF`，多为 LLM 输出被截断。可增大 `SKILLLITE_MAX_TOKENS`（如 16384）后重试。

**需审核 Skill（L4 未通过）**：网络请求类 Skill 可能因 L4 安全扫描未通过而保存为 draft。`skilllite evolution status` 会显示 `(需审核)`。人工在 SKILL.md 的 front matter 中补充 `compatibility: Requires Python 3.x, network access` 后，执行 `skilllite evolution confirm <name>` 即可加入。

---

## 可观测性与审计 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_AUDIT_LOG` | string | `{data_root}/audit` | 审计目录或文件。目录则按天存储 `audit_YYYY-MM-DD.jsonl`；以 `.jsonl` 结尾则单文件 |
| `SKILLLITE_AUDIT_DISABLED` | bool | `false` | 设为 `1` 时关闭审计（默认开启） |
| `SKILLLITE_AUDIT_CONTEXT` | string | `cli` | 审计上下文（如 session_id、invoker）；写入 `skill_invocation` 与 **Agent 层 edit 事件**（`edit_applied` / `edit_previewed` / `edit_failed` / `edit_inserted`）的 `context` 字段 |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | 安全事件日志（拦截、scan_high 等） |
| `SKILLLITE_SUPPLY_CHAIN_BLOCK` | bool | `false` | P0 可观测 vs P1 可阻断：`1` 时 HashChanged/SignatureInvalid/TrustDeny 会阻断执行；`0`（默认）仅展示状态不阻断 |
| `SKILLLITE_LOG_LEVEL` | string | `info` | Rust 日志级别（**推荐**） |
| `SKILLLITE_LOG_JSON` | bool | `false` | 是否输出 JSON 格式日志 |
| `SKILLLITE_SKILL_DENYLIST` | string | - | **P1 手动禁用**：逗号分隔的 SKILL `name`（与审计 `skill_id` 一致），与下方 denylist 文件合并；命中则 `run` / `exec` / `bash` / Agent / MCP 执行前拒绝 |
| `SKILLLITE_AUDIT_ALERT_WEBHOOK` | string | - | `skilllite audit-report --alert` 命中规则时，除 stderr 与 tracing 外，可 POST JSON 告警到此 URL（也可用命令行 `--webhook`） |
| `SKILLLITE_AUDIT_ALERT_MAX_INVOCATIONS_PER_SKILL` | int | `200` | 告警：时间窗内单 Skill `skill_invocation` 次数超过此值 |
| `SKILLLITE_AUDIT_ALERT_MIN_INVOCATIONS_FOR_FAILURE` | int | `5` | 告警：至少多少次调用才参与「失败率」判定 |
| `SKILLLITE_AUDIT_ALERT_FAILURE_RATIO` | float | `0.5` | 告警：失败率 ≥ 此值（0–1）且调用次数 ≥ 上一项时触发 |
| `SKILLLITE_AUDIT_ALERT_EDIT_UNIQUE_PATHS` | int | `80` | 告警：时间窗内 `edit_*` 事件触及的不重复路径数超过此值 |

**P1 denylist 文件**（与 `SKILLLITE_SKILL_DENYLIST` 合并，每行一个 `name`，`#` 为注释）：`~/.skilllite/skill-denylist.txt`、`{data_root}/.skilllite/skill-denylist.txt`、当前工作目录下 `.skilllite/skill-denylist.txt`。**解禁**：从上述文件或环境变量中移除对应名称即可（每次执行前重新读取，无需重启进程）。

**P1 审计分析**：`skilllite audit-report [--dir DIR] [--hours N] [--json] [--alert] [--webhook URL]` — 汇总 `audit_*.jsonl` 在时间窗内的各 Skill 调用次数、失败率、`edit_*` 路径分布；`--alert` 在命中规则时输出到 stderr 与 `tracing`（target `skilllite::audit`），并可 POST 到 webhook。

**Edit 审计（Agent 内置工具）**：`search_replace`、`preview_edit`、`insert_lines` 会写入 JSONL，事件类型包括 `edit_applied`、`edit_previewed`、`edit_failed`、`edit_inserted`。每条记录含 `edit_id`（UUID）、顶层 `path`、`workspace`、失败时的 `reason`/`tool` 等；写入后 `flush`，便于流式消费。

**开发与测试**：`skilllite-agent` 的 builtin 单元测试在启动时自动设置 `SKILLLITE_AUDIT_DISABLED=1`，不向默认审计目录写入。其他 crate 或集成测试若需关闭审计，请显式设置 `SKILLLITE_AUDIT_DISABLED=1`。

---

## A11 高危工具确认 <small>[高级]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_HIGH_RISK_CONFIRM` | string | `write_key_path,run_command,network` | 逗号分隔：需发消息确认的高危操作。注：.env、.key、.git/config 等配置和密码文件读取已直接拒绝 |

---

## 调试与高级 <small>[高级/内部]</small>

| 变量 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `SKILLLITE_DEBUG` | bool | `false` | 设为 `1` 打印沙箱调试信息（旧：`SKILLBOX_DEBUG`） |
| `SKILLLITE_USE_IPC` | bool | 自动 | 是否使用 IPC 模式（通常更快）；旧：`SKILLBOX_USE_IPC` |
| `SKILLLITE_PATH` | string | - | skilllite 二进制路径 |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | 技能环境缓存目录（Python venv / Node），`skilllite env clean` 清理此目录 |
| `SKILLLITE_IPC_POOL_SIZE` | int | `10` | IPC 连接池大小（旧：`SKILLBOX_IPC_POOL_SIZE`） |
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
SKILLLITE_SANDBOX_LEVEL=1
# 或
SKILLLITE_LOG_LEVEL=debug
```

### 生产环境审计

```bash
# 默认已开启，审计按天存储于 ~/.skilllite/audit/audit_YYYY-MM-DD.jsonl
# 自定义目录（同样按天）：
SKILLLITE_AUDIT_LOG=/var/log/skilllite/audit
# 或单文件（不按天）：
SKILLLITE_AUDIT_LOG=/var/log/skilllite/audit.jsonl

SKILLLITE_SECURITY_EVENTS_LOG=~/.skilllite/audit/security.jsonl
```
