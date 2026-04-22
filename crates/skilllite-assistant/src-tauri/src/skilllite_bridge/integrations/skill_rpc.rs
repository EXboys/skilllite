//! 工作区技能发现与 **`skilllite` CLI** 子进程桥（`add` / `repair-skills` 等）。
//!
//! 与 [`crate::skilllite_bridge::protocol`] 中的 **agent-rpc JSON-lines 流** 分离：本模块只处理
//! 可执行 CLI 与工作区目录，不承担 stdout 行协议解析。契约测试见同目录各子模块 `#[cfg(test)]`。

use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};
use serde::Serialize;
use skilllite_core::skill::{deps, manifest, metadata, trust::TrustTier};
use std::fs;
use std::path::{Path, PathBuf};

use super::shared::{discover_skill_instances, find_skill_dir, resolve_workspace_skills_root};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSkillInfo {
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

/// List desktop skill infos in workspace using core-owned discovery.
pub fn list_skills(workspace: &str) -> Vec<DesktopSkillInfo> {
    let root = find_project_root(workspace);
    let mut manifest_cache = std::collections::HashMap::new();
    let mut skills = Vec::new();
    for (path, name) in discover_skill_instances(&root) {
        skills.push(build_desktop_skill_info(&path, &name, &mut manifest_cache));
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn build_desktop_skill_info(
    skill_path: &Path,
    fallback_name: &str,
    manifest_cache: &mut std::collections::HashMap<PathBuf, manifest::SkillManifest>,
) -> DesktopSkillInfo {
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

    DesktopSkillInfo {
        name,
        description,
        skill_type,
        source: manifest_entry.as_ref().map(|entry| entry.source.clone()),
        trust_tier: manifest_entry
            .as_ref()
            .map(|entry| trust_tier_label(entry.trust_tier.clone()))
            .unwrap_or_else(|| "unknown".to_string()),
        trust_score: manifest_entry.as_ref().map(|entry| entry.trust_score),
        admission_risk: manifest_entry.as_ref().and_then(|entry| entry.admission_risk.clone()),
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
    manifest_cache: &mut std::collections::HashMap<PathBuf, manifest::SkillManifest>,
) -> Option<manifest::SkillManifestEntry> {
    let parent = skill_path.parent()?.to_path_buf();
    if !manifest_cache.contains_key(&parent) {
        manifest_cache.insert(parent.clone(), manifest::load_manifest(&parent).unwrap_or_default());
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
    let mut missing_commands = std::collections::BTreeSet::new();
    let mut missing_env_vars = std::collections::BTreeSet::new();
    let mut missing_any_command_groups = Vec::new();

    for pattern in meta.get_bash_patterns() {
        if !command_exists(&pattern.command_prefix) {
            missing_commands.insert(pattern.command_prefix);
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
            let with_ext = dir.join(format!("{command}.{}", ext));
            if with_ext.is_file() {
                return true;
            }
        }
    }
    false
}

/// Open the given skill's directory in the system file manager.
pub fn open_skill_directory(workspace: &str, skill_name: &str) -> Result<(), String> {
    let path = find_skill_dir(workspace, skill_name)
        .ok_or_else(|| format!("未找到技能目录: {}", skill_name))?;
    if !path.exists() || !path.is_dir() {
        return Err(format!("技能目录不存在: {}", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove installed skills under the workspace (same discovery as list/open).
/// Updates `.skilllite-manifest.json` when present. `skill_names` must be non-empty.
pub fn remove_skills(workspace: &str, skill_names: &[String]) -> Result<String, String> {
    if skill_names.is_empty() {
        return Err("请至少勾选一个要删除的技能".to_string());
    }
    let mut lines: Vec<String> = Vec::new();
    let mut deleted = 0usize;
    for name in skill_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(skill_path) = find_skill_dir(workspace, name) else {
            lines.push(format!("未找到技能，已跳过: {}", name));
            continue;
        };
        let skills_parent = skill_path
            .parent()
            .ok_or_else(|| format!("无效技能路径: {}", skill_path.display()))?;
        manifest::remove_skill_entry(skills_parent, &skill_path).map_err(|e| e.to_string())?;
        fs::remove_dir_all(&skill_path)
            .map_err(|e| format!("删除目录失败 {}: {}", skill_path.display(), e))?;
        deleted += 1;
        lines.push(format!("已删除: {}", name));
    }
    if deleted == 0 {
        return Err(if lines.is_empty() {
            "没有可删除的技能".to_string()
        } else {
            lines.join("\n")
        });
    }
    Ok(lines.join("\n"))
}

/// Run `skilllite evolution repair-skills [skill_names...]`. If skill_names is empty, repairs all failed; otherwise only those.
pub fn repair_skills(
    workspace: &str,
    skill_names: &[String],
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);

    let mut cmd = std::process::Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.arg("evolution").arg("repair-skills");
    for name in skill_names {
        cmd.arg(name);
    }
    cmd.arg("--from-source");
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 repair-skills 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(combined)
}

/// Run `skilllite add <source>` in the workspace using the canonical resolved skills dir.
/// Source: owner/repo, owner/repo@skill-name, https://github.com/..., or local path.
pub fn add_skill(
    workspace: &str,
    source: &str,
    force: bool,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);
    let skills_root = resolve_workspace_skills_root(workspace);
    let source = source.trim();
    if source.is_empty() {
        return Err("请填写来源，例如：owner/repo 或 owner/repo@skill-name".to_string());
    }

    let mut cmd = std::process::Command::new(skilllite_path);
    crate::windows_spawn::hide_child_console(&mut cmd);
    cmd.arg("add")
        .arg(source)
        .arg("--skills-dir")
        .arg(&skills_root);
    if force {
        cmd.arg("--force");
    }
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite add 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    let added_skill_names = extract_added_skill_names(&combined);
    let all_skills = list_skills(workspace);
    Ok(summarise_add_output(
        &combined,
        &all_skills,
        &added_skill_names,
    ))
}

/// 从 skilllite add 的完整输出中提取简短摘要，避免在桌面端刷屏。
fn summarise_add_output(
    output: &str,
    all_skills: &[DesktopSkillInfo],
    added_skill_names: &[String],
) -> String {
    let base = summarise_add_output_base(output);
    let hints = build_setup_hints(all_skills, added_skill_names);
    if hints.is_empty() {
        base
    } else {
        format!("{base}\n{}", hints.join("\n"))
    }
}

fn summarise_add_output_base(output: &str) -> String {
    if output.is_empty() {
        return "已添加".to_string();
    }
    // 匹配 "🎉 Successfully added 14 skill(s) from obra/superpowers" 或 "Successfully added 1 skill(s)"
    let line = output
        .lines()
        .find(|line| line.contains("Successfully added") && line.contains("skill(s)"));
    if let Some(line) = line {
        let line = line.trim().trim_start_matches("🎉 ").trim();
        if let Some(after) = line.strip_prefix("Successfully added ") {
            let num_str = after.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<u32>() {
                let from = after.split(" from ").nth(1).map(str::trim);
                return if let Some(src) = from {
                    format!("已添加 {} 个技能（来自 {}）", n, src)
                } else {
                    format!("已添加 {} 个技能", n)
                };
            }
        }
    }
    "已添加".to_string()
}

fn extract_added_skill_names(output: &str) -> Vec<String> {
    let mut after_summary = false;
    let mut names = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("Successfully added") && trimmed.contains("skill(s)") {
            after_summary = true;
            continue;
        }
        if !after_summary {
            continue;
        }
        if let Some(name) = trimmed.strip_prefix('•') {
            let name = name.trim();
            if !name.is_empty() {
                names.push(name.to_string());
            }
            continue;
        }
        if !names.is_empty() && trimmed.starts_with('=') {
            break;
        }
    }
    names
}

fn build_setup_hints(all_skills: &[DesktopSkillInfo], added_skill_names: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    for name in added_skill_names {
        let Some(skill) = all_skills.iter().find(|skill| skill.name == *name) else {
            continue;
        };
        let mut missing = Vec::new();
        if !skill.missing_commands.is_empty() {
            missing.push(format!("缺少命令 {}", skill.missing_commands.join(", ")));
        }
        for group in &skill.missing_any_command_groups {
            missing.push(format!("需要至少一个命令 {}", group.join(" / ")));
        }
        if !skill.missing_env_vars.is_empty() {
            missing.push(format!("缺少环境变量 {}", skill.missing_env_vars.join(", ")));
        }
        if !missing.is_empty() {
            lines.push(format!("- {}：{}", skill.name, missing.join("；")));
        }
    }
    if lines.is_empty() {
        lines
    } else {
        let mut out = vec!["安装后仍需补齐：".to_string()];
        out.extend(lines);
        out
    }
}

#[cfg(test)]
mod skill_discovery_tests {
    use super::super::shared::resolve_workspace_skills_root;
    use super::*;
    use std::path::PathBuf;

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "skilllite_assistant_{}_{}_{}",
            prefix,
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn list_skill_names_uses_core_discovery_roots() {
        let tmp = temp_test_dir("skill_roots");
        let nested_skill = tmp.join(".claude").join("skills").join("nested-skill");
        std::fs::create_dir_all(nested_skill.join("scripts")).expect("nested scripts");
        std::fs::write(nested_skill.join("SKILL.md"), "name: nested-skill\n").expect("nested md");
        std::fs::write(
            nested_skill.join("scripts").join("run.sh"),
            "#!/usr/bin/env bash\necho ok\n",
        )
        .expect("nested script");

        let evolved_skill = tmp.join(".skills").join("_evolved").join("evolved-skill");
        std::fs::create_dir_all(evolved_skill.join("scripts")).expect("evolved scripts");
        std::fs::write(evolved_skill.join("SKILL.md"), "name: evolved-skill\n")
            .expect("evolved md");
        std::fs::write(
            evolved_skill.join("scripts").join("run.py"),
            "print('ok')\n",
        )
        .expect("evolved script");

        let skills = list_skills(nested_skill.to_string_lossy().as_ref());
        let names: Vec<String> = skills.into_iter().map(|skill| skill.name).collect();
        assert_eq!(names, vec!["evolved-skill", "nested-skill"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn list_skill_names_includes_non_script_skills() {
        let tmp = temp_test_dir("non_script_skill");
        let bash_tool = tmp.join(".skills").join("web-search");
        std::fs::create_dir_all(&bash_tool).expect("bash tool dir");
        std::fs::write(
            bash_tool.join("SKILL.md"),
            "---\nname: web-search\nallowed-tools: Bash(infsh *)\n---\n",
        )
        .expect("bash tool skill md");

        let skills = list_skills(tmp.to_string_lossy().as_ref());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "web-search");
        assert_eq!(skills[0].skill_type, "bash_tool");
        assert_eq!(skills[0].dependency_type, "node");
        assert_eq!(skills[0].dependency_packages, vec!["infsh".to_string()]);
        assert_eq!(skills[0].missing_commands, vec!["infsh".to_string()]);
        let resolved = find_skill_dir(tmp.to_string_lossy().as_ref(), "web-search")
            .expect("find non-script skill");
        assert_eq!(
            resolved.canonicalize().expect("resolved canonical"),
            bash_tool.canonicalize().expect("bash tool canonical")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn summarise_add_output_appends_setup_hints() {
        let output = "🎉 Successfully added 1 skill(s) from /tmp/demo.zip\n  • web-search\n==================================================";
        let all_skills = vec![DesktopSkillInfo {
            name: "web-search".to_string(),
            description: None,
            skill_type: "bash_tool".to_string(),
            source: Some("/tmp/demo.zip".to_string()),
            trust_tier: "community".to_string(),
            trust_score: Some(63),
            admission_risk: Some("safe".to_string()),
            dependency_type: "node".to_string(),
            dependency_packages: vec!["infsh".to_string()],
            missing_commands: vec!["infsh".to_string()],
            missing_any_command_groups: Vec::new(),
            missing_env_vars: vec!["TAVILY_API_KEY".to_string()],
        }];
        let summary = summarise_add_output(output, &all_skills, &extract_added_skill_names(output));
        assert!(summary.contains("已添加 1 个技能"));
        assert!(summary.contains("缺少命令 infsh"));
        assert!(summary.contains("缺少环境变量 TAVILY_API_KEY"));
    }

    #[test]
    fn resolve_workspace_skills_root_keeps_legacy_fallback() {
        let tmp = temp_test_dir("legacy_fallback");
        let legacy = tmp.join(".skills");
        std::fs::create_dir_all(&legacy).expect("legacy root");

        let resolved = resolve_workspace_skills_root(tmp.to_string_lossy().as_ref());
        assert_eq!(
            resolved.canonicalize().expect("resolved canonical"),
            legacy.canonicalize().expect("legacy canonical")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
