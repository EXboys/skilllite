//! 技能验证：文档完整性检测、技能目录收集、批量验证
//!
//! - **check_skill_md_completeness**: LLM 优先 + 启发式 fallback 检测 SKILL.md 质量
//! - **validate_skills**: 对每个 skill infer → test → doc check，返回验证结果

use std::path::{Path, PathBuf};

use crate::Result;

use crate::EvolutionLlm;
use crate::EvolutionMessage;

use super::infer;

// ─── SKILL.md 文档完整性检测 ─────────────────────────────────────────────────

/// 通过 LLM 检查 SKILL.md 文档完整性，LLM 失败时回退到启发式检测
pub(super) async fn check_skill_md_completeness<L: EvolutionLlm>(
    skill_dir: &Path,
    llm: &L,
    model: &str,
) -> Option<String> {
    let skill_md_path = skill_dir.join("SKILL.md");
    let content = match skilllite_fs::read_file(&skill_md_path) {
        Ok(c) => c,
        Err(_) => return Some("SKILL.md 不存在或无法读取".to_string()),
    };

    let prompt = format!(
        "请判断以下 SKILL.md 是否**同时**包含：\n\
         1. **使用案例**：至少一个完整的调用示例（含具体输入参数值和预期输出）\n\
         2. **参数说明**：所有输入参数的名称、类型和用途\n\n\
         ## SKILL.md\n{}\n\n\
         只返回 JSON，不要 markdown 包裹：\n\
         {{\"complete\": true, \"missing\": \"\"}}\n\
         或\n\
         {{\"complete\": false, \"missing\": \"缺少内容的简述\"}}",
        content,
    );

    let messages = vec![EvolutionMessage::user(&prompt)];
    match llm.complete(&messages, model, 0.0).await {
        Ok(response) => {
            let trimmed = response.trim();
            if let Some(json_str) = infer::extract_first_json_object(trimmed) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let complete = parsed
                        .get("complete")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if complete {
                        return None;
                    }
                    let missing = parsed
                        .get("missing")
                        .and_then(|v| v.as_str())
                        .unwrap_or("使用案例和参数说明");
                    return Some(format!("SKILL.md 文档不完整，缺少: {}", missing));
                }
            }
            if trimmed.contains("\"complete\": true") || trimmed.contains("\"complete\":true") {
                None
            } else {
                Some("SKILL.md 文档不完整（LLM 评估未通过）".to_string())
            }
        }
        Err(e) => {
            tracing::warn!(
                "LLM doc quality check failed: {}, falling back to heuristic",
                e
            );
            check_skill_md_completeness_heuristic(&content)
        }
    }
}

/// 启发式检测 SKILL.md 完整性（LLM 不可用时的 fallback）。
/// 也可在生成阶段对尚未落盘的 skill_md_content 做写入前校验。
pub(super) fn check_skill_md_completeness_heuristic(content: &str) -> Option<String> {
    let has_examples = has_section_with_content(content, &["example", "usage", "用法", "示例"]);
    let has_params = has_section_with_content(
        content,
        &["input schema", "parameters", "parameter", "参数"],
    );

    if has_examples && has_params {
        return None;
    }

    let mut missing = Vec::new();
    if !has_examples {
        missing.push("使用案例 (Examples/Usage)");
    }
    if !has_params {
        missing.push("参数说明及示例 (Input Schema/Parameters with examples)");
    }

    Some(format!("SKILL.md 文档不完整，缺少: {}", missing.join("、")))
}

fn has_section_with_content(content: &str, keywords: &[&str]) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        let heading_lower = trimmed.trim_start_matches('#').trim().to_lowercase();
        if !keywords.iter().any(|kw| heading_lower.contains(kw)) {
            continue;
        }
        for next_line in &lines[(i + 1)..] {
            let next = next_line.trim();
            if next.is_empty() {
                continue;
            }
            if next.starts_with('#') {
                break;
            }
            return true;
        }
    }
    false
}

// ─── 技能目录收集 ────────────────────────────────────────────────────────────

/// 收集所有含 scripts 的 skill 目录
pub(super) fn collect_skill_dirs(skills_root: &Path) -> Vec<(PathBuf, String)> {
    if !skills_root.exists() {
        return Vec::new();
    }
    let mut dirs: Vec<(PathBuf, String)> = Vec::new();
    for entry in std::fs::read_dir(skills_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_evolved" || name == "_pending" {
                for e in std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                {
                    let p = e.path();
                    let sub = e.file_name().to_string_lossy().into_owned();
                    if !p.is_dir() {
                        continue;
                    }
                    if p.join("SKILL.md").exists() {
                        dirs.push((p, sub));
                    } else if sub == "_pending" {
                        for e2 in std::fs::read_dir(&p)
                            .ok()
                            .into_iter()
                            .flatten()
                            .filter_map(|e| e.ok())
                        {
                            let p2 = e2.path();
                            if p2.is_dir() && p2.join("SKILL.md").exists() {
                                dirs.push((p2, e2.file_name().to_string_lossy().into_owned()));
                            }
                        }
                    }
                }
            } else if path.join("SKILL.md").exists() {
                dirs.push((path, name));
            }
            continue;
        }
        if path.join("SKILL.md").exists() {
            dirs.push((path, name));
        }
    }
    dirs.into_iter()
        .filter(|(p, _)| !infer::list_scripts(p).is_empty())
        .collect()
}

// ─── 验证结果 ────────────────────────────────────────────────────────────────

