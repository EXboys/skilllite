//! SkillLite 统一配置层
//!
//! 所有环境变量读取集中在此模块，业务代码通过结构化配置访问，避免直接 `std::env::var`。
//!
//! - `loader`：env_or、env_optional、env_bool 等辅助函数
//! - `schema`：LlmConfig、PathsConfig、AgentFeatureFlags
//! - `env_keys`：key 常量（含 legacy 向后兼容）

pub mod env_keys;
pub mod loader;
pub mod schema;

#[allow(unused_imports)] // 供后续迁移 observability 等模块使用
pub use loader::{env_bool, env_optional, env_or, load_dotenv, load_dotenv_from_dir};
pub use loader::{
    ensure_default_output_dir, init_daemon_env, init_llm_env, remove_env_var, set_env_var,
    ScopedEnvGuard,
};
pub use schema::{
    AgentFeatureFlags, CacheConfig, EmbeddingConfig, LlmConfig, ObservabilityConfig, PathsConfig,
};
