//! 工作区技能发现与 **`skilllite` CLI** 子进程桥（`add` / `repair-skills` 等）。
//!
//! 与 [`crate::skilllite_bridge::protocol`] 中的 **agent-rpc JSON-lines 流** 分离：本模块只处理
//! 可执行 CLI 与工作区目录，不承担 stdout 行协议解析。契约测试见同目录各子模块 `#[cfg(test)]`。

use crate::skilllite_bridge::local::env_keys::paths as env_paths;
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;

use super::shared::{find_skill_dir, resolve_workspace_skills_root};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// List desktop skill infos in workspace (`skilllite skills list --json` only).
pub fn list_skills(workspace: &str, skilllite_path: &Path) -> Result<Vec<DesktopSkillInfo>, String> {
    spawn_skilllite_json(
        skilllite_path,
        workspace,
        None,
        &["skills", "list", "--json", "--workspace", workspace],
    )
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

/// Remove installed skills via `skilllite remove --force`.
pub fn remove_skills(
    workspace: &str,
    skill_names: &[String],
    skilllite_path: &Path,
) -> Result<String, String> {
    if skill_names.is_empty() {
        return Err("请至少勾选一个要删除的技能".to_string());
    }
    let root = find_project_root(workspace);
    let skills_root = resolve_workspace_skills_root(workspace);
    let skills_dir = skills_root.to_string_lossy();
    let mut lines: Vec<String> = Vec::new();
    let mut deleted = 0usize;
    for name in skill_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let mut cmd = std::process::Command::new(skilllite_path);
        crate::windows_spawn::hide_child_console(&mut cmd);
        cmd.arg("remove")
            .arg(name)
            .arg("--skills-dir")
            .arg(skills_dir.as_ref())
            .arg("--force")
            .current_dir(&root)
            .env(
                env_paths::SKILLLITE_WORKSPACE,
                root.to_string_lossy().as_ref(),
            )
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in load_dotenv_for_child(workspace) {
            cmd.env(k, v);
        }
        let output = cmd
            .output()
            .map_err(|e| format!("执行 skilllite remove 失败: {}", e))?;
        if output.status.success() {
            deleted += 1;
            lines.push(format!("已删除: {}", name));
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            lines.push(format!("删除失败 {}: {}", name, err.trim()));
        }
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
        .env(
            env_paths::SKILLLITE_WORKSPACE,
            root.to_string_lossy().as_ref(),
        )
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
        .env(
            env_paths::SKILLLITE_WORKSPACE,
            root.to_string_lossy().as_ref(),
        )
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
    let all_skills = list_skills(workspace, skilllite_path).unwrap_or_default();
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
            missing.push(format!(
                "缺少环境变量 {}",
                skill.missing_env_vars.join(", ")
            ));
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
