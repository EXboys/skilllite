//! 环境变量 key 常量与别名定义
//!
//! 主变量优先使用 `SKILLLITE_*`，兼容 `OPENAI_*`、`SKILLBOX_*` 等。

// ─── Legacy flat constants (backward compat: env/builder, etc.) ──────────────
pub const SKILLLITE_CACHE_DIR: &str = "SKILLLITE_CACHE_DIR";
pub const SKILLLITE_OUTPUT_DIR: &str = "SKILLLITE_OUTPUT_DIR";
pub const SKILLLITE_SKILLS_DIR: &str = "SKILLLITE_SKILLS_DIR";
pub const SKILLLITE_MODEL: &str = "SKILLLITE_MODEL";
pub const SKILLLITE_QUIET: &str = "SKILLLITE_QUIET";
pub const SKILLLITE_LOG_LEVEL: &str = "SKILLLITE_LOG_LEVEL";
pub const SKILLLITE_ENABLE_TASK_PLANNING: &str = "SKILLLITE_ENABLE_TASK_PLANNING";
pub const SKILLBOX_SKILLS_ROOT: &str = "SKILLBOX_SKILLS_ROOT";
pub const SKILLBOX_CACHE_DIR: &str = "SKILLBOX_CACHE_DIR";
pub const AGENTSKILL_CACHE_DIR: &str = "AGENTSKILL_CACHE_DIR";

/// LLM API 配置
pub mod llm {
    /// API Base — 主变量优先
    pub const API_BASE: &str = "SKILLLITE_API_BASE";
    pub const API_BASE_ALIASES: &[&str] = &["OPENAI_API_BASE", "OPENAI_BASE_URL", "BASE_URL"];

    /// API Key
    pub const API_KEY: &str = "SKILLLITE_API_KEY";
    pub const API_KEY_ALIASES: &[&str] = &["OPENAI_API_KEY", "API_KEY"];

    /// Model
    pub const MODEL: &str = "SKILLLITE_MODEL";
    pub const MODEL_ALIASES: &[&str] = &["OPENAI_MODEL", "MODEL"];
}

/// Skills、输出、工作区
pub mod paths {
    pub const SKILLLITE_SKILLS_DIR: &str = "SKILLLITE_SKILLS_DIR";
    pub const SKILLS_DIR_ALIASES: &[&str] = &["SKILLS_DIR"];

    pub const SKILLLITE_OUTPUT_DIR: &str = "SKILLLITE_OUTPUT_DIR";

    pub const SKILLLITE_WORKSPACE: &str = "SKILLLITE_WORKSPACE";

    pub const SKILLLITE_SKILLS_REPO: &str = "SKILLLITE_SKILLS_REPO";

    pub const SKILLBOX_SKILLS_ROOT: &str = "SKILLBOX_SKILLS_ROOT";
}

/// `skilllite schedule tick`：非 dry-run 时是否允许调用 LLM（默认视为关闭，需显式开启）
pub mod schedule {
    pub const SKILLLITE_SCHEDULE_ENABLED: &str = "SKILLLITE_SCHEDULE_ENABLED";
}

/// 缓存目录
pub mod cache {
    pub const SKILLLITE_CACHE_DIR: &str = "SKILLLITE_CACHE_DIR";
    pub const CACHE_DIR_ALIASES: &[&str] = &["SKILLBOX_CACHE_DIR", "AGENTSKILL_CACHE_DIR"];
}

/// 可观测性与日志
pub mod observability {
    pub const SKILLLITE_QUIET: &str = "SKILLLITE_QUIET";
    pub const QUIET_ALIASES: &[&str] = &["SKILLBOX_QUIET"];

    pub const SKILLLITE_LOG_LEVEL: &str = "SKILLLITE_LOG_LEVEL";
    pub const LOG_LEVEL_ALIASES: &[&str] = &["SKILLBOX_LOG_LEVEL"];

    pub const SKILLLITE_LOG_JSON: &str = "SKILLLITE_LOG_JSON";
    pub const LOG_JSON_ALIASES: &[&str] = &["SKILLBOX_LOG_JSON"];

    pub const SKILLLITE_AUDIT_LOG: &str = "SKILLLITE_AUDIT_LOG";
    pub const AUDIT_LOG_ALIASES: &[&str] = &["SKILLBOX_AUDIT_LOG"];

    /// 设为 1/true 时显式关闭审计（默认开启）
    pub const SKILLLITE_AUDIT_DISABLED: &str = "SKILLLITE_AUDIT_DISABLED";

    pub const SKILLLITE_SECURITY_EVENTS_LOG: &str = "SKILLLITE_SECURITY_EVENTS_LOG";

