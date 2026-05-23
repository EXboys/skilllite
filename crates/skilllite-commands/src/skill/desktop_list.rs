//! `skilllite skills list --json` — workspace skill discovery for desktop UI (L2).

use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use skilllite_core::skill::deps;
use skilllite_core::skill::discovery::discover_skill_instances_in_workspace;
use skilllite_core::skill::{manifest, metadata};
use skilllite_core::skill::trust::TrustTier;

use crate::Result;

fn resolve_workspace_root(workspace: &str) -> PathBuf {
    let raw = workspace.trim();
    let p = if raw.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(raw)
    };
    if p.is_absolute() {
        p
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}

/// Matches `skilllite-assistant` `DesktopSkillInfo` (camelCase JSON).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSkillSnapshot {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub skill_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub trust_tier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admission_risk: Option<String>,
    pub dependency_type: String,
    pub dependency_packages: Vec<String>,
    pub missing_commands: Vec<String>,
    pub missing_any_command_groups: Vec<Vec<String>>,
    pub missing_env_vars: Vec<String>,
}

pub fn list_desktop_skills(workspace: &str) -> Vec<DesktopSkillSnapshot> {
    let root = resolve_workspace_root(workspace);
    let mut manifest_cache = HashMap::new();
    let mut skills = Vec::new();
    for skill in discover_skill_instances_in_workspace(&root, None) {
        skills.push(build_desktop_skill_snapshot(
            &skill.path,
            &skill.name,
            &mut manifest_cache,
        ));
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// `skilllite skills list`
pub fn cmd_list_desktop(workspace: &str, json_output: bool) -> Result<()> {
    let rows = list_desktop_skills(workspace);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        eprintln!("(no skills found in workspace {})", workspace);
    } else {
        eprintln!("Skills in workspace {} ({}):", workspace, rows.len());
        for row in rows {
            eprintln!(
                "  • {} [{}] trust={} deps={}",
                row.name, row.skill_type, row.trust_tier, row.dependency_type
            );
        }
    }
    Ok(())
}

fn build_desktop_skill_snapshot(
    skill_path: &Path,
    fallback_name: &str,
    manifest_cache: &mut HashMap<PathBuf, manifest::SkillManifest>,
) -> DesktopSkillSnapshot {
    let manifest_entry = manifest_entry_for_skill(skill_path, manifest_cache);
    let parsed = metadata::parse_skill_metadata(skill_path).ok();
    let name = parsed
        .as_ref()
        .map(|meta| meta.name.clone())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| fallback_name.to_string());
    let description = parsed.as_ref().and_then(|meta| meta.description.clone());
    let skill_type = parsed
        .as_ref()
        .map(|meta| desktop_skill_type(skill_path, meta))
        .unwrap_or_else(|| {
            if metadata::has_executable_scripts(skill_path) {
                "script".to_string()
            } else {
                "prompt_only".to_string()
            }
        });
    let dependency = parsed
        .as_ref()
        .and_then(|meta| deps::detect_dependencies(skill_path, meta).ok());
    let (missing_commands, missing_any_command_groups, missing_env_vars) = parsed
        .as_ref()
        .map(detect_missing_requirements)
        .unwrap_or_default();

    DesktopSkillSnapshot {
        name,
        description,
        skill_type,
        source: manifest_entry.as_ref().map(|entry| entry.source.clone()),
        trust_tier: manifest_entry
            .as_ref()
            .map(|entry| trust_tier_label(entry.trust_tier.clone()))
            .unwrap_or_else(|| "unknown".to_string()),
        trust_score: manifest_entry.as_ref().map(|entry| entry.trust_score),
        admission_risk: manifest_entry
            .as_ref()
            .and_then(|entry| entry.admission_risk.clone()),
        dependency_type: dependency_type_label(dependency.as_ref().map(|dep| &dep.dep_type)),
        dependency_packages: dependency
            .as_ref()
            .map(|dep| dep.packages.clone())
            .unwrap_or_default(),
        missing_commands,
        missing_any_command_groups,
        missing_env_vars,
    }
}

