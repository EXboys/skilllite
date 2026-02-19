//! 按领域分组的配置结构体
//!
//! 从环境变量加载，统一 fallback 逻辑。

use super::env_keys::{observability as obv_keys, llm};
use super::loader::{env_bool, env_or, env_optional};
use std::path::PathBuf;

/// LLM API 配置
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

impl LlmConfig {
    /// 从环境变量加载，空值使用默认（会自动加载 .env）
    pub fn from_env() -> Self {
        super::loader::load_dotenv();
        Self {
            api_base: env_or(
                llm::API_BASE,
                llm::API_BASE_ALIASES,
                || "https://api.openai.com/v1".to_string(),
            ),
            api_key: env_or(llm::API_KEY, llm::API_KEY_ALIASES, String::new),
            model: env_or(llm::MODEL, llm::MODEL_ALIASES, || "gpt-4o".to_string()),
        }
    }

    /// 从环境变量加载，若 api_key 或 api_base 为空则返回 None
    pub fn try_from_env() -> Option<Self> {
        let cfg = Self::from_env();
        if cfg.api_key.trim().is_empty() || cfg.api_base.trim().is_empty() {
            None
        } else {
            Some(cfg)
        }
    }

    /// 默认 model（当未显式设置时，按 api_base 推断）
    pub fn default_model_for_base(api_base: &str) -> &'static str {
        if api_base.contains("localhost:11434") || api_base.contains("127.0.0.1:11434") {
            "qwen2.5:7b"
        } else if api_base.contains("api.openai.com") {
            "gpt-4o"
        } else if api_base.contains("api.deepseek.com") {
            "deepseek-chat"
        } else if api_base.contains("dashscope.aliyuncs.com") {
            "qwen-plus"
        } else {
            "gpt-4o"
        }
    }
}

/// 工作区与输出路径配置
#[derive(Debug, Clone)]
pub struct PathsConfig {
    pub workspace: String,
    pub output_dir: Option<String>,
    pub skills_repo: String,
    /// 沙箱内 skill 路径的根目录，用于 path validation
    pub skills_root: Option<String>,
}

impl PathsConfig {
    pub fn from_env() -> Self {
        super::loader::load_dotenv();
        let workspace = super::loader::env_optional(
            super::env_keys::paths::SKILLLITE_WORKSPACE,
            &[],
        )
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .to_string_lossy()
                .to_string()
        });

        let output_dir = super::loader::env_optional(
            super::env_keys::paths::SKILLLITE_OUTPUT_DIR,
            &[],
        );

        let skills_repo = super::loader::env_or(
            super::env_keys::paths::SKILLLITE_SKILLS_REPO,
            &[],
            || "EXboys/skilllite".to_string(),
        );

        let skills_root =
            super::loader::env_optional(super::env_keys::paths::SKILLBOX_SKILLS_ROOT, &[]);

        Self {
            workspace,
            output_dir,
            skills_repo,
            skills_root,
        }
    }
}

/// Agent 功能开关
#[derive(Debug, Clone)]
pub struct AgentFeatureFlags {
    pub enable_memory: bool,
    pub enable_task_planning: bool,
}

impl AgentFeatureFlags {
    pub fn from_env() -> Self {
        Self {
            enable_memory: env_bool("SKILLLITE_ENABLE_MEMORY", &[], true),
            enable_task_planning: env_bool("SKILLLITE_ENABLE_TASK_PLANNING", &[], true),
        }
    }
}

/// 可观测性配置：quiet、log_level、log_json、audit_log、security_events_log
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub quiet: bool,
    pub log_level: String,
    pub log_json: bool,
    pub audit_log: Option<String>,
    pub security_events_log: Option<String>,
}

impl ObservabilityConfig {
    pub fn from_env() -> &'static Self {
        use std::sync::OnceLock;
        static CACHE: OnceLock<ObservabilityConfig> = OnceLock::new();
        CACHE.get_or_init(|| {
            super::loader::load_dotenv();
        let quiet = env_bool(obv_keys::SKILLLITE_QUIET, obv_keys::QUIET_ALIASES, false);
        let log_level = env_or(
            obv_keys::SKILLLITE_LOG_LEVEL,
            obv_keys::LOG_LEVEL_ALIASES,
            || "skilllite=info".to_string(),
        );
        let log_json = env_bool(obv_keys::SKILLLITE_LOG_JSON, obv_keys::LOG_JSON_ALIASES, false);
        let audit_log = env_optional(obv_keys::SKILLLITE_AUDIT_LOG, obv_keys::AUDIT_LOG_ALIASES);
        let security_events_log =
            env_optional(obv_keys::SKILLLITE_SECURITY_EVENTS_LOG, &[]);
        Self {
            quiet,
            log_level,
            log_json,
            audit_log,
            security_events_log,
        }
        })
    }
}

/// 缓存目录配置
#[derive(Debug, Clone)]
pub struct CacheConfig;

impl CacheConfig {
    pub fn cache_dir() -> Option<String> {
        super::loader::load_dotenv();
        env_optional(
            super::env_keys::cache::SKILLLITE_CACHE_DIR,
            super::env_keys::cache::CACHE_DIR_ALIASES,
        )
    }
}