    /// P0 可观测 vs P1 可阻断：设为 1/true 时，HashChanged/SignatureInvalid/TrustDeny 会阻断执行；不设或 0 时仅展示状态不阻断（P0 模式）
    pub const SKILLLITE_SUPPLY_CHAIN_BLOCK: &str = "SKILLLITE_SUPPLY_CHAIN_BLOCK";

    /// 审计上下文：谁调用了 Skill（如 session_id、invoker），用于 audit 日志
    pub const SKILLLITE_AUDIT_CONTEXT: &str = "SKILLLITE_AUDIT_CONTEXT";

    /// 逗号分隔的 SKILL 名称列表，与 `skill-denylist.txt` 合并；命中则执行前拒绝（P1 手动禁用）
    pub const SKILLLITE_SKILL_DENYLIST: &str = "SKILLLITE_SKILL_DENYLIST";

    /// `skilllite audit-report --alert` 或需 webhook 时的告警 URL
    pub const SKILLLITE_AUDIT_ALERT_WEBHOOK: &str = "SKILLLITE_AUDIT_ALERT_WEBHOOK";

    /// 时间窗内单 Skill 调用次数超过此值则预警（默认 200）
    pub const SKILLLITE_AUDIT_ALERT_MAX_INVOCATIONS_PER_SKILL: &str =
        "SKILLLITE_AUDIT_ALERT_MAX_INVOCATIONS_PER_SKILL";

    /// 参与「失败率」判定的最少调用次数（默认 5）
    pub const SKILLLITE_AUDIT_ALERT_MIN_INVOCATIONS_FOR_FAILURE: &str =
        "SKILLLITE_AUDIT_ALERT_MIN_INVOCATIONS_FOR_FAILURE";

    /// 失败率不低于此值则预警，0.0–1.0（默认 0.5）
    pub const SKILLLITE_AUDIT_ALERT_FAILURE_RATIO: &str = "SKILLLITE_AUDIT_ALERT_FAILURE_RATIO";

    /// 时间窗内 edit 事件触及的不重复路径数超过此值则预警（默认 80）
    pub const SKILLLITE_AUDIT_ALERT_EDIT_UNIQUE_PATHS: &str =
        "SKILLLITE_AUDIT_ALERT_EDIT_UNIQUE_PATHS";
}

/// Agent 行为提示（桌面 / CLI 可选）
pub mod agent {
    /// 桌面或包装器传入的界面语言：`zh` | `en`。合并进聊天类 system prompt 的附加段（与 `AgentConfig.context_append` 同源逻辑）。
    pub const SKILLLITE_UI_LOCALE: &str = "SKILLLITE_UI_LOCALE";
}

/// Memory 向量检索
pub mod memory {
    pub const SKILLLITE_EMBEDDING_MODEL: &str = "SKILLLITE_EMBEDDING_MODEL";
    pub const SKILLLITE_EMBEDDING_DIMENSION: &str = "SKILLLITE_EMBEDDING_DIMENSION";
}

