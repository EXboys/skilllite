//! Path validation utilities.
//!
//! Ensures paths stay within allowed root to prevent path traversal attacks.

use crate::error::PathValidationError;
use std::path::{Path, PathBuf};

/// Get the allowed root directory for path validation.
pub fn get_allowed_root() -> Result<PathBuf, PathValidationError> {
    let allowed_root = crate::config::PathsConfig::from_env()
        .skills_root
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    allowed_root
        .canonicalize()
        .map_err(PathValidationError::InvalidRoot)
}

/// Validate path is within allowed root. Prevents path traversal.
pub fn validate_path_under_root(
    path: &str,
    path_type: &str,
) -> Result<PathBuf, PathValidationError> {
    let allowed_root = get_allowed_root()?;
    let input = Path::new(path);
    let full = if input.is_absolute() {
        input.to_path_buf()
    } else {
        allowed_root.join(input)
    };
    let canonical = full
        .canonicalize()
        .map_err(|_| PathValidationError::NotFound {
            path_type: path_type.to_string(),
            path: path.to_string(),
        })?;
    if !canonical.starts_with(&allowed_root) {
        return Err(PathValidationError::PathEscape {
            path_type: path_type.to_string(),
            path: path.to_string(),
        });
    }
    Ok(canonical)
}

/// Validate skill_dir is within allowed root. Prevents path traversal.
pub fn validate_skill_path(skill_dir: &str) -> Result<PathBuf, PathValidationError> {
    validate_path_under_root(skill_dir, "Skill path")
}
