//! Skills invocation: wraps sandbox run/exec for the agent layer.
//!
//! Since Agent is in the same process as Sandbox, we call the sandbox
//! executor directly (no IPC needed). Ported from Python `ToolCallHandler`.
//!
//! Phase 2.5 additions:
//!   - Security scanning before skill execution (L3)
//!   - Multi-script skill support (skill_name__script_name)
//!   - Argparse schema inference for Python scripts
//!   - .skilllite.lock dependency resolution

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use skilllite_core::skill::metadata::SkillMetadata;

use super::types::ToolDefinition;

use loader::{load_single_skill, load_evolved_skills, sanitize_tool_name};

mod loader;
mod executor;
pub(crate) mod security;

pub use executor::execute_skill;
pub use security::{LockFile, read_lock_file, write_lock_file};

/// A loaded skill ready for invocation.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub name: String,
    pub skill_dir: PathBuf,
    pub metadata: SkillMetadata,
    pub tool_definitions: Vec<ToolDefinition>,
    /// Multi-script tool mapping: tool_name → script_path (e.g. "scripts/init_skill.py")
    pub multi_script_entries: HashMap<String, String>,
}

/// Load skills from directories, parse SKILL.md, generate tool definitions.
/// Also loads evolved skills from `_evolved/` subdirectories (EVO-4),
/// skipping archived ones based on `.meta.json`.
pub fn load_skills(skill_dirs: &[String]) -> Vec<LoadedSkill> {
    let mut skills = Vec::new();

    for dir_path in skill_dirs {
        let path = Path::new(dir_path);
        if !path.exists() || !path.is_dir() {
            tracing::debug!("Skill directory not found: {}", dir_path);
            continue;
        }

        // Check if this directory itself is a skill (has SKILL.md)
        if path.join("SKILL.md").exists() {
            if let Some(skill) = load_single_skill(path) {
                skills.push(skill);
            }
        } else {
            // Scan subdirectories for skills
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    let entry_path = entry.path();
                    if entry_path.is_dir() && entry_path.join("SKILL.md").exists() {
                        if let Some(skill) = load_single_skill(&entry_path) {
                            skills.push(skill);
                        }
                    }
                }
            }
        }

        // EVO-4: load evolved skills from _evolved/ subdirectory
        let evolved_dir = path.join("_evolved");
        if evolved_dir.exists() && evolved_dir.is_dir() {
            let evolved = load_evolved_skills(&evolved_dir);
            tracing::debug!("Loaded {} evolved skills from {}", evolved.len(), evolved_dir.display());
            skills.extend(evolved);
        }
    }

    skills
}

/// Load evolved skills from `_evolved/` directory, filtering out archived ones.

/// Find a loaded skill by tool name.
///
/// Supports fuzzy matching: normalizes both the query and registered names
/// so that `frontend-design` matches `frontend_design` and vice versa.
/// This is needed because LLMs sometimes use the original skill name (with hyphens)
/// instead of the sanitized tool name (with underscores).
pub fn find_skill_by_tool_name<'a>(
    skills: &'a [LoadedSkill],
    tool_name: &str,
) -> Option<&'a LoadedSkill> {
    // Exact match first (fast path)
    if let Some(skill) = skills.iter().find(|s| {
        s.tool_definitions.iter().any(|td| td.function.name == tool_name)
    }) {
        return Some(skill);
    }

    // Normalized match: replace hyphens with underscores and compare
    let normalized = sanitize_tool_name(tool_name);
    skills.iter().find(|s| {
        s.tool_definitions.iter().any(|td| td.function.name == normalized)
    })
}

/// Find a loaded skill by its original name (not tool definition name).
///
/// This is useful for finding reference-only skills that have no tool definitions
/// but are still loaded and available for documentation injection.
/// Matches both exact name and normalized name (hyphens ↔ underscores).
pub fn find_skill_by_name<'a>(
    skills: &'a [LoadedSkill],
    name: &str,
) -> Option<&'a LoadedSkill> {
    // Exact match
    if let Some(skill) = skills.iter().find(|s| s.name == name) {
        return Some(skill);
    }
    // Normalized: frontend_design matches frontend-design
    let with_hyphens = name.replace('_', "-");
    let with_underscores = name.replace('-', "_");
    skills.iter().find(|s| s.name == with_hyphens || s.name == with_underscores)
}