/// 进化引擎
pub mod evolution {
    /// Evolution mode: "1" (default, all), "prompts", "memory", "skills", "0" (disabled).
    pub const SKILLLITE_EVOLUTION: &str = "SKILLLITE_EVOLUTION";
    pub const SKILLLITE_MAX_EVOLUTIONS_PER_DAY: &str = "SKILLLITE_MAX_EVOLUTIONS_PER_DAY";
    /// A9: Periodic evolution interval (seconds). Default 600 (10 min). Used by `ChatSession` and desktop Life Pulse.
    pub const SKILLLITE_EVOLUTION_INTERVAL_SECS: &str = "SKILLLITE_EVOLUTION_INTERVAL_SECS";
    /// A9: OR-trigger — raw unprocessed decision rows (`evolved = 0`, default 10). Used with weighted signal arm.
    pub const SKILLLITE_EVOLUTION_DECISION_THRESHOLD: &str =
        "SKILLLITE_EVOLUTION_DECISION_THRESHOLD";
    /// A9: Weighted sum of recent meaningful unprocessed decisions must reach this (default 3). See `growth_schedule`.
    pub const SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN: &str = "SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN";
    /// A9: Sliding window size for weighted trigger (default 10).
    pub const SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW: &str = "SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW";
    /// A9: If no `evolution_run` for this many seconds and weighted sum ≥ 1, allow sweep trigger (default 86400).
    pub const SKILLLITE_EVO_SWEEP_INTERVAL_SECS: &str = "SKILLLITE_EVO_SWEEP_INTERVAL_SECS";
    /// A9: Minimum seconds since last `evolution_run` before another autorun (0 = disabled).
    pub const SKILLLITE_EVO_MIN_RUN_GAP_SEC: &str = "SKILLLITE_EVO_MIN_RUN_GAP_SEC";
    /// Skip snapshot + learners when there is no decision backlog (default on). Set `0` to disable.
    pub const SKILLLITE_EVO_SHALLOW_PREFLIGHT: &str = "SKILLLITE_EVO_SHALLOW_PREFLIGHT";
    /// Active-scope proposals: minimum stable successful decisions before active evolution (default 10).
    pub const SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS: &str =
        "SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS";
    /// Prompt snapshot dirs under `chat/prompts/_versions/` to keep after each evolution (oldest pruned first).
    /// Default `10`. Set to `0` to never delete snapshots (full local history, no Git required; disk usage grows).
    pub const SKILLLITE_EVOLUTION_SNAPSHOT_KEEP: &str = "SKILLLITE_EVOLUTION_SNAPSHOT_KEEP";
    /// Allow coordinator to auto-execute low-risk proposals when policy runtime is enabled.
    /// Default enabled (`1`/`true`).
    pub const SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK: &str = "SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK";
    /// Enable policy runtime in coordinator (`allow`/`ask`/`deny` with reason chain).
    /// Default enabled (`1`/`true`).
    pub const SKILLLITE_EVO_POLICY_RUNTIME_ENABLED: &str = "SKILLLITE_EVO_POLICY_RUNTIME_ENABLED";
    /// Deny critical-risk proposals in policy runtime by default.
    /// Default enabled (`1`/`true`).
    pub const SKILLLITE_EVO_DENY_CRITICAL: &str = "SKILLLITE_EVO_DENY_CRITICAL";
    /// Daily auto-execution budget for low-risk proposals (coordinator policy runtime).
    /// Default `5`.
    pub const SKILLLITE_EVO_RISK_BUDGET_LOW_PER_DAY: &str = "SKILLLITE_EVO_RISK_BUDGET_LOW_PER_DAY";
    /// Daily auto-execution budget for medium-risk proposals (default `0` = manual only).
    pub const SKILLLITE_EVO_RISK_BUDGET_MEDIUM_PER_DAY: &str =
        "SKILLLITE_EVO_RISK_BUDGET_MEDIUM_PER_DAY";
    /// Daily auto-execution budget for high-risk proposals (default `0` = manual only).
    pub const SKILLLITE_EVO_RISK_BUDGET_HIGH_PER_DAY: &str =
        "SKILLLITE_EVO_RISK_BUDGET_HIGH_PER_DAY";
    /// Daily auto-execution budget for critical-risk proposals (default `0`).
    pub const SKILLLITE_EVO_RISK_BUDGET_CRITICAL_PER_DAY: &str =
        "SKILLLITE_EVO_RISK_BUDGET_CRITICAL_PER_DAY";
    /// 进化触发场景：demo（更频繁）/ default（与不设一致）/ conservative（更少、省成本）。不设或 default 时行为与原有默认完全一致。
    pub const SKILLLITE_EVO_PROFILE: &str = "SKILLLITE_EVO_PROFILE";

    // ── 5.2 进化触发条件（高级，可单独覆盖；未设时由 EVO_PROFILE 或默认值决定）────────────
    /// 上次进化后冷却时间（小时），此时间内不再次触发。默认 1。
    pub const SKILLLITE_EVO_COOLDOWN_HOURS: &str = "SKILLLITE_EVO_COOLDOWN_HOURS";
    /// 统计决策的时间窗口（天）。默认 7。
    pub const SKILLLITE_EVO_RECENT_DAYS: &str = "SKILLLITE_EVO_RECENT_DAYS";
    /// 时间窗口内最多取多少条决策参与统计。默认 100。
    pub const SKILLLITE_EVO_RECENT_LIMIT: &str = "SKILLLITE_EVO_RECENT_LIMIT";
    /// 单条决策至少多少 tool 调用才计入「有意义」条数。默认 2。
    pub const SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS: &str = "SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS";
    /// 技能进化：有意义决策数 ≥ 此值且（有失败或存在重复模式）才触发。默认 3。
    pub const SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS: &str =
        "SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS";
    /// 记忆进化：有意义决策数 ≥ 此值才触发。默认 3。
    pub const SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY: &str =
        "SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY";
    /// 规则进化：有意义决策数 ≥ 此值且（失败次数或重规划次数达标）才触发。默认 5。
    pub const SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS: &str =
        "SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS";
    /// 规则进化：失败次数 ≥ 此值才考虑规则进化。默认 2。
    pub const SKILLLITE_EVO_FAILURES_MIN_PROMPTS: &str = "SKILLLITE_EVO_FAILURES_MIN_PROMPTS";
    /// 规则进化：重规划次数 ≥ 此值才考虑规则进化。默认 2。
    pub const SKILLLITE_EVO_REPLANS_MIN_PROMPTS: &str = "SKILLLITE_EVO_REPLANS_MIN_PROMPTS";
    /// 重复模式判定：同一模式出现次数 ≥ 此值且成功率达标才计为 repeated_pattern。默认 3。
    pub const SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT: &str =
        "SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT";
    /// 重复模式判定：成功率 ≥ 此值（0~1）。默认 0.8。
    pub const SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE: &str =
        "SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE";
    /// Acceptance window size (days) for auto-linking backlog acceptance status. Default 3.
    pub const SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS: &str = "SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS";
    /// Acceptance threshold: minimum first_success_rate in window. Default 0.70.
    pub const SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE: &str =
        "SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE";
    /// Acceptance threshold: maximum user_correction_rate in window. Default 0.20.
    pub const SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE: &str =
        "SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE";
    /// Acceptance threshold: maximum rollback_rate in window. Default 0.20.
    pub const SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE: &str =
        "SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE";
}

