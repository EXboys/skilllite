//! Full evolution cycle orchestration.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::audit::{log_evolution_event, mark_decisions_evolved};
use crate::changelog::append_changelog;
use crate::config::EvolutionMode;
use crate::external_learner;
use crate::feedback;
use crate::llm::EvolutionLlm;
use crate::memory_learner;
use crate::prompt_learner;
use crate::rollback::check_auto_rollback;
use crate::run_state::{finish_evolution, try_start_evolution, EvolutionRunResult};
use crate::scope::{
    auto_link_acceptance_status, build_evolution_proposals, coordinate_proposals,
    load_backlog_proposal_by_id, recover_forced_proposal_by_authorization_log, set_backlog_status,
    CoordinatorDecision,
};
use crate::skill_synth;
use crate::snapshots::{create_extended_snapshot, versions_dir};
use crate::Result;

/// One audit row per `run_evolution` invocation when the run does not enter execution
/// (or fails before returning `Ok`). Uses `evolution_log` + `evolution.log`; ignores DB errors.
fn try_log_evolution_run_outcome(chat_root: &Path, reason: &str) {
    if let Ok(conn) = feedback::open_evolution_db(chat_root) {
        let _ = log_evolution_event(
            &conn,
            chat_root,
            "evolution_run_outcome",
            "run",
            reason,
            "",
        );
    }
}

// ─── Run evolution (main entry point) ──────────────────────────────────────────

/// Run a full evolution cycle.
///
/// Returns [EvolutionRunResult]: SkippedBusy if another run in progress, NoScope if nothing to evolve, Completed(txn_id) otherwise.
/// When force=true (manual trigger), bypass decision thresholds.
/// skills_root: project-level dir (workspace/.skills). When None, skips skill evolution.
pub async fn run_evolution<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: Option<&Path>,
    llm: &L,
    api_base: &str,
    api_key: &str,
    model: &str,
    force: bool,
) -> Result<EvolutionRunResult> {
    if !try_start_evolution() {
        return Ok(EvolutionRunResult::SkippedBusy);
    }

    let result =
        run_evolution_inner(chat_root, skills_root, llm, api_base, api_key, model, force).await;

    finish_evolution();
    result
}

