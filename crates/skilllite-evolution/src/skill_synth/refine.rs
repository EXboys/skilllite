//! 技能精炼与退役：refine_loop、refine_weakest_skill、retire_skills

use std::path::Path;

use anyhow::Result;

use crate::feedback;
use crate::gatekeeper_l3_content;
use crate::log_evolution_event;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

use super::infer;
use super::parse;
use super::query;
use super::scan;
use super::SkillMeta;
use super::SKILL_REFINEMENT_PROMPT;
use super::MAX_REFINE_ROUNDS;
use super::MAX_PARSE_RETRIES;
use super::RETIRE_UNUSED_DAYS;
use super::RETIRE_LOW_SUCCESS_RATE;

/// Retry fixing a skill script up to MAX_REFINE_ROUNDS times.
pub(super) async fn refine_loop<L: EvolutionLlm>(
    llm: &L,
    model: &str,
    skill_dir: &Path,
    skill_name: &str,
    skill_desc: &str,
    entry_point: &str,
    initial_script: &str,
    initial_error: &str,
    failure_type: &str,
    allow_network: bool,
) -> Result<Option<String>> {
    let mut current_script = initial_script.to_string();
    let mut current_error = initial_error.to_string();
    let script_path = skill_dir.join(entry_point);

    for round in 1..=MAX_REFINE_ROUNDS {
        tracing::info!(
            "Refinement round {}/{} for skill '{}'",
            round,
            MAX_REFINE_ROUNDS,
            skill_name
        );

        let prompt = SKILL_REFINEMENT_PROMPT
            .replace("{{skill_name}}", skill_name)
            .replace("{{skill_description}}", skill_desc)
            .replace("{{entry_point}}", entry_point)
            .replace("{{current_script}}", &current_script)
            .replace("{{error_trace}}", &current_error)
            .replace("{{failure_type}}", failure_type)
            .replace("{{current_test_input}}", "")
            .replace("{{current_skill_md}}", "");

        let messages = vec![EvolutionMessage::user(&prompt)];
        let content = llm.complete(&messages, model, 0.3).await?.trim().to_string();

        let parsed = match parse::parse_refinement_response(&content) {
            Ok(Some(r)) => {
                if r.fix_type != parse::FixType::Script || r.fixed_script.is_none() {
                    tracing::info!("LLM returned non-script fix for security_scan, skipping");
                    return Ok(None);
                }
                r
            }
            Ok(None) => {
                tracing::info!("LLM skipped refinement for '{}': unfixable", skill_name);
                return Ok(None);
            }
            Err(e) => {
                if MAX_PARSE_RETRIES == 0 {
                    tracing::warn!("Failed to parse refinement output (round {}): {}", round, e);
                    return Ok(None);
                }
                tracing::info!(
                    "Refinement JSON parse failed (round {}), retrying with LLM feedback: {}",
                    round, e
                );
                let retry_msg = format!(
                    "你的输出无法解析为 JSON。错误: {}。请重新输出，严格遵循格式: {{\"fixed_script\": \"完整 Python 脚本\", \"fix_summary\": \"修正说明\", \"skip_reason\": \"若无法修正则说明原因\"}}。换行用 \\n 转义。",
                    e
                );
                let mut msgs = messages.to_vec();
                msgs.push(EvolutionMessage::user(&retry_msg));
                let content2 = llm.complete(&msgs, model, 0.3).await?.trim().to_string();
                match parse::parse_refinement_response(&content2) {
                    Ok(Some(r)) => {
                        if r.fix_type != parse::FixType::Script || r.fixed_script.is_none() {
                            return Ok(None);
                        }
                        r
                    }
                    Ok(None) => {
                        tracing::info!("LLM skipped refinement for '{}': unfixable", skill_name);
                        return Ok(None);
                    }
                    Err(e2) => {
                        tracing::warn!("Refinement parse retry failed (round {}): {}", round, e2);
                        return Ok(None);
                    }
                }
            }
        };

        let fixed_script = match parsed.fixed_script.as_ref() {
            Some(s) => s,
            None => {
                tracing::warn!("Refinement parse returned Script fix_type but no fixed_script");
                return Ok(None);
            }
        };
        if fixed_script.lines().count() > 150 {
            tracing::warn!(
                "Refined script exceeds 150 lines (round {}), asking for shorter fix",
                round
            );
            current_error =
                "上一轮修正后脚本超过 150 行被拒绝。请只做最小化修改，不要重写整个脚本。"
                    .to_string();
            continue;
        }
        if let Err(e) = gatekeeper_l3_content(fixed_script) {
            tracing::warn!("L3 rejected refined script (round {}): {}", round, e);
            return Ok(None);
        }

        let scan_ok = scan::run_l4_scan(fixed_script, &script_path, allow_network)?;
        if scan_ok {
            tracing::info!(
                "Refinement succeeded for '{}' in round {}: {}",
                skill_name,
                round,
                parsed.fix_summary
            );
            return Ok(Some(fixed_script.clone()));
        }

        current_script = fixed_script.clone();
        current_error = format!(
            "Previous fix attempt (round {}) still failed security scan. Summary: {}",
            round, parsed.fix_summary
        );
    }

    tracing::warn!(
        "Skill '{}' still failing after {} refinement rounds, abandoning",
        skill_name,
        MAX_REFINE_ROUNDS
    );
    Ok(None)
}

