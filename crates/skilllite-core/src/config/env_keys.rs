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
    pub const SKILLLITE_EVOLUTION_DECISION_THRESHOLD: &str = "SKILLLITE_EVOLUTION_DECISION_THRESHOLD";
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
