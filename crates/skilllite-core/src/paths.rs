//! 数据根与 chat 根路径的唯一来源
//!
//! 规则：`SKILLLITE_WORKSPACE`（绝对路径）→ 否则 `~/.skilllite`；
//! chat 根 = `data_root/chat`。全仓库仅在此处维护该逻辑。

use std::path::PathBuf;

use crate::config::env_keys::paths as env_paths;

/// 解析 skilllite 数据根。
///
/// 优先使用环境变量 `SKILLLITE_WORKSPACE`（若为绝对路径），否则为 `~/.skilllite`。
pub fn data_root() -> PathBuf {
    if let Ok(ws) = std::env::var(env_paths::SKILLLITE_WORKSPACE) {
        let p = PathBuf::from(ws);
        if p.is_absolute() {
            return p;
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skilllite")
}

/// 解析 chat 根（会话、transcript、plans、memory 等）。
///
/// 即 `data_root().join("chat")`。
pub fn chat_root() -> PathBuf {
    data_root().join("chat")
}
