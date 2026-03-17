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

    pub const SKILLLITE_SECURITY_EVENTS_LOG: &str = "SKILLLITE_SECURITY_EVENTS_LOG";
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
    /// A9: Periodic evolution interval (seconds). Default 1800 (30 min). Evolution runs even when user is active.
    pub const SKILLLITE_EVOLUTION_INTERVAL_SECS: &str = "SKILLLITE_EVOLUTION_INTERVAL_SECS";
    /// A9: Decision count threshold. When unprocessed decisions >= this, trigger evolution. Default 10.
    pub const SKILLLITE_EVOLUTION_DECISION_THRESHOLD: &str =
        "SKILLLITE_EVOLUTION_DECISION_THRESHOLD";
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
}

/// A11: 高危工具确认 — 可配置哪些操作需发消息确认
pub mod high_risk {
    /// SKILLLITE_HIGH_RISK_CONFIRM: 逗号分隔，如 "write_key_path,run_command,network"。
    /// 可选值: write_key_path, run_command, network。默认 "write_key_path,run_command,network"。
    /// "none" 表示全部跳过确认；"all" 等同默认。
    pub const SKILLLITE_HIGH_RISK_CONFIRM: &str = "SKILLLITE_HIGH_RISK_CONFIRM";
}

/// 规划与 dependency-audit
pub mod misc {
    pub const SKILLLITE_COMPACT_PLANNING: &str = "SKILLLITE_COMPACT_PLANNING";
    pub const SKILLLITE_AUDIT_API: &str = "SKILLLITE_AUDIT_API";
    pub const PYPI_MIRROR_URL: &str = "PYPI_MIRROR_URL";
    pub const OSV_API_URL: &str = "OSV_API_URL";
}
