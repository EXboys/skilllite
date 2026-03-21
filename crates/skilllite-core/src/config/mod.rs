//! SkillLite 统一配置层
//!
//! 所有环境变量读取集中在此模块，业务代码通过结构化配置访问，避免直接 `std::env::var`。
//!
//! **配置来源优先级**（高 → 低）：CLI/显式参数 > 环境变量 > .env 文件 > 默认值。
//! 详见 `docs/zh/ENV_REFERENCE.md` 的「配置来源优先级」章节。
//!
//! - `loader`：env_or、env_optional、env_bool、load_dotenv、parse_dotenv_* 等
//! - `schema`：LlmConfig、PathsConfig、AgentFeatureFlags
//! - `env_keys`：key 常量（含 legacy 向后兼容）

pub mod env_keys;
pub mod loader;
pub mod schema;

pub use loader::{
    ensure_default_output_dir, init_daemon_env, init_llm_env, remove_env_var, set_env_var,
    supply_chain_block_enabled, ScopedEnvGuard,
};
#[allow(unused_imports)] // 供后续迁移 observability 等模块使用
pub use loader::{
    env_bool, env_optional, env_or, load_dotenv, load_dotenv_from_dir, parse_dotenv_from_dir,
    parse_dotenv_walking_up,
};
pub use schema::{
    AgentFeatureFlags, CacheConfig, EmbeddingConfig, LlmConfig, ObservabilityConfig, PathsConfig,
    SandboxEnvConfig,
};
