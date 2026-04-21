//! 工作区技能根路径与脚本技能发现（供 CLI 桥与进化 UI 共用）。

use crate::skilllite_bridge::paths::find_project_root;
use skilllite_core::skill::discovery::discover_skill_instances_in_workspace;
use skilllite_services::WorkspaceService;
use std::path::PathBuf;

/// Whether the skill dir has any script file (scripts/ or root with common script extensions).
fn skill_has_scripts(path: &std::path::Path) -> bool {
    let scripts_dir = path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            if entries
                .filter(|e| e.as_ref().ok().map(|e| e.path().is_file()).unwrap_or(false))
                .count()
                > 0
            {
                return true;
            }
        }
    }
    const EXTS: &[&str] = &["py", "js", "ts", "sh", "bash"];
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if EXTS.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

pub(crate) fn discover_scripted_skill_instances(root: &std::path::Path) -> Vec<(PathBuf, String)> {
    discover_skill_instances_in_workspace(root, None)
        .into_iter()
        .filter(|skill| skill_has_scripts(&skill.path))
        .map(|skill| (skill.path, skill.name))
        .collect()
}

pub(crate) fn resolve_workspace_skills_root(workspace: &str) -> PathBuf {
    let root = find_project_root(workspace);
    // Phase 1A (TASK-2026-044) preserves the previous "silently drop conflict
    // warning" Desktop behaviour to avoid changing UI surface in this refactor.
    // A follow-up TASK can route `response.conflicting_skill_names` /
    // `conflict_warning` into a structured assistant notification channel.
    let _ = workspace; // intentionally unused after find_project_root; kept for arg shape parity
    match WorkspaceService::new().resolve_skills_dir_for_workspace(&root, "skills") {
        Ok(response) => response.effective_path,
        Err(_) => {
            skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback(
                &root, "skills",
            )
            .effective_path
        }
    }
}

pub(crate) fn existing_workspace_skills_root(workspace: &str) -> Option<PathBuf> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skills_root.is_dir().then_some(skills_root)
}

/// Resolve skill directory path by name using core-owned discovery.
pub(crate) fn find_skill_dir(workspace: &str, skill_name: &str) -> Option<std::path::PathBuf> {
    let root = find_project_root(workspace);
    for (path, name) in discover_scripted_skill_instances(&root) {
        if name == skill_name {
            return Some(path);
        }
    }
    None
}