/// A11: 高危工具确认 — 可配置哪些操作需发消息确认
pub mod high_risk {
    /// SKILLLITE_HIGH_RISK_CONFIRM: 逗号分隔，如 "write_key_path,run_command,network"。
    /// 可选值: write_key_path, run_command, network。默认 "write_key_path,run_command,network"。
    /// "none" 表示全部跳过确认；"all" 等同默认。
    pub const SKILLLITE_HIGH_RISK_CONFIRM: &str = "SKILLLITE_HIGH_RISK_CONFIRM";
}

/// Agent 主循环：外层迭代上限与单任务工具调用预算
pub mod agent_loop {
    /// 全局迭代上限（默认 50）。有任务计划时，实际轮次还会与 `SKILLLITE_MAX_TOOL_CALLS_PER_TASK` 组合 capped。
    pub const SKILLLITE_MAX_ITERATIONS: &str = "SKILLLITE_MAX_ITERATIONS";
    /// 单任务内工具调用深度上限，并参与有计划时的有效迭代上限计算（默认 15）。
    pub const SKILLLITE_MAX_TOOL_CALLS_PER_TASK: &str = "SKILLLITE_MAX_TOOL_CALLS_PER_TASK";
}

/// 规划与 dependency-audit
pub mod misc {
    pub const SKILLLITE_COMPACT_PLANNING: &str = "SKILLLITE_COMPACT_PLANNING";
    pub const SKILLLITE_AUDIT_API: &str = "SKILLLITE_AUDIT_API";
    pub const PYPI_MIRROR_URL: &str = "PYPI_MIRROR_URL";
    pub const OSV_API_URL: &str = "OSV_API_URL";
}

/// 沙箱执行：级别、资源限制、开关等（SKILLLITE_* 优先，兼容 SKILLBOX_*）
pub mod sandbox {
    pub const SKILLLITE_SANDBOX_LEVEL: &str = "SKILLLITE_SANDBOX_LEVEL";
    pub const SANDBOX_LEVEL_ALIASES: &[&str] = &["SKILLBOX_SANDBOX_LEVEL"];

    pub const SKILLLITE_MAX_MEMORY_MB: &str = "SKILLLITE_MAX_MEMORY_MB";
    pub const MAX_MEMORY_MB_ALIASES: &[&str] = &["SKILLBOX_MAX_MEMORY_MB"];

    pub const SKILLLITE_TIMEOUT_SECS: &str = "SKILLLITE_TIMEOUT_SECS";
    pub const TIMEOUT_SECS_ALIASES: &[&str] = &["SKILLBOX_TIMEOUT_SECS"];

    pub const SKILLLITE_AUTO_APPROVE: &str = "SKILLLITE_AUTO_APPROVE";
    pub const AUTO_APPROVE_ALIASES: &[&str] = &["SKILLBOX_AUTO_APPROVE"];

    pub const SKILLLITE_NO_SANDBOX: &str = "SKILLLITE_NO_SANDBOX";
    pub const NO_SANDBOX_ALIASES: &[&str] = &["SKILLBOX_NO_SANDBOX"];

    /// When bwrap/firejail fail or are missing, allow weak PID/UTS/net namespace fallback (Linux only).
    pub const SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK: &str =
        "SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK";
    pub const ALLOW_LINUX_NAMESPACE_FALLBACK_ALIASES: &[&str] =
        &["SKILLBOX_ALLOW_LINUX_NAMESPACE_FALLBACK"];

    pub const SKILLLITE_ALLOW_PLAYWRIGHT: &str = "SKILLLITE_ALLOW_PLAYWRIGHT";
    pub const ALLOW_PLAYWRIGHT_ALIASES: &[&str] = &["SKILLBOX_ALLOW_PLAYWRIGHT"];

    pub const SKILLLITE_SCRIPT_ARGS: &str = "SKILLLITE_SCRIPT_ARGS";
    pub const SCRIPT_ARGS_ALIASES: &[&str] = &["SKILLBOX_SCRIPT_ARGS"];
}
