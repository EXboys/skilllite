//! Minimal environment spec for sandbox env building.
//!
//! Sandbox does not depend on `skill::metadata`; callers (e.g. commands) build
//! `EnvSpec` from `SkillMetadata` and pass it to `ensure_environment`.

use std::path::Path;

use crate::skill::metadata;

/// Minimal input for building an isolated runtime environment (venv / node_modules).
///
/// Built from `SkillMetadata` by the caller so that the sandbox crate does not
/// depend on skill parsing.
#[derive(Debug, Clone)]
pub struct EnvSpec {
    /// Resolved language: "python", "node", or "bash"
    pub language: String,
    /// Skill name (for cache key / logging)
    pub name: Option<String>,
    /// Raw compatibility string (for bash+agent-browser detection, etc.)
    pub compatibility: Option<String>,
    /// Resolved package list (from .skilllite.lock or parsed compatibility)
    pub resolved_packages: Option<Vec<String>>,
}

impl EnvSpec {
    /// Build an env spec from skill metadata. Uses `metadata::detect_language`.
    pub fn from_metadata(skill_dir: &Path, meta: &metadata::SkillMetadata) -> Self {
        let language = metadata::detect_language(skill_dir, meta);
        let language = if language == "bash" {
            let has_pkg = skill_dir.join("package.json").exists();
            let compat_has_agent_browser = meta
                .compatibility
                .as_ref()
                .map_or(false, |c| c.to_lowercase().contains("agent-browser"));
            let resolved_has_agent_browser = meta
                .resolved_packages
                .as_ref()
                .map_or(false, |p| p.iter().any(|s| s.contains("agent-browser")));
            if has_pkg || compat_has_agent_browser || resolved_has_agent_browser {
                "node".to_string()
            } else {
                language
            }
        } else {
            language
        };

        Self {
            language,
            name: Some(meta.name.clone()),
            compatibility: meta.compatibility.clone(),
            resolved_packages: meta.resolved_packages.clone(),
        }
    }
}