pub(super) async fn refine_weakest_skill<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
) -> Result<Option<String>> {
    let evolved_dir = skills_root.join("_evolved");
    if !evolved_dir.exists() {
        return Ok(None);
    }

    let mut weakest: Option<(String, SkillMeta, f64)> = None;

    for entry in std::fs::read_dir(&evolved_dir)?.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        let meta_path = skill_dir.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        let meta: SkillMeta = match skilllite_fs::read_file(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        if meta.archived || meta.call_count < 3 {
            continue;
        }
        let rate = meta.success_rate();
        if rate >= 0.60 {
            continue;
        }
        if weakest.as_ref().map_or(true, |(_, _, r)| rate < *r) {
            let name = entry.file_name().to_string_lossy().to_string();
            weakest = Some((name.clone(), meta.clone(), rate));
        }
    }

    let (skill_name, _meta, _rate) = match weakest {
        Some(w) => w,
        None => return Ok(None),
    };

    let skill_dir = evolved_dir.join(&skill_name);
    let skill_md_path = skill_dir.join("SKILL.md");
    let skill_md = skilllite_fs::read_file(&skill_md_path).unwrap_or_default();

    let (entry_point, _test_input) = infer::infer_skill_execution(llm, model, &skill_dir).await?;
    let script_path = skill_dir.join(&entry_point);
    let current_script = skilllite_fs::read_file(&script_path).unwrap_or_default();

    if current_script.is_empty() {
        return Ok(None);
    }

    let error_trace = {
        let conn = feedback::open_evolution_db(chat_root)?;
        query::query_skill_failures(&conn, &skill_name)?
    };

    if error_trace.is_empty() {
        return Ok(None);
    }

    let desc = infer::extract_description_from_skill_md(&skill_md);
    let allow_network = scan::skill_md_needs_network(&skill_md);

    let fixed = refine_loop(
        llm,
        model,
        &skill_dir,
        &skill_name,
        &desc,
        &entry_point,
        &current_script,
        &error_trace,
        "execution_failure",
        allow_network,
    )
    .await?;

    if let Some(fixed_script) = fixed {
        skilllite_fs::write_file(&script_path, &fixed_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755));
        }

        if let Ok(conn) = feedback::open_evolution_db(chat_root) {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "skill_refined",
                &skill_name,
                "Refined after low success rate",
                txn_id,
            );
        }

        tracing::info!("Refined evolved skill: {}", skill_name);
        return Ok(Some(skill_name));
    }

    Ok(None)
}

/// Retire skills, using the provided connection. Reduces DB opens when called from evolve_skills.
pub(super) fn retire_skills_with_conn(
    chat_root: &Path,
    skills_root: &Path,
    txn_id: &str,
    conn: &rusqlite::Connection,
) -> Result<Vec<(String, String)>> {
    let evolved_dir = skills_root.join("_evolved");
    if !evolved_dir.exists() {
        return Ok(Vec::new());
    }

    let mut retired = Vec::new();
    let mut to_log: Vec<(String, String)> = Vec::new(); // (name, reason)
    let now = chrono::Utc::now();

    for entry in std::fs::read_dir(&evolved_dir)?.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        if entry.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        let meta_path = skill_dir.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        let mut meta: SkillMeta = match skilllite_fs::read_file(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        if meta.archived {
            continue;
        }

        let should_retire = if meta.call_count >= 3 && meta.success_rate() < RETIRE_LOW_SUCCESS_RATE
        {
            Some(format!(
                "success rate {:.0}% < {:.0}% threshold",
                meta.success_rate() * 100.0,
                RETIRE_LOW_SUCCESS_RATE * 100.0,
            ))
        } else if let Some(ref last) = meta.last_used {
            if let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(last) {
                let days = (now - last_dt.with_timezone(&chrono::Utc)).num_days();
                if days >= RETIRE_UNUSED_DAYS {
                    Some(format!(
                        "unused for {} days (threshold: {})",
                        days, RETIRE_UNUSED_DAYS
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&meta.created_at) {
                let days = (created.with_timezone(&chrono::Utc) - now).num_days().abs();
                if days >= RETIRE_UNUSED_DAYS {
                    Some(format!("never used, {} days since creation", days))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(reason) = should_retire {
            meta.archived = true;
            let _ =
                skilllite_fs::write_file(&meta_path, &serde_json::to_string_pretty(&meta)?);

            let name = entry.file_name().to_string_lossy().to_string();
            tracing::info!("Retired skill '{}': {}", name, reason);
            to_log.push((name.clone(), reason));
            retired.push(("skill_retired".to_string(), name));
        }
    }

    if !to_log.is_empty() {
        for (name, reason) in &to_log {
            let _ = log_evolution_event(conn, chat_root, "skill_retired", name, reason, txn_id);
        }
    }

    Ok(retired)
}
