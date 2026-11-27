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
    let min_pattern_count: u32 =
        std::env::var(skilllite_core::config::env_keys::evolution::SKILLLITE_MIN_PATTERN_COUNT)
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
    let min_pattern_count: u32 =
        std::env::var(skilllite_core::config::env_keys::evolution::SKILLLITE_MIN_PATTERN_COUNT)
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

/// A pending skill is one directory directly under `_evolved/_pending`.
///
/// `Path::join` replaces the base when `skill_name` is absolute, and `..`
/// walks out of `_pending`. Either form would make confirm rename, or reject
/// delete, a directory outside the pending root.
fn require_pending_skill_dir_name(skill_name: &str) -> Result<()> {
    let path = Path::new(skill_name);
    if path.is_absolute() {
        bail!("待确认 Skill 名称必须是单个目录名");
    }
    let mut components = path.components();
    let Some(std::path::Component::Normal(part)) = components.next() else {
        bail!("待确认 Skill 名称必须是单个目录名");
    };
    if components.next().is_some() || part != std::ffi::OsStr::new(skill_name) {
        bail!("待确认 Skill 名称必须是单个目录名");
    }
    Ok(())
}

pub fn confirm_pending_skill(skills_root: &Path, skill_name: &str) -> Result<()> {
    require_pending_skill_dir_name(skill_name)?;
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
    require_pending_skill_dir_name(skill_name)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn pending_layout() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let root = tempfile::tempdir().expect("tempdir");
        let skills = root.path().join("workspace").join("project").join("skills");
        fs::create_dir_all(skills.join("_evolved").join("_pending")).expect("pending dir");
        let victim = root.path().join("workspace").join("victim");
        fs::create_dir_all(&victim).expect("victim dir");
        fs::write(victim.join("keep.txt"), "保留 中文").expect("marker");
        (root, skills, victim)
    }

    #[test]
    fn reject_pending_skill_does_not_delete_outside_the_pending_directory() {
        let (_root, skills, victim) = pending_layout();
        let absolute = victim.to_str().expect("utf8 victim path");
        let err = reject_pending_skill(&skills, absolute).expect_err("absolute name");
        assert!(
            err.to_string().contains("单个目录名"),
            "unexpected error: {err}"
        );
        assert_eq!(
            fs::read_to_string(victim.join("keep.txt")).expect("marker remains"),
            "保留 中文"
        );

        let err = reject_pending_skill(&skills, "../../../../victim").expect_err("relative name");
        assert!(
            err.to_string().contains("单个目录名"),
            "unexpected error: {err}"
        );
        assert_eq!(
            fs::read_to_string(victim.join("keep.txt")).expect("marker remains"),
            "保留 中文"
        );
    }

    #[test]
    fn confirm_pending_skill_does_not_move_a_directory_outside_pending() {
        let (root, skills, victim) = pending_layout();
        let err = confirm_pending_skill(&skills, "../../../../victim").expect_err("relative name");
        assert!(
            err.to_string().contains("单个目录名"),
            "unexpected error: {err}"
        );
        assert_eq!(
            fs::read_to_string(victim.join("keep.txt")).expect("marker remains"),
            "保留 中文"
        );
        assert!(!root.path().join("victim").exists());
    }

    #[test]
    fn confirm_and_reject_keep_a_normal_pending_skill_name() {
        let (_root, skills, _victim) = pending_layout();
        let pending = skills.join("_evolved").join("_pending").join("日程");
        fs::create_dir_all(&pending).expect("pending skill");
        fs::write(pending.join("SKILL.md"), "---\nname: 日程\n---\n").expect("skill md");
        confirm_pending_skill(&skills, "日程").expect("confirm");
        let confirmed = skills.join("_evolved").join("日程").join("SKILL.md");
        assert_eq!(
            fs::read_to_string(&confirmed).expect("confirmed skill"),
            "---\nname: 日程\n---\n"
        );
        assert!(!pending.exists());

        let notes = skills.join("_evolved").join("_pending").join("notes");
        let sibling = skills.join("_evolved").join("_pending").join("notes-extra");
        fs::create_dir_all(&notes).expect("notes");
        fs::create_dir_all(&sibling).expect("sibling");
        fs::write(sibling.join("keep.txt"), "保留 中文").expect("sibling marker");
        reject_pending_skill(&skills, "notes").expect("reject");
        assert!(!notes.exists());
        assert_eq!(
            fs::read_to_string(sibling.join("keep.txt")).expect("sibling remains"),
            "保留 中文"
        );

        for name in ["", ".", "..", "foo/bar", "日程/"] {
            let err = reject_pending_skill(&skills, name).expect_err(name);
            assert!(err.to_string().contains("单个目录名"), "{name}: {err}");
            assert!(sibling.join("keep.txt").is_file(), "{name}");
        }
    }
}
