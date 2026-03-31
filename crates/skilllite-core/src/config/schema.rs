//! 按领域分组的配置结构体
//!
//! 从环境变量加载，统一 fallback 逻辑。

use super::env_keys::{agent_loop as al_keys, llm, observability as obv_keys, sandbox as sb_keys};
use super::loader::{env_bool, env_optional, env_or};
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
            api_base: env_or(llm::API_BASE, llm::API_BASE_ALIASES, || {
                "https://api.openai.com/v1".to_string()
            }),
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
        } else if api_base.contains("minimax") {
            "MiniMax-M2.5"
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
    pub data_dir: PathBuf,
}

impl PathsConfig {
    pub fn from_env() -> Self {
        let default_data_dir = crate::paths::data_root();
        super::loader::load_dotenv();
        let workspace =
            super::loader::env_optional(super::env_keys::paths::SKILLLITE_WORKSPACE, &[])
                .unwrap_or_else(|| {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .to_string_lossy()
                        .to_string()
                });

        let output_dir =
            super::loader::env_optional(super::env_keys::paths::SKILLLITE_OUTPUT_DIR, &[]);

        let skills_repo =
            super::loader::env_or(super::env_keys::paths::SKILLLITE_SKILLS_REPO, &[], || {
                "EXboys/skilllite".to_string()
            });

        let skills_root =
            super::loader::env_optional(super::env_keys::paths::SKILLBOX_SKILLS_ROOT, &[]);

        Self {
            workspace,
            output_dir,
            skills_repo,
            skills_root,
            data_dir: default_data_dir,
        }
    }
}

/// Agent 功能开关
#[derive(Debug, Clone)]
pub struct AgentFeatureFlags {
    pub enable_memory: bool,
    pub enable_task_planning: bool,
    /// 启用 Memory 向量检索（需 memory_vector feature + embedding API）
    pub enable_memory_vector: bool,
}

impl AgentFeatureFlags {
    pub fn from_env() -> Self {
        Self {
            enable_memory: env_bool("SKILLLITE_ENABLE_MEMORY", &[], true),
            enable_task_planning: env_bool("SKILLLITE_ENABLE_TASK_PLANNING", &[], true),
            enable_memory_vector: env_bool("SKILLLITE_ENABLE_MEMORY_VECTOR", &[], false),
        }
    }
}

/// Agent 主循环预算：与 `skilllite_agent::types::AgentConfig` 中对应字段一致
#[derive(Debug, Clone)]
pub struct AgentLoopLimitsConfig {
    pub max_iterations: usize,
    pub max_tool_calls_per_task: usize,
}

impl AgentLoopLimitsConfig {
    pub fn from_env() -> Self {
        super::loader::load_dotenv();
        Self {
            max_iterations: parse_positive_usize(
                super::loader::env_optional(al_keys::SKILLLITE_MAX_ITERATIONS, &[]),
                50,
            ),
            max_tool_calls_per_task: parse_positive_usize(
                super::loader::env_optional(al_keys::SKILLLITE_MAX_TOOL_CALLS_PER_TASK, &[]),
                15,
            ),
        }
    }
}

fn parse_positive_usize(raw: Option<String>, default: usize) -> usize {
    raw.and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(default)
}

/// Embedding API 配置（用于 memory vector 检索）
#[derive(Debug, Clone)]
#[allow(dead_code)] // used when memory_vector feature is enabled
pub struct EmbeddingConfig {
    pub model: String,
    pub dimension: usize,
    pub api_base: String,
    pub api_key: String,
}

