pub mod memory;
pub mod plan;
pub mod rpc;
pub mod session;
pub mod transcript;

use anyhow::Result;

/// Resolve skilllite data root (~/.skilllite). Used for chat, memory, etc.
pub fn skilllite_data_root() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".skilllite")
}

/// Resolve chat root (~/.skilllite/chat). Sessions, transcripts, plans, memory.
pub fn chat_root() -> std::path::PathBuf {
    skilllite_data_root().join("chat")
}

/// Resolve workspace root. Prefers SKILLLITE_WORKSPACE env, else ~/.skilllite
pub fn workspace_root(workspace_path: Option<&str>) -> Result<std::path::PathBuf> {
    if let Some(p) = workspace_path {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            return Ok(path);
        }
        return Ok(std::env::current_dir()?.join(p));
    }
    Ok(skilllite_data_root())
}
