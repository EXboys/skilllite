//! 技能生成：成功驱动 + 失败驱动

use std::path::Path;

use crate::Result;

use crate::feedback;
use crate::gatekeeper_l1_path;
use crate::gatekeeper_l3_content;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

use super::infer;
use super::parse;
use super::query;
use super::refine;
use super::repair;
use super::scan;
use super::validate;
use super::SkillMeta;
use super::MAX_EVOLVED_SKILLS;
use super::SKILL_GENERATION_FROM_FAILURES_PROMPT;
use super::SKILL_GENERATION_PROMPT;

/// Pre-fetched (patterns_display, executions) for success-driven generation. When `Some`, caller holds conn and passed data to avoid reopening DB.
pub(super) type SuccessQueryData = (String, String);
/// Pre-fetched (failed_patterns, failed_executions) for failure-driven generation.
pub(super) type FailureQueryData = (String, String);

/// 成功驱动：从高成功率模式生成 Skill。
/// `pre_fetched`: 若为 `Some` 则使用已有查询结果，否则本函数内打开 DB 查询。
pub(super) async fn generate_skill<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
    min_pattern_count: u32,
    pre_fetched: Option<SuccessQueryData>,
) -> Result<Option<String>> {
    let evolved_dir = skills_root.join("_evolved");
    let pending_dir = evolved_dir.join("_pending");

    let current_count = infer::count_active_evolved_skills(&evolved_dir);
    if current_count >= MAX_EVOLVED_SKILLS {
        tracing::debug!(
            "Evolved skill cap reached ({}/{}), skipping generation",
            current_count,
            MAX_EVOLVED_SKILLS
        );
        return Ok(None);
    }

    let (patterns, executions) = match pre_fetched {
        Some((p, e)) => (p, e),
        None => {
            let conn = feedback::open_evolution_db(chat_root)?;
            let (patterns_display, pattern_descs) =
                query::query_repeated_patterns(&conn, min_pattern_count)?;
            let executions = if !pattern_descs.is_empty() {
                query::query_pattern_executions(&conn, &pattern_descs)?
            } else {
                String::new()
            };
            (patterns_display, executions)
        }
    };

    if patterns.is_empty() {
        return Ok(None);
    }

    let existing_skills = infer::list_existing_skill_names(skills_root);

    let prompt = SKILL_GENERATION_PROMPT
        .replace("{{repeated_patterns}}", &patterns)
        .replace("{{successful_executions}}", &executions)
        .replace("{{existing_skills}}", &existing_skills);

    let messages = vec![EvolutionMessage::user(&prompt)];
    let parsed = match parse::parse_skill_generation_with_retry(llm, model, &messages).await {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(None),
        Err(e) => {
            tracing::warn!(
                "Failed to parse skill generation output (after retry): {}",
                e
            );
            return Ok(None);
        }
    };

    let name = generate_skill_inner(
        parsed,
        chat_root,
        skills_root,
        &pending_dir,
        txn_id,
        llm,
        model,
    )
    .await?;
    if let Some(ref n) = name {
        tracing::info!("Generated evolved skill (pending confirmation): {}", n);
    }
    Ok(name)
}

