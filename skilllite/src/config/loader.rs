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
            eprintln!(
                "[DEPRECATED] 以下环境变量已废弃，建议迁移：\n   {}",
                hints.join("\n   ")
            );
            eprintln!("   详见 docs/zh/ENV_REFERENCE.md");
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
