//! 从 SKILL.md 推理 entry_point 和 test_input，以及脚本执行

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::bail;
use crate::Result;

use skilllite_sandbox::common::hide_child_console;
use skilllite_sandbox::env::builder;

use super::SkillMeta;
use super::MAX_PARSE_RETRIES;
use super::SKILL_EXECUTION_INFERENCE_PROMPT;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

/// 从文本中提取第一个平衡的 JSON 对象 {...}
pub(super) fn extract_first_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let rest = &text[start..];
    let mut depth = 0u32;
    let mut in_str = false;
    let mut quote = b'"';
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if b == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if b == quote {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' => {
                in_str = true;
                quote = b;
                i += 1;
            }
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                if depth == 1 {
                    return Some(&rest[..=i]);
                }
                depth = depth.saturating_sub(1);
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// List executable scripts: scripts/ 下任意 .py .js .ts，以及 skill 根目录下的可执行文件。
pub(super) fn list_scripts(skill_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for root in [skill_dir.to_path_buf(), skill_dir.join("scripts")] {
        if !root.is_dir() {
            continue;
        }
        for e in std::fs::read_dir(&root)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
        {
            let p = e.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    let ext = ext.to_string_lossy();
                    if ext == "py" || ext == "js" || ext == "ts" || ext == "sh" {
                        if let Ok(rel) = p.strip_prefix(skill_dir) {
                            out.push(rel.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub(super) fn list_existing_skill_names(skills_root: &Path) -> String {
    let evolved_dir = skills_root.join("_evolved");
    if !evolved_dir.exists() {
        return "(无已有 Skill)".to_string();
    }

    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&evolved_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_pending" && path.is_dir() {
                for e in std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                {
                    if e.path().is_dir() && e.path().join("SKILL.md").exists() {
                        names.push(format!("- {}", e.file_name().to_string_lossy()));
                    }
                }
            }
            continue;
        }
        if path.is_dir() && path.join("SKILL.md").exists() {
            names.push(format!("- {}", name));
        }
    }

    if names.is_empty() {
        "(无已有 Skill)".to_string()
    } else {
        names.join("\n")
    }
}

pub(super) fn count_active_evolved_skills(evolved_dir: &Path) -> usize {
    if !evolved_dir.exists() {
        return 0;
    }
    let mut count = 0;
    for entry in std::fs::read_dir(evolved_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_pending" && path.is_dir() {
                count += std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().join(".meta.json").exists())
                    .count();
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        if skilllite_fs::read_file(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str::<SkillMeta>(&s).ok())
            .map(|m| !m.archived)
            .unwrap_or(true)
        {
            count += 1;
        }
    }
    count
}

pub(super) fn extract_description_from_skill_md(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("---") {
            return trimmed.to_string();
        }
    }
    String::new()
}

/// 列出 _pending 下已有 skill 的 (name, description)，用于同轮去重。
pub(super) fn list_pending_skill_descriptions(pending_dir: &Path) -> Vec<(String, String)> {
    if !pending_dir.exists() || !pending_dir.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(pending_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let skill_md = path.join("SKILL.md");
        let desc = skilllite_fs::read_file(&skill_md)
            .ok()
            .map(|c| extract_description_from_skill_md(&c))
            .unwrap_or_default();
        out.push((name, desc));
    }
    out
}

/// 轻量描述相似度：用于同轮内避免明显重复。当任一描述包含另一（归一化后）或完全相同时返回 true。
fn normalize_desc(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn is_description_similar(a: &str, b: &str) -> bool {
    let na = normalize_desc(a);
    let nb = normalize_desc(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    na == nb || na.contains(&nb) || nb.contains(&na)
}

/// 通过大模型从 SKILL.md 推理入口脚本和测试输入。
pub(super) async fn infer_skill_execution<L: EvolutionLlm>(
    llm: &L,
    model: &str,
    skill_dir: &Path,
) -> Result<(String, String)> {
    let skill_md_path = skill_dir.join("SKILL.md");
    let skill_md = skilllite_fs::read_file(&skill_md_path).unwrap_or_else(|_| "".to_string());
    let scripts = list_scripts(skill_dir);
    if scripts.is_empty() {
        bail!("无 scripts 或可执行脚本，跳过（如 agent-browser 等 CLI 文档型 skill）");
    }
    let scripts_list = scripts.join(", ");

    let prompt = format!(
        r#"## SKILL.md

{}

## scripts/ 目录下的可执行文件

{}

## 任务

1. **entry_point**：必须从上面「可执行文件」列表中精确选一项，不能编造不存在的路径。
2. **test_input**：根据 Examples/Input Schema/Usage 推理最小可用 JSON，若无示例则用 `{{}}`。

只返回 JSON，不要 markdown 包裹：
{{"entry_point": "<从上面列表选一项>", "test_input": {{}}}}"#,
        skill_md, scripts_list
    );

    let messages = vec![
        EvolutionMessage::system(SKILL_EXECUTION_INFERENCE_PROMPT),
        EvolutionMessage::user(&prompt),
    ];
    let content = llm.complete(&messages, model, 0.0).await?;
    let trimmed = content.trim();

    #[derive(serde::Deserialize)]
    struct InferResult {
        entry_point: String,
        test_input: serde_json::Value,
    }

    fn try_parse_infer(trimmed: &str) -> Option<InferResult> {
        let trimmed = super::parse::strip_think_blocks(trimmed);
        let json_str = trimmed
            .strip_prefix("```json")
            .or_else(|| trimmed.strip_prefix("```"))
            .and_then(|s| s.strip_suffix("```"))
            .map(|s| s.trim())
            .or_else(|| extract_first_json_object(trimmed));
        json_str.and_then(|s| serde_json::from_str(s).ok())
    }

    let mut parsed = try_parse_infer(trimmed);
    if parsed.is_none() && MAX_PARSE_RETRIES > 0 {
        let err_hint = "未找到合法 JSON 或 JSON 格式错误";
        tracing::info!(
            "infer_skill_execution JSON parse failed, retrying with LLM feedback: {}",
            err_hint
        );
        let retry_msg = format!(
            "你的输出无法解析为 JSON。请重新输出，严格遵循格式: {{\"entry_point\": \"从列表 [{}] 中选一项\", \"test_input\": {{}}}}。只返回 JSON，不要 markdown 包裹。",
            scripts_list
        );
        let mut msgs = messages.to_vec();
        msgs.push(EvolutionMessage::user(&retry_msg));
        let content2 = llm.complete(&msgs, model, 0.0).await?;
        parsed = try_parse_infer(content2.trim());
    }

    let parsed: InferResult = match parsed {
        Some(p) => p,
        None => {
            let fallback = scripts
                .first()
                .cloned()
                .unwrap_or_else(|| "scripts/main.py".to_string());
            let full = skill_dir.join(&fallback);
            if full.exists() {
                return Ok((fallback, "{}".to_string()));
            }
            bail!(
                "LLM inference parse failed (raw: {}...). No valid scripts for fallback.",
                trimmed.chars().take(100).collect::<String>()
            );
        }
    };

    let mut entry = parsed.entry_point.trim().to_string();
    if !scripts.contains(&entry) {
        tracing::warn!(
            "LLM 返回 entry_point '{}' 不在列表 [{}] 中，改用第一项",
            entry,
            scripts_list
        );
        entry = scripts.first().cloned().unwrap_or_default();
        if entry.is_empty() {
            bail!("无有效脚本");
        }
    }
    let test_json = if parsed.test_input.is_object() {
        serde_json::to_string(&parsed.test_input).unwrap_or_else(|_| "{}".to_string())
    } else {
        "{}".to_string()
    };

    let full_path = skill_dir.join(&entry);
    if !full_path.exists() {
        bail!("LLM inferred entry_point '{}' does not exist", entry);
    }

    Ok((entry, test_json))
}

/// Run skill with inferred entry_point and test_input. Returns (success, error_trace).
/// When `env_path` is Some, uses the skill's isolated env (venv/node_modules) so dependencies are available.
pub(super) fn test_skill_invoke(
    skill_dir: &Path,
    entry_point: &str,
    test_input: &str,
    env_path: Option<&Path>,
) -> Result<(bool, String)> {
    let script_path = skill_dir.join(entry_point);
    if !script_path.exists() {
        return Ok((false, "no entry script".to_string()));
    }

    let runtime = env_path
        .filter(|p| !p.as_os_str().is_empty() && p.exists())
        .map(builder::build_runtime_paths);

    let mut run_cmd = if entry_point.ends_with(".py") {
        let interpreter = runtime
            .as_ref()
            .map(|r| r.python.to_string_lossy().into_owned())
            .unwrap_or_else(|| "python3".to_string());
        Command::new(&interpreter)
    } else if entry_point.ends_with(".js") {
        let node = runtime
            .as_ref()
            .map(|r| r.node.to_string_lossy().into_owned())
            .unwrap_or_else(|| "node".to_string());
        let mut c = Command::new(&node);
        if let Some(ref r) = runtime {
            if let Some(ref nm) = r.node_modules {
                c.env("NODE_PATH", nm);
            }
        }
        c
    } else {
        return Ok((false, "unsupported entry point".to_string()));
    };

    hide_child_console(&mut run_cmd);

    let script_arg = script_path.to_string_lossy().into_owned();
    let mut child = run_cmd
        .arg(&script_arg)
        .current_dir(skill_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::Error::validation(format!("run failed: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(test_input.as_bytes());
    }
    let output = child
        .wait_with_output()
        .map_err(|e| crate::Error::validation(format!("wait failed: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let trace = format!(
            "exit_code={}\nstdout:\n{}\nstderr:\n{}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );
        // CLI/argparse scripts often require positional args; stdin JSON probing is not suitable.
        // Retry once with `--help` and treat a successful help response as a validation pass.
        let lower_trace = trace.to_lowercase();
        let looks_like_cli_usage = lower_trace.contains("usage:")
            || lower_trace.contains("the following arguments are required");
        if looks_like_cli_usage {
            let mut help_cmd = if entry_point.ends_with(".py") {
                let interpreter = runtime
                    .as_ref()
                    .map(|r| r.python.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "python3".to_string());
                Command::new(&interpreter)
            } else if entry_point.ends_with(".js") {
                let node = runtime
                    .as_ref()
                    .map(|r| r.node.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "node".to_string());
                let mut c = Command::new(&node);
                if let Some(ref r) = runtime {
                    if let Some(ref nm) = r.node_modules {
                        c.env("NODE_PATH", nm);
                    }
                }
                c
            } else {
                return Ok((false, trace));
            };
            hide_child_console(&mut help_cmd);
            let help_output = help_cmd
                .arg(&script_arg)
                .arg("--help")
                .current_dir(skill_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| crate::Error::validation(format!("help probe failed: {}", e)))?;
            if help_output.status.success() {
                return Ok((true, String::new()));
            }
        }
        return Ok((false, trace));
    }

    if stdout.is_empty() {
        return Ok((false, "no output".to_string()));
    }

    if serde_json::from_str::<serde_json::Value>(&stdout).is_ok() {
        return Ok((true, String::new()));
    }

    // stdout 整体不是合法 JSON，尝试从最后一行向前找第一个合法 JSON 行
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
            return Ok((true, String::new()));
        }
    }

    Ok((false, format!("output not valid JSON:\n{}", stdout)))
}