impl EmbeddingConfig {
    pub fn from_env() -> Self {
        super::loader::load_dotenv();
        // 支持独立的 embedding API 配置
        let api_base = super::loader::env_or(
            "SKILLLITE_EMBEDDING_BASE_URL",
            &["EMBEDDING_BASE_URL"],
            || {
                super::loader::env_or(llm::API_BASE, llm::API_BASE_ALIASES, || {
                    "https://api.openai.com/v1".to_string()
                })
            },
        );
        let api_key = super::loader::env_or(
            "SKILLLITE_EMBEDDING_API_KEY",
            &["EMBEDDING_API_KEY"],
            || super::loader::env_or(llm::API_KEY, llm::API_KEY_ALIASES, || "".to_string()),
        );
        let (default_model, default_dim) = Self::default_for_base(&api_base);
        let model =
            super::loader::env_or("SKILLLITE_EMBEDDING_MODEL", &["EMBEDDING_MODEL"], || {
                default_model.to_string()
            });
        let dimension = super::loader::env_optional("SKILLLITE_EMBEDDING_DIMENSION", &[])
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(default_dim);
        Self {
            model,
            dimension,
            api_base,
            api_key,
        }
    }

    /// 按 api_base 推断默认 embedding 模型和维度
    fn default_for_base(api_base: &str) -> (&'static str, usize) {
        let base_lower = api_base.to_lowercase();
        if base_lower.contains("dashscope.aliyuncs.com") {
            // 通义千问 / Qwen
            ("text-embedding-v3", 1024)
        } else if base_lower.contains("api.deepseek.com") {
            ("deepseek-embedding", 1536)
        } else if base_lower.contains("generativelanguage.googleapis.com") {
            // Google Gemini API (OpenAI 兼容端点)
            ("gemini-embedding-001", 3072)
        } else if base_lower.contains("localhost:11434") || base_lower.contains("127.0.0.1:11434") {
            // Ollama
            ("nomic-embed-text", 768)
        } else if base_lower.contains("minimax") {
            // MiniMax embedding
            ("text-embedding-01", 1536)
        } else {
            ("text-embedding-3-small", 1536)
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
                || "skilllite=info,skilllite_swarm=info".to_string(),
            );
            let log_json = env_bool(
                obv_keys::SKILLLITE_LOG_JSON,
                obv_keys::LOG_JSON_ALIASES,
                false,
            );
            let audit_disabled = env_bool(obv_keys::SKILLLITE_AUDIT_DISABLED, &[], false);
            let audit_log = if audit_disabled {
                None
            } else {
                env_optional(obv_keys::SKILLLITE_AUDIT_LOG, obv_keys::AUDIT_LOG_ALIASES).or_else(
                    || {
                        Some(
                            crate::paths::data_root()
                                .join("audit")
                                .to_string_lossy()
                                .into_owned(),
                        )
                    },
                )
            };
            let security_events_log = env_optional(obv_keys::SKILLLITE_SECURITY_EVENTS_LOG, &[]);
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

/// 沙箱环境配置：级别、资源限制、开关等
///
/// 所有沙箱相关环境变量统一由此读取，兼容 `SKILLLITE_*` 与 `SKILLBOX_*`。
/// runner、linux、macos、windows、policy 等应通过本配置访问，不再直接使用 env_compat。
#[derive(Debug, Clone)]
pub struct SandboxEnvConfig {
    /// 沙箱级别 1/2/3，默认 3
    pub sandbox_level: u8,
    /// 最大内存 MB，默认 256
    pub max_memory_mb: u64,
    /// 执行超时秒数，默认 30
    pub timeout_secs: u64,
    /// 是否自动批准 L3 安全提示
    pub auto_approve: bool,
    /// 是否禁用沙箱（等同于 level 1）
    pub no_sandbox: bool,
    /// Linux：bwrap/firejail 不可用或失败时，是否允许弱命名空间降级（默认 false，与 Windows fail-closed 对齐）
    pub allow_linux_namespace_fallback: bool,
    /// 是否允许 Playwright（放宽沙箱）
    pub allow_playwright: bool,
    /// 透传给脚本的额外参数（SKILLLITE_SCRIPT_ARGS / SKILLBOX_SCRIPT_ARGS）
    pub script_args: Option<String>,
}

impl SandboxEnvConfig {
    pub fn from_env() -> Self {
        super::loader::load_dotenv();
        let sandbox_level = env_or(
            sb_keys::SKILLLITE_SANDBOX_LEVEL,
            sb_keys::SANDBOX_LEVEL_ALIASES,
            || "3".to_string(),
        )
        .parse::<u8>()
        .ok()
        .and_then(|n| if (1..=3).contains(&n) { Some(n) } else { None })
        .unwrap_or(3);

        let max_memory_mb = env_or(
            sb_keys::SKILLLITE_MAX_MEMORY_MB,
            sb_keys::MAX_MEMORY_MB_ALIASES,
            || "256".to_string(),
        )
        .parse::<u64>()
        .ok()
        .unwrap_or(256);

        let timeout_secs = env_or(
            sb_keys::SKILLLITE_TIMEOUT_SECS,
            sb_keys::TIMEOUT_SECS_ALIASES,
            || "30".to_string(),
        )
        .parse::<u64>()
        .ok()
        .unwrap_or(30);

        let auto_approve = env_bool(
            sb_keys::SKILLLITE_AUTO_APPROVE,
            sb_keys::AUTO_APPROVE_ALIASES,
            false,
        );
        let no_sandbox = env_bool(
            sb_keys::SKILLLITE_NO_SANDBOX,
            sb_keys::NO_SANDBOX_ALIASES,
            false,
        );
        let allow_linux_namespace_fallback = env_bool(
            sb_keys::SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK,
            sb_keys::ALLOW_LINUX_NAMESPACE_FALLBACK_ALIASES,
            false,
        );
        let allow_playwright = env_bool(
            sb_keys::SKILLLITE_ALLOW_PLAYWRIGHT,
            sb_keys::ALLOW_PLAYWRIGHT_ALIASES,
            false,
        );
        let script_args =
            env_optional(sb_keys::SKILLLITE_SCRIPT_ARGS, sb_keys::SCRIPT_ARGS_ALIASES);

        Self {
            sandbox_level,
            max_memory_mb,
            timeout_secs,
            auto_approve,
            no_sandbox,
            allow_linux_namespace_fallback,
            allow_playwright,
            script_args,
        }
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

#[cfg(test)]
mod agent_loop_limits_tests {
    use super::super::loader::{remove_env_var, set_env_var};
    use super::al_keys;
    use super::AgentLoopLimitsConfig;
    use std::env;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env test lock poisoned")
    }

    #[test]
    fn from_env_reads_max_iterations_and_max_tool_calls_per_task() {
        let _g = lock_env();
        let k_it = al_keys::SKILLLITE_MAX_ITERATIONS;
        let k_tc = al_keys::SKILLLITE_MAX_TOOL_CALLS_PER_TASK;
        let prev_it = env::var(k_it).ok();
        let prev_tc = env::var(k_tc).ok();
        set_env_var(k_it, "12");
        set_env_var(k_tc, "4");
        let cfg = AgentLoopLimitsConfig::from_env();
        assert_eq!(cfg.max_iterations, 12);
        assert_eq!(cfg.max_tool_calls_per_task, 4);
        restore(k_it, prev_it.as_deref());
        restore(k_tc, prev_tc.as_deref());
    }

    #[test]
    fn from_env_invalid_or_zero_falls_back_to_defaults() {
        let _g = lock_env();
        let k_it = al_keys::SKILLLITE_MAX_ITERATIONS;
        let k_tc = al_keys::SKILLLITE_MAX_TOOL_CALLS_PER_TASK;
        let prev_it = env::var(k_it).ok();
        let prev_tc = env::var(k_tc).ok();
        set_env_var(k_it, "0");
        set_env_var(k_tc, "not_a_number");
        let cfg = AgentLoopLimitsConfig::from_env();
        assert_eq!(cfg.max_iterations, 50);
        assert_eq!(cfg.max_tool_calls_per_task, 15);
        restore(k_it, prev_it.as_deref());
        restore(k_tc, prev_tc.as_deref());
    }

    fn restore(key: &str, value: Option<&str>) {
        match value {
            Some(v) => set_env_var(key, v),
            None => remove_env_var(key),
        }
    }
}