async fn run_evolution_inner<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: Option<&Path>,
    llm: &L,
    _api_base: &str,
    _api_key: &str,
    model: &str,
    force: bool,
) -> Result<EvolutionRunResult> {
    let conn = feedback::open_evolution_db(chat_root)?;
    let forced_proposal_id = std::env::var("SKILLLITE_EVO_FORCE_PROPOSAL_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let decision = if let Some(pid) = forced_proposal_id.as_deref() {
        match load_backlog_proposal_by_id(&conn, pid)? {
            Some(p) => {
                tracing::info!(
                    "Manual proposal trigger: forcing execution candidate {}",
                    pid
                );
                coordinate_proposals(&conn, vec![p], true)?
            }
            None => {
                if let Some(recovered) = recover_forced_proposal_by_authorization_log(&conn, pid)? {
                    let _ = log_evolution_event(
                        &conn,
                        chat_root,
                        "evolution_proposal_recovered",
                        pid,
                        &format!(
                            "Forced proposal id was stale; recovered backlog proposal {} via authorization log",
                            recovered.proposal_id
                        ),
                        "",
                    );
                    coordinate_proposals(&conn, vec![recovered], true)?
                } else {
                    let _ = log_evolution_event(
                        &conn,
                        chat_root,
                        "evolution_proposal_missing",
                        pid,
                        "Forced proposal id not found in backlog",
                        "",
                    );
                    return Ok(EvolutionRunResult::NoScope);
                }
            }
        }
    } else {
        let proposals = build_evolution_proposals(&conn, EvolutionMode::from_env(), force)?;
        coordinate_proposals(&conn, proposals, force)?
    };
    let (scope, proposal) = match decision {
        CoordinatorDecision::NoCandidate => return Ok(EvolutionRunResult::NoScope),
        CoordinatorDecision::Queued(p) => {
            let reason = format!(
                "Proposal {} ({}) queued; waiting execution gate",
                p.proposal_id,
                p.source.as_str()
            );
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_proposal",
                &p.proposal_id,
                &reason,
                "",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }
        CoordinatorDecision::Denied(p) => {
            let reason = format!(
                "Proposal {} ({}) denied by policy runtime",
                p.proposal_id,
                p.source.as_str()
            );
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_proposal_denied",
                &p.proposal_id,
                &reason,
                "",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }
        CoordinatorDecision::Execute(p) => (p.scope.clone(), p),
    };

    let txn_id = format!("evo_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    tracing::info!(
        "Starting evolution txn={} proposal={} source={} (prompts={}, memory={}, skills={})",
        txn_id,
        proposal.proposal_id,
        proposal.source.as_str(),
        scope.prompts,
        scope.memory,
        scope.skills
    );
    let snapshot_files = create_extended_snapshot(
        chat_root,
        skills_root,
        &txn_id,
        scope.prompts,
        scope.memory,
        scope.skills,
    )?;

    // Drop conn before async work (Connection is !Send, cannot hold across .await).
    drop(conn);

    let mut all_changes: Vec<(String, String)> = Vec::new();
    let mut reason_parts: Vec<String> = Vec::new();

    // Run prompts / skills / memory evolution in parallel. Each module uses block_in_place
    // to batch its DB operations (one open per module), so we get both parallelism and fewer opens.
    let (prompt_res, skills_res, memory_res) = tokio::join!(
        async {
            if scope.prompts {
                prompt_learner::evolve_prompts(chat_root, llm, model, &txn_id).await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if scope.skills {
                let generate = scope.skill_action.should_run_skill_generation_paths();
                skill_synth::evolve_skills(
                    chat_root,
                    skills_root,
                    llm,
                    model,
                    &txn_id,
                    generate,
                    force,
                )
                .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if scope.memory {
                memory_learner::evolve_memory(chat_root, llm, model, &txn_id).await
            } else {
                Ok(Vec::new())
            }
        },
    );

    if scope.prompts {
        match prompt_res {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} prompt changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Prompt evolution failed: {}", e),
        }
    }
    if scope.skills {
        match skills_res {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} skill changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Skill evolution failed: {}", e),
        }
    }
    if scope.memory {
        match memory_res {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} memory knowledge update(s)", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Memory evolution failed: {}", e),
        }
    }

    // Run external learning before changelog so its changes and modified files are in the same txn entry.
    match external_learner::run_external_learning(chat_root, llm, model, &txn_id).await {
        Ok(ext_changes) => {
            if !ext_changes.is_empty() {
                tracing::info!("EVO-6: {} external changes applied", ext_changes.len());
                reason_parts.push(format!("{} external change(s)", ext_changes.len()));
                all_changes.extend(ext_changes);
            }
        }
        Err(e) => tracing::warn!("EVO-6 external learning failed (non-fatal): {}", e),
    }

    {
        let conn = feedback::open_evolution_db(chat_root)?;

        for (ctype, cid) in &all_changes {
            log_evolution_event(&conn, chat_root, ctype, cid, "prompt evolution", &txn_id)?;
        }

        if scope.prompts {
            if let Err(e) = prompt_learner::update_reusable_status(&conn, chat_root) {
                tracing::warn!("Failed to update reusable status: {}", e);
            }
        }

        mark_decisions_evolved(&conn, &scope.decision_ids)?;
        let _ = feedback::update_daily_metrics(&conn);
        let auto_rolled_back = check_auto_rollback(&conn, chat_root, skills_root)?;
        if auto_rolled_back {
            tracing::info!("EVO: auto-rollback triggered for txn={}", txn_id);
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                "rollback",
                "Auto-rollback triggered due to performance degradation",
                &txn_id,
            );
        } else {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                "no_rollback",
                "No auto-rollback triggered",
                &txn_id,
            );
        }
        // let _ = feedback::export_judgement(&conn, &chat_root.join("JUDGEMENT.md")); // Removed for refactor
        if let Ok(Some(summary)) = feedback::build_latest_judgement(&conn) {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                summary.judgement.as_str(),
                &summary.reason,
                &txn_id,
            );
            // Insert new judgement output to file here
            let judgement_output = format!(
                "## Evolution Judgement\n\n**Judgement:** {}\n\n**Reason:** {}\n",
                summary.judgement.as_str(),
                summary.reason
            );
            let judgement_path = chat_root.join("JUDGEMENT.md");
            if let Err(e) = skilllite_fs::atomic_write(&judgement_path, &judgement_output) {
                tracing::warn!("Failed to write JUDGEMENT.md: {}", e);
            }
        }

        if all_changes.is_empty() {
            // 即使无变更也记录一次，便于前端时间线展示进化运行记录（含本轮选择的进化方向）
            let dir = scope.direction_label();
            let reason = if dir.is_empty() {
                "进化运行完成，无新规则/技能产出".to_string()
            } else {
                format!("方向: {}；进化运行完成，无新规则/技能产出", dir)
            };
            let _ = log_evolution_event(&conn, chat_root, "evolution_run", "run", &reason, &txn_id);
            let _ = set_backlog_status(
                &conn,
                &proposal.proposal_id,
                "executed",
                "not_met",
                "Executed with no material changes",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }

        let dir = scope.direction_label();
        let reason = if dir.is_empty() {
            reason_parts.join("; ")
        } else {
            format!("方向: {}；{}", dir, reason_parts.join("; "))
        };
        // 记录本轮进化运行（含方向），便于前端时间线统一展示
        let _ = log_evolution_event(&conn, chat_root, "evolution_run", "run", &reason, &txn_id);

        // 只记录内容真正发生变化的文件：用快照与当前版本逐一对比。
        // snapshot_files 是进化前备份的全量清单，但实际修改的往往只是其中一部分
        // （如 rules.json / examples.json），planning.md 等通常未被触碰。
        let snap_dir = versions_dir(chat_root).join(&txn_id);
        let prompts_dir = chat_root.join("prompts");
        let mut modified_files: Vec<String> = snapshot_files
            .iter()
            .filter(|fname| {
                let snap_path = snap_dir.join(fname);
                let curr_path = prompts_dir.join(fname);
                match (std::fs::read(&snap_path), std::fs::read(&curr_path)) {
                    (Ok(old), Ok(new)) => old != new,
                    _ => false,
                }
            })
            .cloned()
            .collect();

        // External learner writes to prompts/rules.json; include it when external merged/promoted rules but snapshot didn't cover it (e.g. no scope.prompts).
        if all_changes
            .iter()
            .any(|(t, _)| t == "external_rule_added" || t == "external_rule_promoted")
        {
            const EXTERNAL_RULES_FILE: &str = "rules.json";
            if !modified_files.iter().any(|f| f == EXTERNAL_RULES_FILE) {
                let rules_path = prompts_dir.join(EXTERNAL_RULES_FILE);
                if rules_path.exists() {
                    modified_files.push(EXTERNAL_RULES_FILE.to_string());
                }
            }
        }

        append_changelog(chat_root, &txn_id, &modified_files, &all_changes, &reason)?;

        let _decisions_path = chat_root.join("DECISIONS.md");
        // let _ = feedback::export_decisions_md(&conn, &decisions_path); // Removed for refactor
        let _ = set_backlog_status(
            &conn,
            &proposal.proposal_id,
            "executed",
            "pending_validation",
            "Execution completed; awaiting acceptance metrics window",
        );
        if let Err(e) = auto_link_acceptance_status(&conn, &proposal.proposal_id) {
            tracing::warn!(
                "Failed to auto-link acceptance status for proposal {}: {}",
                proposal.proposal_id,
                e
            );
        }

        tracing::info!("Evolution txn={} complete: {}", txn_id, reason);
    }

    Ok(EvolutionRunResult::Completed(Some(txn_id)))
}

