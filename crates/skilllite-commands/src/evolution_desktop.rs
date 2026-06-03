//! Desktop-shaped evolution JSON (L2 contract). See `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback;

use crate::evolution_status::resolve_workspace_root;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionBacklogRowSnapshot {
    pub proposal_id: String,
    pub source: String,
    pub risk_level: String,
    pub status: String,
    pub acceptance_status: String,
    pub roi_score: f64,
    pub updated_at: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionProposalStatusSnapshot {
    pub proposal_id: String,
    pub status: String,
    pub acceptance_status: String,
    pub updated_at: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSkillSnapshot {
    pub name: String,
    pub needs_review: bool,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionOpSnapshot {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub fn resolve_skills_root(workspace: &str) -> Result<PathBuf> {
    let root = resolve_workspace_root(workspace);
    skilllite_core::config::load_dotenv_from_dir(&root);
    let skills = resolve_skills_dir_with_legacy_fallback(&root, "skills").effective_path;
    if skills.is_dir() {
        Ok(skills)
    } else {
        Err(crate::Error::validation(format!(
            "skills directory not found under workspace: {}",
            root.display()
        )))
    }
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

pub fn query_backlog_desktop(limit: usize) -> Result<Vec<EvolutionBacklogRowSnapshot>> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root)?;
    let limit = limit.clamp(1, 200);
    let mut stmt = conn
        .prepare(
            "SELECT proposal_id, source, risk_level, status, acceptance_status, roi_score, updated_at, COALESCE(note, '')
         FROM evolution_backlog
         WHERE NOT (
           status = 'executed'
           AND COALESCE(acceptance_status, '') IN ('met', 'not_met')
         )
         ORDER BY updated_at DESC
         LIMIT ?1",
        )
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(EvolutionBacklogRowSnapshot {
                proposal_id: row.get(0)?,
                source: row.get(1)?,
                risk_level: row.get(2)?,
                status: row.get(3)?,
                acceptance_status: row.get(4)?,
                roi_score: row.get(5)?,
                updated_at: row.get(6)?,
                note: row.get(7)?,
            })
        })
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    rows.map(|r| r.map_err(|e| crate::Error::from(anyhow::Error::from(e))))
        .collect::<Result<Vec<_>>>()
}

pub fn query_proposal_status(proposal_id: &str) -> Result<EvolutionProposalStatusSnapshot> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root)?;
    conn.query_row(
        "SELECT proposal_id, status, acceptance_status, updated_at, note
         FROM evolution_backlog
         WHERE proposal_id = ?1
         LIMIT 1",
        [proposal_id],
        |row| {
            Ok(EvolutionProposalStatusSnapshot {
                proposal_id: row.get(0)?,
                status: row.get(1)?,
                acceptance_status: row.get(2)?,
                updated_at: row.get(3)?,
                note: row.get(4)?,
            })
        },
    )
    .map_err(|e| crate::Error::from(anyhow::Error::from(e)))
}

pub fn list_pending_skills(workspace: &str) -> Result<Vec<PendingSkillSnapshot>> {
    let skills_root = resolve_skills_root(workspace)?;
    Ok(
        skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root)
            .into_iter()
            .map(|(name, needs_review)| {
                let path = skills_root
                    .join("_evolved")
                    .join("_pending")
                    .join(&name)
                    .join("SKILL.md");
                let preview = std::fs::read_to_string(&path)
                    .map(|s| truncate_utf8(&s, 4000))
                    .unwrap_or_default();
                PendingSkillSnapshot {
                    name,
                    needs_review,
                    preview,
                }
            })
            .collect(),
    )
}

pub fn read_pending_skill_md(workspace: &str, skill_name: &str) -> Result<String> {
    let skill_name = skilllite_evolution::skill_synth::validate_pending_skill_name(skill_name)?;
    let skills_root = resolve_skills_root(workspace)?;
    let path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name)
        .join("SKILL.md");
    if !path.is_file() {
        return Err(crate::Error::validation(format!(
            "pending skill not found: {}",
            skill_name
        )));
    }
    std::fs::read_to_string(&path).map_err(Into::into)
}

pub fn confirm_pending_skill(workspace: &str, skill_name: &str) -> Result<()> {
    let skills_root = resolve_skills_root(workspace)?;
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)?;
    let chat_root = skilllite_core::paths::chat_root();
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &chat_root,
            "skill_confirmed",
            skill_name,
            "user confirmed (assistant)",
            "",
        );
    }
    Ok(())
}

pub fn reject_pending_skill(workspace: &str, skill_name: &str) -> Result<()> {
    let skills_root = resolve_skills_root(workspace)?;
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthorizeCapabilitySnapshot {
    pub proposal_id: String,
}

pub fn authorize_capability_evolution(
    workspace: &str,
    tool_name: &str,
    outcome: &str,
    summary: &str,
) -> Result<AuthorizeCapabilitySnapshot> {
    let _ = resolve_workspace_root(workspace);
    let chat_root = skilllite_core::paths::chat_root();
    let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root)?;
    let proposal_id =
        skilllite_evolution::enqueue_user_capability_evolution(&conn, tool_name, outcome, summary)?;
    let _ = skilllite_evolution::log_evolution_event(
        &conn,
        &chat_root,
        "capability_evolution_authorized",
        tool_name,
        &format!("outcome={}, proposal_id={}", outcome, proposal_id),
        workspace,
    );
    Ok(AuthorizeCapabilitySnapshot { proposal_id })
}

fn clip_manual_trigger_summary(summary: &str) -> String {
    truncate_utf8(summary, 480)
}

pub fn log_manual_evolution_trigger(
    workspace: &str,
    proposal_id: Option<&str>,
    summary: &str,
) -> Result<()> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root)?;
    let clipped = clip_manual_trigger_summary(summary);
    let _ = skilllite_evolution::log_evolution_event(
        &conn,
        &chat_root,
        "manual_evolution_run_triggered",
        proposal_id.unwrap_or("all"),
        &clipped,
        workspace,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_trigger_summary_clip_is_utf8_boundary_safe() {
        let mut summary = "界".repeat(159);
        summary.push('🙂');
        summary.push_str("tail");

        let clipped = clip_manual_trigger_summary(&summary);

        assert!(clipped.ends_with('…'));
        assert!(clipped.is_char_boundary(clipped.len()));
        assert!(clipped.len() <= 480 + "…".len());
    }

    #[test]
    fn manual_trigger_summary_clip_leaves_short_text_unchanged() {
        let summary = "Evolution completed: 新技能已生成";

        assert_eq!(clip_manual_trigger_summary(summary), summary);
    }
}
