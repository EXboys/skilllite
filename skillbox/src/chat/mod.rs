//! Chat module: session, transcript, memory.
//!
//! Only compiled when the `chat` feature is enabled.

pub mod memory;
pub mod rpc;
pub mod session;
pub mod transcript;

use anyhow::Result;

/// Resolve workspace root. Prefers SKILLLITE_WORKSPACE env, else ~/.skilllite
pub fn workspace_root(workspace_path: Option<&str>) -> Result<std::path::PathBuf> {
    if let Some(p) = workspace_path {
        let path = std::path::PathBuf::from(p);
        if path.is_absolute() {
            return Ok(path);
        }
        return Ok(std::env::current_dir()?.join(p));
    }
    Ok(dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".skilllite"))
}
