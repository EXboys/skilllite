//! Skill synthesis: auto-generate, refine, and retire skills (EVO-4).
//!
//! - **Generate**: 既总结成功经验，也总结失败经验
//!   - 成功驱动：高成功率重复模式 → SKILL.md + script
//!   - 失败驱动：持续失败模式 → 补全能力缺口的 Skill
//! - **Refine**: failed skill → analyze error trace → LLM fix → retry (max 2 rounds)
//! - **Retire**: low success rate or unused skills → archive
//!
//! ## React / Check / Retry
//! 重度依赖大模型能力，每个环节都有校验与重试：
//! - **Check**: L3 内容门禁、L4 安全扫描、test_skill_invoke 实测
//! - **Retry**: 任何 LLM 输出 JSON 解析失败时，将错误反馈给大模型并重试 1 次
//! - 代码修改/修复仅由大模型完成，不使用正则或模式匹配
//!
//! All evolved skills live in `chat/skills/_evolved/` with `.meta.json` metadata.
//! A10: Newly generated skills go to `_evolved/_pending/` until user confirms.

mod env_helper;
mod generate;
mod infer;
mod parse;
mod query;
mod refine;
mod repair;
mod scan;
mod validate;

use std::collections::HashSet;
use std::path::Path;

use rusqlite::Connection;

use crate::error::bail;
use crate::Result;
use tokio::task::block_in_place;

use crate::EvolutionLlm;

// ─── Constants (shared across submodules) ────────────────────────────────────

pub(super) const SKILL_GENERATION_PROMPT: &str =
    include_str!("../seed/evolution_prompts/skill_generation.seed.md");
pub(super) const SKILL_GENERATION_FROM_FAILURES_PROMPT: &str =
    include_str!("../seed/evolution_prompts/skill_generation_from_failures.seed.md");
pub(super) const SKILL_REFINEMENT_PROMPT: &str =
    include_str!("../seed/evolution_prompts/skill_refinement.seed.md");
pub(super) const SKILL_EXECUTION_INFERENCE_PROMPT: &str =
    include_str!("../seed/evolution_prompts/skill_execution_inference.seed.md");

pub(super) const MAX_EVOLVED_SKILLS: usize = 20;
pub(super) const MAX_REFINE_ROUNDS: usize = 3;
pub(super) const MAX_PARSE_RETRIES: usize = 1;
pub(super) const RETIRE_UNUSED_DAYS: i64 = 30;
pub(super) const RETIRE_LOW_SUCCESS_RATE: f64 = 0.30;

// ─── Skill metadata ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub source_session: String,
    pub created_at: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub call_count: u32,
    pub last_used: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub generation_txn: String,
    #[serde(default)]
    pub needs_review: bool,
}

impl SkillMeta {
    pub fn success_rate(&self) -> f64 {
        if self.call_count == 0 {
            return 1.0;
        }
        self.success_count as f64 / self.call_count as f64
    }
}

// ─── Main entry: evolve skills ────────────────────────────────────────────────