/// 单个技能的验证结果
pub struct SkillValidation {
    pub skill_dir: PathBuf,
    pub skill_name: String,
    pub passed: bool,
    pub entry_point: Option<String>,
    pub test_input: Option<String>,
    pub error: String,
}

// ─── 工具函数 ────────────────────────────────────────────────────────────────

/// 从 error trace 提取可读摘要（优先 stderr 首行）
fn brief_error(trace: &str) -> String {
    fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
        if s.chars().count() > max_chars {
            format!("{}…", s.chars().take(max_chars.saturating_sub(3)).collect::<String>())
        } else {
            s.to_string()
        }
    }

    if trace.is_empty() {
        return String::new();
    }
    for section in ["stderr:\n", "stdout:\n"] {
        if let Some(part) = trace.split(section).nth(1) {
            let first = part
                .lines()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim())
                .unwrap_or("");
            if !first.is_empty() {
                return truncate_with_ellipsis(first, 80);
            }
        }
    }
    let first = trace.lines().next().unwrap_or("");
    truncate_with_ellipsis(first, 80)
}

// ─── validate_skills ─────────────────────────────────────────────────────────

/// 验证技能：对每个 skill infer → test → doc check，返回结果列表。
/// 若 `skill_names_filter` 为 `Some` 且非空，仅验证该列表中的技能名（目录名），否则验证全部。
pub async fn validate_skills<L: EvolutionLlm>(
    skills_root: &Path,
    llm: &L,
    model: &str,
    skill_names_filter: Option<&[String]>,
) -> Result<Vec<SkillValidation>> {
    let mut skill_dirs = collect_skill_dirs(skills_root);
    if let Some(names) = skill_names_filter {
        if !names.is_empty() {
            let set: std::collections::HashSet<&str> = names.iter().map(String::as_str).collect();
            skill_dirs.retain(|(_, name)| set.contains(name.as_str()));
            // 同名可能同时存在于 .skills/xxx 与 .skills/_evolved/xxx，只保留一个：优先 _evolved > _pending > 其它
            let prefer = |p: &PathBuf| {
                let s = p.to_string_lossy();
                if s.contains("_evolved") {
                    2
                } else if s.contains("_pending") {
                    1
                } else {
                    0
                }
            };
            let mut by_name: std::collections::HashMap<String, (PathBuf, String)> =
                std::collections::HashMap::new();
            for (path, name) in skill_dirs {
                let keep = match by_name.get(&name) {
                    None => true,
                    Some((existing, _)) => prefer(&path) > prefer(existing),
                };
                if keep {
                    by_name.insert(name.clone(), (path, name));
                }
            }
            // 按用户传入的筛选顺序输出，保证“只修选的”且顺序一致
            skill_dirs = names
                .iter()
                .filter_map(|n| by_name.get(n.as_str()).cloned())
                .collect();
        }
    }
    let total = skill_dirs.len();
    if total == 0 {
        eprintln!("📋 未找到可验证的技能（无 scripts 的已跳过）");
        return Ok(Vec::new());
    }

    eprintln!("📋 验证 {} 个技能...", total);
    let mut results = Vec::with_capacity(total);
    for (idx, (skill_dir, skill_name)) in skill_dirs.iter().enumerate() {
        eprintln!("  [{}/{}] {} ...", idx + 1, total, skill_name);

        let (entry_point, test_input) =
            match infer::infer_skill_execution(llm, model, skill_dir).await {
                Ok(ep) => ep,
                Err(e) => {
                    let err = format!("推理失败: {}", e);
                    tracing::warn!("Skill '{}' {}", skill_name, err);
                    results.push(SkillValidation {
                        skill_dir: skill_dir.clone(),
                        skill_name: skill_name.clone(),
                        passed: false,
                        entry_point: None,
                        test_input: None,
                        error: err,
                    });
                    continue;
                }
            };

        // 验证前先安装依赖；无 package.json/requirements.txt 时从 SKILL.md compatibility 推断
        let env_path: Option<PathBuf> = super::env_helper::ensure_skill_deps_and_env(skill_dir);

        let (passed, error) = match infer::test_skill_invoke(
            skill_dir,
            &entry_point,
            &test_input,
            env_path.as_deref(),
        ) {
            Ok((ok, trace)) => {
                if ok {
                    match check_skill_md_completeness(skill_dir, llm, model).await {
                        None => (true, String::new()),
                        Some(doc_err) => (false, doc_err),
                    }
                } else {
                    (false, trace)
                }
            }
            Err(e) => (false, format!("调用失败: {}", e)),
        };

        results.push(SkillValidation {
            skill_dir: skill_dir.clone(),
            skill_name: skill_name.clone(),
            passed,
            entry_point: Some(entry_point),
            test_input: Some(test_input),
            error,
        });
    }

    let pass = results.iter().filter(|v| v.passed).count();
    eprintln!("📋 验证完成: {} 通过, {} 失败", pass, total - pass);
    for v in &results {
        if v.passed {
            eprintln!("  ✅ {}", v.skill_name);
        } else {
            eprintln!("  ❌ {} → {}", v.skill_name, brief_error(&v.error));
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::brief_error;

    #[test]
    fn brief_error_handles_multibyte_without_panic() {
        let input = "SKILL.md 文档不完整，缺少: 缺少该AgentSkill（skill-creator）的完整使用案例（含具体输入参数值和预期输出），以及其所有输入参数的名称、类型和用途说明。文档中提供的示例和参数说明是针对辅助脚本的，不满足主流程。";
        let out = brief_error(input);
        assert!(!out.is_empty());
        assert!(out.chars().count() <= 81);
    }
}