fn manifest_entry_for_skill(
    skill_path: &Path,
    manifest_cache: &mut HashMap<PathBuf, manifest::SkillManifest>,
) -> Option<manifest::SkillManifestEntry> {
    let parent = skill_path.parent()?.to_path_buf();
    if !manifest_cache.contains_key(&parent) {
        manifest_cache.insert(
            parent.clone(),
            manifest::load_manifest(&parent).unwrap_or_default(),
        );
    }
    let skill_key = skill_path.file_name()?.to_str()?;
    manifest_cache
        .get(&parent)
        .and_then(|loaded| loaded.skills.get(skill_key).cloned())
}

fn desktop_skill_type(skill_path: &Path, meta: &metadata::SkillMetadata) -> String {
    if !meta.entry_point.is_empty() || metadata::has_executable_scripts(skill_path) {
        "script".to_string()
    } else if meta.is_bash_tool_skill() {
        "bash_tool".to_string()
    } else {
        "prompt_only".to_string()
    }
}

fn trust_tier_label(tier: TrustTier) -> String {
    match tier {
        TrustTier::Trusted => "trusted",
        TrustTier::Reviewed => "reviewed",
        TrustTier::Community => "community",
        TrustTier::Unknown => "unknown",
    }
    .to_string()
}

fn dependency_type_label(dep_type: Option<&deps::DependencyType>) -> String {
    match dep_type {
        Some(deps::DependencyType::Python) => "python",
        Some(deps::DependencyType::Node) => "node",
        _ => "none",
    }
    .to_string()
}

fn detect_missing_requirements(
    meta: &metadata::SkillMetadata,
) -> (Vec<String>, Vec<Vec<String>>, Vec<String>) {
    let mut missing_commands = BTreeSet::new();
    let mut missing_env_vars = BTreeSet::new();
    let mut missing_any_command_groups = Vec::new();

    for pattern in meta.get_bash_patterns() {
        if !command_exists(&pattern.command_prefix) {
            missing_commands.insert(pattern.command_prefix.clone());
        }
    }

    let compatibility = meta.compatibility.as_deref();
    for command in extract_compat_values(compatibility, "Requires bins: ") {
        if !command_exists(&command) {
            missing_commands.insert(command);
        }
    }
    let any_group = extract_compat_values(compatibility, "Requires at least one bin: ");
    if !any_group.is_empty() && !any_group.iter().any(|command| command_exists(command)) {
        missing_any_command_groups.push(any_group);
    }
    for env_key in extract_compat_values(compatibility, "Requires env: ") {
        if std::env::var_os(&env_key).is_none() {
            missing_env_vars.insert(env_key);
        }
    }
    for env_key in extract_compat_values(compatibility, "Primary credential env: ") {
        if std::env::var_os(&env_key).is_none() {
            missing_env_vars.insert(env_key);
        }
    }

    (
        missing_commands.into_iter().collect(),
        missing_any_command_groups,
        missing_env_vars.into_iter().collect(),
    )
}

fn extract_compat_values(compatibility: Option<&str>, prefix: &str) -> Vec<String> {
    let Some(compatibility) = compatibility else {
        return Vec::new();
    };
    compatibility
        .split('.')
        .map(str::trim)
        .filter_map(|segment| segment.strip_prefix(prefix))
        .flat_map(|rest| rest.split(','))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(std::string::ToString::to_string)
        .collect()
}

fn command_exists(command: &str) -> bool {
    let candidate = Path::new(command);
    if candidate.components().count() > 1 {
        return candidate.is_file();
    }

    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };

    #[cfg(windows)]
    let path_exts: Vec<String> = std::env::var_os("PATHEXT")
        .map(|raw| {
            raw.to_string_lossy()
                .split(';')
                .filter(|item| !item.is_empty())
                .map(|item| item.trim_matches('.').to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_else(|| vec!["exe".to_string(), "cmd".to_string(), "bat".to_string()]);

    for dir in std::env::split_paths(&paths) {
        let direct = dir.join(command);
        if direct.is_file() {
            return true;
        }
        #[cfg(windows)]
        for ext in &path_exts {
            let with_ext = dir.join(format!("{command}.{ext}"));
            if with_ext.is_file() {
                return true;
            }
        }
    }
    false
}
