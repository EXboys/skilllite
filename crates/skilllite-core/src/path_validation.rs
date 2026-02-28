//! Path validation utilities.
//!
//! Ensures paths stay within allowed root to prevent path traversal attacks.

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Get the allowed root directory for path validation.
pub fn get_allowed_root() -> Result<PathBuf> {
    let allowed_root = crate::config::PathsConfig::from_env()
        .skills_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    allowed_root
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Invalid SKILLLITE_SKILLS_ROOT: {}", e))
}

/// Validate path is within allowed root. Prevents path traversal.
pub fn validate_path_under_root(path: &str, path_type: &str) -> Result<PathBuf> {
    let allowed_root = get_allowed_root()?;
    let input = Path::new(path);
    let full = if input.is_absolute() {
        input.to_path_buf()
    } else {
        allowed_root.join(input)
    };
    let canonical = full
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("{} does not exist: {}", path_type, path))?;
    if !canonical.starts_with(&allowed_root) {
        anyhow::bail!("{} escapes allowed root: {}", path_type, path);
    }
    Ok(canonical)
}

/// Validate skill_dir is within allowed root. Prevents path traversal.
pub fn validate_skill_path(skill_dir: &str) -> Result<PathBuf> {
    validate_path_under_root(skill_dir, "Skill path")
}
