//! 用大模型根据 SKILL.md 推理入口脚本，兼容未显式写 entry_point 的 skill。

use std::path::Path;

use crate::Result;
use skilllite_evolution::{EvolutionLlm, EvolutionMessage};

/// 列出 skill 目录下 scripts/ 中可执行脚本路径（相对 skill_dir）。
fn list_script_paths(skill_dir: &Path) -> Vec<String> {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() || !scripts_dir.is_dir() {
        return Vec::new();
    }
    let mut paths = Vec::new();
    let Ok(entries) = std::fs::read_dir(&scripts_dir) else {
        return paths;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str());
        if !matches!(ext, Some("py") | Some("js") | Some("ts") | Some("sh")) {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with("test_")
            || name.ends_with("_test.py")
            || name == "__init__.py"
            || name.starts_with('.')
        {
            continue;
        }
        paths.push(format!("scripts/{}", name));
    }
    paths.sort();
    paths
}

/// 用大模型根据 SKILL.md 正文和脚本列表推理入口路径。
/// 返回的路径一定在 script_paths 中且对应文件存在；否则 None。
pub async fn infer_entry_point_from_skill_md<L: EvolutionLlm>(
    skill_dir: &Path,
    llm: &L,
    model: &str,
) -> Result<Option<String>> {
    let skill_md_path = skill_dir.join("SKILL.md");
    let content = match std::fs::read_to_string(&skill_md_path) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let script_paths = list_script_paths(skill_dir);
    if script_paths.is_empty() {
        return Ok(None);
    }

    let paths_list = script_paths.join("\n");
    let prompt = format!(
        r#"根据下面这份 Skill 的 SKILL.md 文档和脚本列表，推断**主入口脚本**是哪一个（即用户调用该 skill 时应该执行的脚本）。
只回复一个路径，不要解释。若无法推断或文档未说明，回复 NONE。

## 脚本列表
{}

## SKILL.md
{}
"#,
        paths_list, content
    );

    let messages = vec![EvolutionMessage::user(&prompt)];
    let response = llm
        .complete(&messages, model, 0.0)
        .await?
        .trim()
        .to_string();
    let response_upper = response.to_uppercase();
    if response_upper == "NONE" || response.is_empty() {
        return Ok(None);
    }

    // 允许回复带反引号或换行，取首行并去掉反引号
    let path = response
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('`')
        .trim();
    if path.is_empty() {
        return Ok(None);
    }

    if script_paths.iter().any(|p| p == path) && skill_dir.join(path).is_file() {
        return Ok(Some(path.to_string()));
    }
    Ok(None)
}