pub(super) async fn generate_skill_inner<L: EvolutionLlm>(
    parsed: parse::GeneratedSkill,
    chat_root: &Path,
    skills_root: &Path,
    pending_dir: &Path,
    txn_id: &str,
    llm: &L,
    model: &str,
) -> Result<Option<String>> {
    if let Err(e) = gatekeeper_l3_content(&parsed.script_content) {
        tracing::warn!("L3 rejected generated skill script: {}", e);
        return Ok(None);
    }
    if let Err(e) = gatekeeper_l3_content(&parsed.skill_md_content) {
        tracing::warn!("L3 rejected generated SKILL.md: {}", e);
        return Ok(None);
    }
    // 写入前校验文档完整性，避免落盘不完整 SKILL.md（缺少 Usage/Examples 等）导致后续验证失败
    if let Some(doc_err) = validate::check_skill_md_completeness_heuristic(&parsed.skill_md_content)
    {
        tracing::warn!(
            "Generated SKILL.md for '{}' incomplete ({}), skipping write to avoid later validate failure",
            parsed.name, doc_err
        );
        return Ok(None);
    }

    let skill_dir = pending_dir.join(&parsed.name);
    // 同名去重：同轮内若已有同名 pending skill，跳过避免覆盖
    if skill_dir.exists() && skill_dir.join("SKILL.md").exists() {
        tracing::debug!(
            "Skill '{}' already in pending (same name), skipping to avoid duplicate",
            parsed.name
        );
        return Ok(None);
    }
    // 描述相似去重（可选）：SKILLLITE_SKILL_DEDUP_DESCRIPTION=0 可关闭
    let dedup_desc = std::env::var("SKILLLITE_SKILL_DEDUP_DESCRIPTION")
        .map(|v| v != "0")
        .unwrap_or(true);
    if dedup_desc {
        for (existing_name, existing_desc) in infer::list_pending_skill_descriptions(pending_dir) {
            if infer::is_description_similar(&parsed.description, &existing_desc) {
                tracing::debug!(
                    "Skill '{}' description similar to pending '{}', skipping duplicate",
                    parsed.name,
                    existing_name
                );
                return Ok(None);
            }
        }
    }
    if !gatekeeper_l1_path(chat_root, &skill_dir, Some(skills_root)) {
        tracing::warn!("L1 rejected skill directory: {}", skill_dir.display());
        return Ok(None);
    }
    std::fs::create_dir_all(&skill_dir)?;

    let script_path = skill_dir.join(&parsed.entry_point);
    let skill_md_path = skill_dir.join("SKILL.md");

    let needs_network = scan::skill_md_needs_network(&parsed.skill_md_content);
    let scan_result = scan::run_l4_scan(&parsed.script_content, &script_path, needs_network)?;

    if !scan_result {
        let refined = refine::refine_loop(
            llm,
            model,
            &skill_dir,
            &parsed.name,
            &parsed.description,
            &parsed.entry_point,
            &parsed.script_content,
            "Security scan found critical/high issues",
            "security_scan",
            needs_network,
        )
        .await?;

        match refined {
            Some(fixed_script) => {
                write_skill_files(
                    &skill_dir,
                    &skill_md_path,
                    &script_path,
                    &parsed.skill_md_content,
                    &fixed_script,
                    &parsed.name,
                    txn_id,
                    false,
                )?;
                let (ep, ti) = infer::infer_skill_execution(llm, model, &skill_dir)
                    .await
                    .unwrap_or_else(|_| (parsed.entry_point.clone(), "{}".to_string()));
                let _ =
                    repair::repair_one_skill(llm, model, &skill_dir, &parsed.name, &ep, &ti, None)
                        .await?;
            }
            None => {
                let final_script = parsed.script_content.clone();
                write_skill_files(
                    &skill_dir,
                    &skill_md_path,
                    &script_path,
                    &parsed.skill_md_content,
                    &final_script,
                    &parsed.name,
                    txn_id,
                    true,
                )?;
                tracing::info!(
                    "Skill '{}' saved as draft (L4 未通过，需人工审核后 confirm)",
                    parsed.name
                );
            }
        }
    } else {
        let final_script = parsed.script_content.clone();
        write_skill_files(
            &skill_dir,
            &skill_md_path,
            &script_path,
            &parsed.skill_md_content,
            &final_script,
            &parsed.name,
            txn_id,
            false,
        )?;
        let (ep, ti) = infer::infer_skill_execution(llm, model, &skill_dir)
            .await
            .unwrap_or_else(|_| (parsed.entry_point.clone(), "{}".to_string()));
        let _ =
            repair::repair_one_skill(llm, model, &skill_dir, &parsed.name, &ep, &ti, None).await?;
    }

    Ok(Some(parsed.name))
}

/// 失败驱动：从持续失败模式生成 Skill（补全能力缺口）。
/// `pre_fetched`: 若为 `Some` 则使用已有查询结果，否则本函数内打开 DB 查询。
pub(super) async fn generate_skill_from_failures<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
    pre_fetched: Option<FailureQueryData>,
) -> Result<Option<String>> {
    let evolved_dir = skills_root.join("_evolved");
    let pending_dir = evolved_dir.join("_pending");

    if infer::count_active_evolved_skills(&evolved_dir) >= MAX_EVOLVED_SKILLS {
        return Ok(None);
    }

    let (failed_patterns, failed_executions) = match pre_fetched {
        Some((p, e)) => (p, e),
        None => {
            let conn = feedback::open_evolution_db(chat_root)?;
            let patterns = query::query_failed_patterns(&conn, 2)?;
            let executions = query::query_failed_executions(&conn)?;
            (patterns, executions)
        }
    };

    if failed_patterns.is_empty() {
        return Ok(None);
    }

    let existing_skills = infer::list_existing_skill_names(skills_root);

    let prompt = SKILL_GENERATION_FROM_FAILURES_PROMPT
        .replace("{{failed_patterns}}", &failed_patterns)
        .replace("{{failed_executions}}", &failed_executions)
        .replace("{{existing_skills}}", &existing_skills);

    let messages = vec![EvolutionMessage::user(&prompt)];
    let parsed = match parse::parse_skill_generation_with_retry(llm, model, &messages).await {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(None),
        Err(e) => {
            tracing::warn!("Failed to parse failure-driven skill output: {}", e);
            return Ok(None);
        }
    };

    let name = generate_skill_inner(
        parsed,
        chat_root,
        skills_root,
        &pending_dir,
        txn_id,
        llm,
        model,
    )
    .await?;
    if let Some(ref n) = name {
        tracing::info!("Generated failure-driven skill (补全): {}", n);
    }
    Ok(name)
}

#[allow(clippy::too_many_arguments)]
fn write_skill_files(
    skill_dir: &Path,
    skill_md_path: &Path,
    script_path: &Path,
    skill_md: &str,
    script: &str,
    name: &str,
    txn_id: &str,
    needs_review: bool,
) -> Result<()> {
    skilllite_fs::write_file(skill_md_path, skill_md)?;
    skilllite_fs::write_file(script_path, script)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(script_path, std::fs::Permissions::from_mode(0o755));
    }

    let meta = SkillMeta {
        name: name.to_string(),
        source_session: String::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        success_count: 0,
        failure_count: 0,
        call_count: 0,
        last_used: None,
        archived: false,
        generation_txn: txn_id.to_string(),
        needs_review,
    };
    let meta_path = skill_dir.join(".meta.json");
    skilllite_fs::write_file(&meta_path, &serde_json::to_string_pretty(&meta)?)?;

    Ok(())
}
