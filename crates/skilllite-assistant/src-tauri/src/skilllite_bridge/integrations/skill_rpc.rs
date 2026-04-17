//! 工作区技能发现与 **`skilllite` CLI** 子进程桥（`add` / `repair-skills` 等）。
//!
//! 与 [`crate::skilllite_bridge::protocol`] 中的 **agent-rpc JSON-lines 流** 分离：本模块只处理
//! 可执行 CLI 与工作区目录，不承担 stdout 行协议解析。契约测试见同目录各子模块 `#[cfg(test)]`。

use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};
use skilllite_core::skill::manifest;
use std::fs;

use super::shared::{
    discover_scripted_skill_instances, find_skill_dir, resolve_workspace_skills_root,
};

/// List skill names in workspace (for repair UI) using core-owned discovery.
pub fn list_skill_names(workspace: &str) -> Vec<String> {
    let root = find_project_root(workspace);
    let mut names = std::collections::HashSet::new();
    for (_, name) in discover_scripted_skill_instances(&root) {
        names.insert(name);
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
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
    Ok(summarise_add_output(&combined))
}

/// 从 skilllite add 的完整输出中提取简短摘要，避免在桌面端刷屏。
fn summarise_add_output(output: &str) -> String {
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

        let names = list_skill_names(nested_skill.to_string_lossy().as_ref());
        assert_eq!(names, vec!["evolved-skill", "nested-skill"]);
        let _ = std::fs::remove_dir_all(&tmp);
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