/// Run skill evolution: generate new skills or refine existing ones.
pub async fn evolve_skills<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: Option<&Path>,
    llm: &L,
    model: &str,
    txn_id: &str,
    generate: bool,
    force: bool,
) -> Result<Vec<(String, String)>> {
    let Some(skills_root) = skills_root else {
        return Ok(Vec::new());
    };
    let mut changes = Vec::new();

    let try_generate = generate || force;
    let min_pattern_count: u32 = std::env::var("SKILLLITE_MIN_PATTERN_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if force { 2 } else { 3 });

    if try_generate {
        // 单次 conn 预取成功/失败数据并执行 retire，减少 DB 打开次数
        let (success_data, failure_data, retired) = block_in_place(|| {
            let conn = crate::feedback::open_evolution_db(chat_root)?;
            let (patterns_display, pattern_descs) =
                query::query_repeated_patterns(&conn, min_pattern_count)?;
            let success_executions = if pattern_descs.is_empty() {
                String::new()
            } else {
                query::query_pattern_executions(&conn, &pattern_descs)?
            };
            let failed_patterns = query::query_failed_patterns(&conn, 2)?;
            let failed_executions = query::query_failed_executions(&conn)?;
            let retired = refine::retire_skills_with_conn(chat_root, skills_root, txn_id, &conn)?;
            Ok::<_, anyhow::Error>((
                (patterns_display, success_executions),
                (failed_patterns, failed_executions),
                retired,
            ))
        })?;
        changes.extend(retired);
        if let Ok(Some(name)) = generate::generate_skill_from_failures(
            chat_root,
            skills_root,
            llm,
            model,
            txn_id,
            Some(failure_data),
        )
        .await
        {
            changes.push(("skill_pending".to_string(), name));
        }
        if let Ok(Some(name)) = generate::generate_skill(
            chat_root,
            skills_root,
            llm,
            model,
            txn_id,
            min_pattern_count,
            Some(success_data),
        )
        .await
        {
            changes.push(("skill_pending".to_string(), name));
        }
        if changes.is_empty() {
            if let Ok(Some(name)) =
                refine::refine_weakest_skill(chat_root, skills_root, llm, model, txn_id).await
            {
                changes.push(("skill_refined".to_string(), name));
            }
        }
    } else {
        let (retired, _) = block_in_place(|| {
            let conn = crate::feedback::open_evolution_db(chat_root)?;
            let retired = refine::retire_skills_with_conn(chat_root, skills_root, txn_id, &conn)?;
            Ok::<_, anyhow::Error>((retired, ()))
        })?;
        changes.extend(retired);
        match refine::refine_weakest_skill(chat_root, skills_root, llm, model, txn_id).await {
            Ok(Some(name)) => changes.push(("skill_refined".to_string(), name)),
            Ok(None) => {}
            Err(e) => tracing::warn!("Skill refinement failed: {}", e),
        }
    }

    // 同轮内名称去重：同一 name 的 skill_pending / skill_refined 只保留首次出现
    let mut seen: HashSet<String> = HashSet::new();
    changes.retain(|(t, id)| {
        if t == "skill_pending" || t == "skill_refined" {
            seen.insert(id.clone())
        } else {
            true
        }
    });

    Ok(changes)
}

/// Decision ids whose rows are read for skill generation inputs (repeated successes + recent failures).
pub(crate) fn decision_ids_read_for_skill_evolution(
    conn: &Connection,
    try_generate: bool,
    force: bool,
) -> Result<Vec<i64>> {
    if !try_generate {
        return Ok(Vec::new());
    }
    let min_pattern_count: u32 = std::env::var("SKILLLITE_MIN_PATTERN_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if force { 2 } else { 3 });
    let mut acc: HashSet<i64> = HashSet::new();
    let (_, pattern_descs) = query::query_repeated_patterns(conn, min_pattern_count)?;
    for id in query::query_pattern_execution_ids(conn, &pattern_descs)? {
        acc.insert(id);
    }
    for id in query::query_failed_execution_ids(conn)? {
        acc.insert(id);
    }
    Ok(acc.into_iter().collect())
}

// ─── A10: Pending skill confirmation ─────────────────────────────────────────

pub fn list_pending_skills(skills_root: &Path) -> Vec<String> {
    list_pending_skills_with_review(skills_root)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

pub fn list_pending_skills_with_review(skills_root: &Path) -> Vec<(String, bool)> {
    let pending_dir = skills_root.join("_evolved").join("_pending");
    if !pending_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&pending_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let needs_review = std::fs::read_to_string(e.path().join(".meta.json"))
                .ok()
                .and_then(|s| serde_json::from_str::<SkillMeta>(&s).ok())
                .map(|m| m.needs_review)
                .unwrap_or(false);
            (name, needs_review)
        })
        .collect()
}

pub fn confirm_pending_skill(skills_root: &Path, skill_name: &str) -> Result<()> {
    let pending_dir = skills_root.join("_evolved").join("_pending");
    let evolved_dir = skills_root.join("_evolved");
    let src = pending_dir.join(skill_name);
    let dst = evolved_dir.join(skill_name);

    if !src.exists() {
        bail!("待确认 Skill '{}' 不存在", skill_name);
    }
    if dst.exists() {
        bail!("Skill '{}' 已存在，请先删除或重命名", skill_name);
    }

    std::fs::rename(&src, &dst)?;
    tracing::info!("Skill '{}' 已确认加入", skill_name);
    Ok(())
}

pub fn reject_pending_skill(skills_root: &Path, skill_name: &str) -> Result<()> {
    let pending_dir = skills_root.join("_evolved").join("_pending");
    let src = pending_dir.join(skill_name);

    if !src.exists() {
        bail!("待确认 Skill '{}' 不存在", skill_name);
    }

    std::fs::remove_dir_all(&src)?;
    tracing::info!("Skill '{}' 已拒绝", skill_name);
    Ok(())
}

// ─── Re-exports ──────────────────────────────────────────────────────────────

pub use repair::{repair_one_skill, repair_skills};
pub use scan::track_skill_usage;
pub use validate::{validate_skills, SkillValidation};