pub fn query_changes_by_txn(conn: &Connection, txn_id: &str) -> Vec<(String, String)> {
    let mut stmt =
        match conn.prepare("SELECT type, target_id FROM evolution_log WHERE version = ?1") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
    stmt.query_map(params![txn_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })
    .ok()
    .into_iter()
    .flatten()
    .filter_map(|r| r.ok())
    .collect()
}

pub fn format_evolution_changes(changes: &[(String, String)]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|(change_type, id)| {
            let msg = match change_type.as_str() {
                "rule_added" => format!("\u{1f4a1} 已学习新规则: {}", id),
                "rule_updated" => format!("\u{1f504} 已优化规则: {}", id),
                "rule_retired" => format!("\u{1f5d1}\u{fe0f} 已退役低效规则: {}", id),
                "example_added" => format!("\u{1f4d6} 已新增示例: {}", id),
                "skill_generated" => format!("\u{2728} 已自动生成 Skill: {}", id),
                "skill_pending" => format!(
                    "\u{1f4a1} 新 Skill {} 待确认（运行 `skilllite evolution confirm {}` 加入）",
                    id, id
                ),
                "skill_refined" => format!("\u{1f527} 已优化 Skill: {}", id),
                "skill_retired" => format!("\u{1f4e6} 已归档 Skill: {}", id),
                "evolution_judgement" => {
                    let label = match id.as_str() {
                        "promote" => "保留",
                        "keep_observing" => "继续观察",
                        "rollback" => "回滚",
                        _ => id,
                    };
                    format!("\u{1f9ed} 本轮判断: {}", label)
                }
                "auto_rollback" => format!("\u{26a0}\u{fe0f} 检测到质量下降，已自动回滚: {}", id),
                "reusable_promoted" => format!("\u{2b06}\u{fe0f} 规则晋升为通用: {}", id),
                "reusable_demoted" => format!("\u{2b07}\u{fe0f} 规则降级为低效: {}", id),
                "external_rule_added" => format!("\u{1f310} 已从外部来源学习规则: {}", id),
                "external_rule_promoted" => format!("\u{2b06}\u{fe0f} 外部规则晋升为优质: {}", id),
                "source_paused" => format!("\u{23f8}\u{fe0f} 信源可达性过低，已暂停: {}", id),
                "source_retired" => format!("\u{1f5d1}\u{fe0f} 已退役低质量信源: {}", id),
                "source_discovered" => format!("\u{1f50d} 发现新信源: {}", id),
                "memory_knowledge_added" => format!("\u{1f4da} 已沉淀知识库（实体与关系）: {}", id),
                _ => return None,
            };
            Some(msg)
        })
        .collect()
}
