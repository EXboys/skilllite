//! Chat/data roots — mirrors `skilllite-core::paths` (L3).

use std::path::PathBuf;

use super::env_keys::paths as env_paths;

pub fn resolve_workspace_filesystem_root(workspace: &str) -> PathBuf {
    let trimmed = workspace.trim();
    let p = std::path::Path::new(trimmed);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(trimmed)
    }
}

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

pub fn chat_root() -> PathBuf {
    data_root().join("chat")
}

pub fn default_runtime_cache_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("SKILLLITE_CACHE_DIR") {
        let p = PathBuf::from(d);
        if p.is_absolute() {
            return Some(p.join("runtime"));
        }
    }
    dirs::home_dir().map(|h| h.join(".cache").join("skilllite").join("runtime"))
}
