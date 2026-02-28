//! 统一环境变量加载逻辑
//!
//! 集中维护 fallback 链，避免在业务代码中重复 `or_else` 调用。

use std::env;

/// 废弃变量 → 推荐变量映射（用于检测并提示迁移）
const DEPRECATED_PAIRS: &[(&str, &str)] = &[
    ("SKILLBOX_AUDIT_LOG", "SKILLLITE_AUDIT_LOG"),
    ("SKILLBOX_QUIET", "SKILLLITE_QUIET"),
    ("SKILLBOX_CACHE_DIR", "SKILLLITE_CACHE_DIR"),
    ("AGENTSKILL_CACHE_DIR", "SKILLLITE_CACHE_DIR"),
    ("SKILLBOX_LOG_LEVEL", "SKILLLITE_LOG_LEVEL"),
    ("SKILLBOX_LOG_JSON", "SKILLLITE_LOG_JSON"),
];

/// 检测废弃变量：若使用了废弃变量且未设置推荐变量，打印一次迁移提示
fn warn_deprecated_env_vars() {
    use std::sync::Once;
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        let mut hints = Vec::new();
        for (deprecated, recommended) in DEPRECATED_PAIRS {
            if env::var(deprecated).is_ok() && env::var(recommended).is_err() {
                hints.push(format!("{} → {}", deprecated, recommended));
            }
        }
        if !hints.is_empty() {
            tracing::warn!(
                "[DEPRECATED] 以下环境变量已废弃，建议迁移：\n   {}\n   详见 docs/zh/ENV_REFERENCE.md",
                hints.join("\n   ")
            );
        }
    });
}

/// 加载当前目录下的 `.env` 到环境变量（不覆盖已存在的变量）
pub fn load_dotenv() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let path = env::current_dir()
            .map(|d| d.join(".env"))
            .unwrap_or_else(|_| std::path::PathBuf::from(".env"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(eq_pos) = line.find('=') {
                    let key = line[..eq_pos].trim();
                    let mut value = line[eq_pos + 1..].trim();
                    // Strip inline comment (# not inside quotes)
                    if let Some(hash_pos) = value.find('#') {
                        let before_hash = value[..hash_pos].trim_end();
                        if !before_hash.contains('"') && !before_hash.contains('\'') {
                            value = before_hash;
                        }
                    }
                    if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        value = &value[1..value.len() - 1];
                    }
                    if !key.is_empty() && env::var(key).is_err() {
                        #[allow(unsafe_code)]
                        unsafe {
                            env::set_var(key, value);
                        }
                    }
                }
            }
        }
        warn_deprecated_env_vars();
    });
}

/// 从主变量或别名链读取环境变量，失败时使用默认值
pub fn env_or<F>(primary: &str, aliases: &[&str], default: F) -> String
where
    F: FnOnce() -> String,
{
    env::var(primary)
        .ok()
        .or_else(|| aliases.iter().find_map(|a| env::var(a).ok()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(default)
}

/// 从主变量或别名链读取，返回 Option（空值视为未设置）
pub fn env_optional(primary: &str, aliases: &[&str]) -> Option<String> {
    env::var(primary)
        .ok()
        .or_else(|| aliases.iter().find_map(|a| env::var(a).ok()))
        .and_then(|s| {
            let s = s.trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
}

/// 解析布尔型环境变量：1/true/yes 为 true，0/false/no 为 false
pub fn env_bool(primary: &str, aliases: &[&str], default: bool) -> bool {
    let v = env::var(primary)
        .ok()
        .or_else(|| aliases.iter().find_map(|a| env::var(a).ok()));
    match v.as_deref() {
        Some(s) => !matches!(
            s.trim().to_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        None => default,
    }
}

/// 检查环境变量是否存在（任意主变量或别名）
#[allow(dead_code)] // 供后续迁移使用
pub fn env_is_set(primary: &str, aliases: &[&str]) -> bool {
    env::var(primary).is_ok()
        || aliases.iter().any(|a| env::var(a).is_ok())
}

// ─── 集中式 env::set_var / remove_var 包装 ─────────────────────────────────
//
// 所有对 `std::env::set_var` / `remove_var` 的调用都应通过下面的函数进行，
// 业务代码不再直接出现 `unsafe { env::set_var(...) }`。
//
// SAFETY 约定：调用方需确保在多线程启动前（tokio runtime 创建前）调用。

/// 设置单个环境变量（unsafe 集中在此处）
#[allow(unsafe_code)]
pub fn set_env_var(key: &str, value: &str) {
    unsafe { env::set_var(key, value) };
}

/// 移除单个环境变量
#[allow(unsafe_code)]
pub fn remove_env_var(key: &str) {
    unsafe { env::remove_var(key) };
}

/// 初始化 LLM 环境变量（api_base / api_key / model）
///
/// quickstart 等入口在 tokio runtime 启动前调用。
pub fn init_llm_env(api_base: &str, api_key: &str, model: &str) {
    set_env_var("OPENAI_API_BASE", api_base);
    set_env_var("OPENAI_API_KEY", api_key);
    set_env_var("SKILLLITE_MODEL", model);
}

/// 初始化 daemon/stdio 模式的静默环境变量
pub fn init_daemon_env() {
    set_env_var("SKILLBOX_AUTO_APPROVE", "1");
    set_env_var("SKILLLITE_QUIET", "1");
}

/// 确保 `SKILLLITE_OUTPUT_DIR` 有值，若未设置则使用 `~/.skilllite/chat/output/`。
///
/// 同时创建目录（若不存在）。chat 和 agent-rpc 入口共用。
pub fn ensure_default_output_dir() {
    let paths = super::PathsConfig::from_env();
    if paths.output_dir.is_none() {
        let chat_output = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".skilllite")
            .join("chat")
            .join("output");
        let s = chat_output.to_string_lossy().to_string();
        set_env_var("SKILLLITE_OUTPUT_DIR", &s);
        if !chat_output.exists() {
            let _ = std::fs::create_dir_all(&chat_output);
        }
    } else if let Some(ref output_dir) = paths.output_dir {
        let p = std::path::PathBuf::from(output_dir);
        if !p.exists() {
            let _ = std::fs::create_dir_all(&p);
        }
    }
}

/// RAII guard：drop 时通过 [`remove_env_var`] 清除指定环境变量。
///
/// 用于 exec_script 等需要临时设置再还原的场景。
pub struct ScopedEnvGuard(pub &'static str);

impl Drop for ScopedEnvGuard {
    fn drop(&mut self) {
        remove_env_var(self.0);
    }
}
