//! 工作区技能根路径与技能发现（供 CLI 桥与进化 UI 共用）。

use crate::skilllite_bridge::paths::find_project_root;
use skilllite_core::skill::discovery::{
    discover_skill_instances_in_workspace, resolve_skills_dir_with_legacy_fallback,
};
use std::path::PathBuf;

pub(crate) fn discover_skill_instances(root: &std::path::Path) -> Vec<(PathBuf, String)> {
    discover_skill_instances_in_workspace(root, None)
        .into_iter()
        .map(|skill| (skill.path, skill.name))
        .collect()
}

pub(crate) fn resolve_workspace_skills_root(workspace: &str) -> PathBuf {
    let root = find_project_root(workspace);
    resolve_skills_dir_with_legacy_fallback(&root, "skills").effective_path
}

pub(crate) fn existing_workspace_skills_root(workspace: &str) -> Option<PathBuf> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skills_root.is_dir().then_some(skills_root)
}

/// Resolve skill directory path by name using core-owned discovery.
pub(crate) fn find_skill_dir(workspace: &str, skill_name: &str) -> Option<std::path::PathBuf> {
    let root = find_project_root(workspace);
    for (path, name) in discover_skill_instances(&root) {
        if name == skill_name {
            return Some(path);
        }
    }
    None
}
