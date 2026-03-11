//! 技能修复：对验证失败的 skill，打包整个目录内容给模型一次性修复

use std::path::Path;

use anyhow::Result;

use crate::gatekeeper_l3_content;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

use super::infer;
use super::parse;
use super::validate::{self, SkillValidation};
use super::SKILL_REFINEMENT_PROMPT;
use super::MAX_REFINE_ROUNDS;

// ─── 工具函数 ────────────────────────────────────────────────────────────────

/// 打包技能目录：目录结构 + 所有文件完整内容
fn build_skill_dir_package(skill_dir: &Path) -> String {
    let mut lines = Vec::new();
    lines.push("## 目录结构".to_string());
    let mut entries: Vec<String> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(skill_dir) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir() {
                entries.push(format!("{}/", name));
                if let Ok(sub) = std::fs::read_dir(&path) {
                    for se in sub.filter_map(|e| e.ok()) {
                        entries.push(format!("  {}", se.file_name().to_string_lossy()));
                    }
                }
            } else {
                entries.push(name);
            }
        }
    }
    entries.sort();
    lines.push(entries.join("\n"));
    lines.push(String::new());
    lines.push("## 文件内容".to_string());

    let skill_md_path = skill_dir.join("SKILL.md");
    if skill_md_path.is_file() {
        lines.push("### SKILL.md".to_string());
        lines.push(
            skilllite_fs::read_file(&skill_md_path).unwrap_or_else(|_| "(无法读取)".to_string()),
        );
        lines.push(String::new());
    }

    for rel in infer::list_scripts(skill_dir) {
        let full = skill_dir.join(&rel);
        if full.is_file() {
            lines.push(format!("### {}", rel));
            lines.push(
                skilllite_fs::read_file(&full).unwrap_or_else(|_| "(无法读取)".to_string()),
            );
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

// ─── repair_one_skill（核心：打包目录 → 模型修复 → 应用 → 验证）───────────

/// 修复单个技能：打包整个目录给模型，模型返回修复，应用后验证，最多 MAX_REFINE_ROUNDS 轮
pub async fn repair_one_skill<L: EvolutionLlm>(
    llm: &L,
    model: &str,
    skill_dir: &Path,
    _skill_name: &str,
    entry_point: &str,
    test_input: &str,
    on_msg: Option<&(dyn Fn(&str) + Send + Sync)>,
) -> Result<(bool, String)> {
    let script_path = skill_dir.join(entry_point);
    let skill_md_path = skill_dir.join("SKILL.md");
    let mut current_test_input = test_input.to_string();

    for round in 1..=MAX_REFINE_ROUNDS {
        // 每轮先安装/更新依赖：无 package.json/requirements.txt 时从 SKILL.md compatibility 推断（大模型可能上轮已补全）
        let env_path = super::env_helper::ensure_skill_deps_and_env(skill_dir);

        let (exec_ok, exec_trace) =
            infer::test_skill_invoke(skill_dir, entry_point, &current_test_input, env_path.as_deref())?;
        let doc_error = validate::check_skill_md_completeness(skill_dir, llm, model).await;
        if exec_ok && doc_error.is_none() {
            return Ok((true, String::new()));
        }
        if let Some(f) = on_msg {
            f(&format!("第 {}/{} 轮修复…", round, MAX_REFINE_ROUNDS));
        }

        let error_trace = if !exec_ok {
            exec_trace
        } else {
            format!("脚本执行通过，但 {}", doc_error.unwrap())
        };

        let package = build_skill_dir_package(skill_dir);
        let prompt = SKILL_REFINEMENT_PROMPT
            .replace("{{skill_dir_package}}", &package)
            .replace("{{tested_script}}", entry_point)
            .replace("{{current_test_input}}", &current_test_input)
            .replace("{{error_trace}}", &error_trace);

        let messages = vec![EvolutionMessage::user(&prompt)];
        let (parsed, raw_dbg) = llm_repair_call(llm, model, &messages, &error_trace).await?;
        let Some(parsed) = parsed else {
            tracing::warn!("Skill repair round {}: model returned no fix. Raw: {}", round, raw_dbg);
            if let Some(f) = on_msg {
                f(&format!("第 {} 轮模型未给出有效修复，继续重试…", round));
            }
            continue;
        };

        if let Some(f) = on_msg {
            f(&parsed.fix_summary);
            if let Some(ref r) = parsed.user_reply {
                f(r);
            }
        }

        if let Some(ref md) = parsed.fix_skill_md {
            if gatekeeper_l3_content(md).is_ok()
                && validate::check_skill_md_completeness_heuristic(md).is_none()
            {
                skilllite_fs::write_file(&skill_md_path, md)?;
            } else if gatekeeper_l3_content(md).is_ok() {
                tracing::warn!(
                    "Repair returned fix_skill_md but still incomplete (missing Usage/Examples), skip applying"
                );
            }
        }
        if let Some(ref script) = parsed.fixed_script {
            if gatekeeper_l3_content(script).is_ok() {
                skilllite_fs::write_file(&script_path, script)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        &script_path,
                        std::fs::Permissions::from_mode(0o755),
                    );
                }
            }
        }
        if let Some(ref ti) = parsed.fix_test_input {
            current_test_input = ti.clone();
        }
    }

    let env_path = super::env_helper::ensure_skill_deps_and_env(skill_dir);
    let (ok, final_trace) =
        infer::test_skill_invoke(skill_dir, entry_point, &current_test_input, env_path.as_deref())?;
    let doc_error = validate::check_skill_md_completeness(skill_dir, llm, model).await;
    if ok && doc_error.is_none() {
        return Ok((true, String::new()));
    }
    let fail_reason = if !ok {
        final_trace.lines().take(20).collect::<Vec<_>>().join("\n")
    } else {
        doc_error.unwrap_or_default()
    };
    Ok((false, format!(
        "{} 轮修复后仍失败\n{}",
        MAX_REFINE_ROUNDS,
        fail_reason
    )))
}

