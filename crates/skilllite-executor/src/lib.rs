pub mod error;
pub mod memory;
pub mod plan;
pub mod rpc;
pub mod session;
pub mod transcript;

use crate::error::ExecutorError;
pub use error::{Error, Result};

/// Resolve skilllite data root. Delegates to [`skilllite_core::paths::data_root`].
pub fn skilllite_data_root() -> std::path::PathBuf {
    skilllite_core::paths::data_root()
}

/// Resolve chat root. Delegates to [`skilllite_core::paths::chat_root`].
pub fn chat_root() -> std::path::PathBuf {
    skilllite_core::paths::chat_root()
}

/// Resolve workspace root. Prefers SKILLLITE_WORKSPACE env, else ~/.skilllite
pub fn workspace_root(
    workspace_path: Option<&str>,
) -> std::result::Result<std::path::PathBuf, ExecutorError> {
    if let Some(p) = workspace_path {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            return Ok(path);
        }
        return Ok(std::env::current_dir()?.join(p));
    }
    Ok(skilllite_data_root())
}

/// Resolve chat root for session/transcript/memory RPC.
/// When workspace_path is None: use chat_root() (SKILLLITE_WORKSPACE/chat or ~/.skilllite/chat).
/// When provided: treat as data root and return path/chat. If path already ends with "chat", use as-is.
pub fn chat_root_for_rpc(
    workspace_path: Option<&str>,
) -> std::result::Result<std::path::PathBuf, ExecutorError> {
    if let Some(p) = workspace_path {
        let path = std::path::PathBuf::from(p);
        let data_root = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(p)
        };
        // 兼容：若传入的已是 chat root，不再追加 chat
        let is_chat_root = data_root.ends_with(std::path::Path::new("chat"));
        return Ok(if is_chat_root {
            data_root
        } else {
            data_root.join("chat")
        });
    }
    Ok(chat_root())
}
