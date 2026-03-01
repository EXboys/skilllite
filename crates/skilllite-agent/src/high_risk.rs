//! A11: 高危工具确认 — 可配置 write_key_path、run_command、network 等需发消息确认
//!
//! 环境变量 SKILLLITE_HIGH_RISK_CONFIRM: 逗号分隔，如 "write_key_path,run_command,network"。
//! - "none": 全部跳过确认
//! - "all": 等同默认（全部确认）
//! - 默认: "write_key_path,run_command,network"

use std::sync::LazyLock;
use std::collections::HashSet;

static CONFIRM_SET: LazyLock<HashSet<String>> = LazyLock::new(|| {
    skilllite_core::config::load_dotenv();
    let raw = std::env::var(skilllite_core::config::env_keys::high_risk::SKILLLITE_HIGH_RISK_CONFIRM)
        .unwrap_or_else(|_| "write_key_path,run_command,network".to_string());
    let raw = raw.trim().to_lowercase();
    if raw == "none" {
        return HashSet::new();
    }
    if raw == "all" || raw.is_empty() {
        return HashSet::from([
            "write_key_path".to_string(),
            "run_command".to_string(),
            "network".to_string(),
        ]);
    }
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
});

/// 写关键路径是否需要确认
pub fn confirm_write_key_path() -> bool {
    CONFIRM_SET.contains("write_key_path")
}

/// run_command 是否需要确认
pub fn confirm_run_command() -> bool {
    CONFIRM_SET.contains("run_command")
}

/// 网络 skill 执行是否需要确认
pub fn confirm_network() -> bool {
    CONFIRM_SET.contains("network")
}