/// 调用模型获取修复方案（含 unfixable 重试 + JSON 解析重试）
/// 返回 (Option<修复方案>, 最后一次模型原始输出)
async fn llm_repair_call<L: EvolutionLlm>(
    llm: &L,
    model: &str,
    messages: &[EvolutionMessage],
    error_trace: &str,
) -> Result<(Option<parse::RefinedSkill>, String)> {
    let content = llm.complete(messages, model, 0.3).await?.trim().to_string();
    let mut last_raw = content.clone();

    match parse::parse_refinement_response(&content) {
        Ok(Some(r)) => return Ok((Some(r), last_raw)),
        Ok(None) => {
            tracing::info!("Repair: model returned no fix or unfixable, will retry");
        }
        Err(e) => {
            tracing::info!("Repair JSON parse failed: {}", e);
            let retry_msg = format!(
                "你的输出无法解析为 JSON。错误: {}。请重新输出合法 JSON，包含 fix_summary 和至少一个修复字段（fixed_script / fix_test_input / fix_skill_md）。所有键名用双引号，换行用 \\n 转义。",
                e
            );
            let mut msgs = messages.to_vec();
            msgs.push(EvolutionMessage::user(&retry_msg));
            let content2 = llm.complete(&msgs, model, 0.3).await?.trim().to_string();
            last_raw = content2.clone();
            if let Ok(Some(r)) = parse::parse_refinement_response(&content2) {
                return Ok((Some(r), last_raw));
            }
        }
    }

    let err_brief: String = error_trace.lines().take(10).collect::<Vec<_>>().join("\n");
    let mut msgs = messages.to_vec();
    for attempt in 1..=3 {
        let force_msg = format!(
            "【第{attempt}次强制要求】这个技能必须被修复，没有例外。\n\n错误：\n{err_brief}\n\n你必须输出一个合法的 JSON 对象，至少包含以下字段之一：\n- \"fix_test_input\": \"{{\\\"base\\\": 2, \\\"exponent\\\": 10}}\"  （注意：值是字符串，内容是 JSON）\n- \"fixed_script\": \"#!/usr/bin/env python3\\n...\"  （完整脚本内容）\n- \"fix_skill_md\": \"---\\nname: ...\\n---\\n...\"  （完整 SKILL.md）\n\n同时必须包含 \"fix_summary\": \"说明修了什么\"。\n\n示例输出：\n```json\n{{\"fix_summary\": \"补充测试输入\", \"fix_test_input\": \"{{\\\"base\\\": 2, \\\"exponent\\\": 10}}\", \"fixed_script\": null, \"fix_skill_md\": null}}\n```\n\n不要返回 unfixable。不要解释，直接输出 JSON。",
        );
        msgs.push(EvolutionMessage::user(&force_msg));
        let content_r = llm.complete(&msgs, model, 0.3).await?.trim().to_string();
        last_raw = content_r.clone();
        tracing::info!("Force retry {attempt}: raw len={}", content_r.len());
        if let Ok(Some(r)) = parse::parse_refinement_response(&content_r) {
            return Ok((Some(r), last_raw));
        }
    }
    Ok((None, last_raw))
}

// ─── repair_skills ───────────────────────────────────────────────────────────

/// 修复技能：先验证，再对失败的逐个打包修复
pub async fn repair_skills<L: EvolutionLlm>(
    skills_root: &Path,
    llm: &L,
    model: &str,
) -> Result<Vec<(String, bool)>> {
    let validated = validate::validate_skills(skills_root, llm, model, None).await?;
    let failed: Vec<&SkillValidation> = validated
        .iter()
        .filter(|v| !v.passed)
        .collect();

    if failed.is_empty() {
        return Ok(validated
            .into_iter()
            .map(|v| (v.skill_name, true))
            .collect());
    }

    eprintln!("\n🔧 修复 {} 个失败的技能...", failed.len());
    let mut results: Vec<(String, bool)> = Vec::new();
    for v in &validated {
        if v.passed {
            results.push((v.skill_name.clone(), true));
            continue;
        }
        let (ep, ti) = match (&v.entry_point, &v.test_input) {
            (Some(ep), Some(ti)) => (ep.as_str(), ti.as_str()),
            _ => {
                eprintln!("  ⏭️ {} (推理失败，跳过)", v.skill_name);
                results.push((v.skill_name.clone(), false));
                continue;
            }
        };
        let idx = results.iter().filter(|(_, ok)| !ok).count() + 1;
        eprintln!("🔧 [{}/{}] {} ...", idx, failed.len(), v.skill_name);
        let on_msg = |msg: &str| eprintln!("  💬 {}", msg);
        let (ok, reason) = repair_one_skill(
            llm, model, &v.skill_dir, &v.skill_name, ep, ti, Some(&on_msg),
        )
        .await
        .unwrap_or_else(|e| (false, format!("{}", e)));

        if ok {
            eprintln!("  ✅ {}", v.skill_name);
        } else {
            eprintln!("  ❌ {}\n{}", v.skill_name, reason);
        }
        results.push((v.skill_name.clone(), ok));
    }
    Ok(results)
}
