//! 数据根与 chat 根路径的唯一来源
//!
//! 规则：`SKILLLITE_WORKSPACE`（绝对路径）→ 否则 `~/.skilllite`；
//! chat 根 = `data_root/chat`。全仓库仅在此处维护该逻辑。
//!
//! 默认 **agent 输出目录**（`SKILLLITE_OUTPUT_DIR` 未设置时）使用
//! [`resolve_workspace_filesystem_root`] 与 `PathsConfig.workspace` 一致：绝对路径原样使用，
//! 相对路径相对当前工作目录解析，再追加 `output/`（见 `config::loader::ensure_default_output_dir`）。

use std::path::{Path, PathBuf};

use crate::config::env_keys::paths as env_paths;

/// 将配置里的 workspace 字符串解析为用于落盘的根目录（与 [`crate::config::schema::PathsConfig`] 语义一致）。
pub fn resolve_workspace_filesystem_root(workspace: &str) -> PathBuf {
    let trimmed = workspace.trim();
    let p = Path::new(trimmed);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(trimmed)
    }
}

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
