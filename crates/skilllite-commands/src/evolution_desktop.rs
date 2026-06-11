//! Desktop-shaped evolution JSON (L2 contract). See `docs/en/ASSISTANT-SPLIT-ARCHITECTURE.md`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback;

use crate::evolution_status::{chat_root_for_workspace, resolve_workspace_root};
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

pub fn query_backlog_desktop(
    workspace: &str,
    limit: usize,
) -> Result<Vec<EvolutionBacklogRowSnapshot>> {
    let chat_root = chat_root_for_workspace(workspace);
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

pub fn query_proposal_status(
    workspace: &str,
    proposal_id: &str,
) -> Result<EvolutionProposalStatusSnapshot> {
    let chat_root = chat_root_for_workspace(workspace);
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
    let chat_root = chat_root_for_workspace(workspace);
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
    let workspace_root = resolve_workspace_root(workspace);
    let chat_root = workspace_root.join("chat");
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
    let chat_root = chat_root_for_workspace(workspace);
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
    use skilllite_core::config::env_keys::paths as env_paths;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            skilllite_core::config::set_env_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                skilllite_core::config::set_env_var(self.key, value);
            } else {
                skilllite_core::config::remove_env_var(self.key);
            }
        }
    }

    fn temp_workspace(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "skilllite-evo-desktop-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    fn seed_backlog_row(workspace: &std::path::Path, proposal_id: &str, note: &str) {
        let chat_root = workspace.join("chat");
        let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root).expect("open db");
        let dedupe_key = format!("dedupe_{proposal_id}");
        conn.execute(
            "INSERT INTO evolution_backlog
             (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, acceptance_status, note)
             VALUES (?1, 'active', ?2, '{}', 'low', 0.5, 0.5, 1.0, '[]', 'queued', 'pending_validation', ?3)",
            [proposal_id, dedupe_key.as_str(), note],
        )
        .expect("insert backlog row");
    }

    fn capability_rows(workspace: &std::path::Path, tool_name: &str) -> i64 {
        let chat_root = workspace.join("chat");
        let conn = skilllite_evolution::feedback::open_evolution_db(&chat_root).expect("open db");
        let dedupe_key = format!("user_capability:{tool_name}:failure");
        conn.query_row(
            "SELECT COUNT(*) FROM evolution_backlog WHERE dedupe_key = ?1",
            [dedupe_key.as_str()],
            |row| row.get(0),
        )
        .expect("count rows")
    }

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

    #[test]
    fn query_backlog_desktop_uses_workspace_argument_over_env() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let env_workspace = temp_workspace("env");
        let target_workspace = temp_workspace("target");
        let _env_restore = EnvRestore::set(
            env_paths::SKILLLITE_WORKSPACE,
            env_workspace.to_string_lossy().as_ref(),
        );
        seed_backlog_row(&env_workspace, "env_only", "ENV_DB_ROW");
        seed_backlog_row(&target_workspace, "target_only", "TARGET_DB_ROW");

        let rows =
            query_backlog_desktop(target_workspace.to_string_lossy().as_ref(), 10).expect("query");

        let ids: Vec<_> = rows.into_iter().map(|row| row.proposal_id).collect();
        assert_eq!(ids, vec!["target_only"]);
        let _ = std::fs::remove_dir_all(env_workspace);
        let _ = std::fs::remove_dir_all(target_workspace);
    }

    #[test]
    fn query_proposal_status_uses_workspace_argument_over_env() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let env_workspace = temp_workspace("env");
        let target_workspace = temp_workspace("target");
        let _env_restore = EnvRestore::set(
            env_paths::SKILLLITE_WORKSPACE,
            env_workspace.to_string_lossy().as_ref(),
        );
        seed_backlog_row(&env_workspace, "shared_id", "ENV_DB_ROW");
        seed_backlog_row(&target_workspace, "shared_id", "TARGET_DB_ROW");

        let row = query_proposal_status(target_workspace.to_string_lossy().as_ref(), "shared_id")
            .expect("query");

        assert_eq!(row.note.as_deref(), Some("TARGET_DB_ROW"));
        let _ = std::fs::remove_dir_all(env_workspace);
        let _ = std::fs::remove_dir_all(target_workspace);
    }

    #[test]
    fn authorize_capability_uses_workspace_argument_over_env() {
        let _lock = ENV_LOCK.lock().expect("env lock");
        let env_workspace = temp_workspace("env");
        let target_workspace = temp_workspace("target");
        let tool_name = "workspace_scope_regression";
        let _env_restore = EnvRestore::set(
            env_paths::SKILLLITE_WORKSPACE,
            env_workspace.to_string_lossy().as_ref(),
        );
        let _ = skilllite_evolution::feedback::open_evolution_db(&env_workspace.join("chat"))
            .expect("open env db");
        let _ = skilllite_evolution::feedback::open_evolution_db(&target_workspace.join("chat"))
            .expect("open target db");

        authorize_capability_evolution(
            target_workspace.to_string_lossy().as_ref(),
            tool_name,
            "failure",
            "summary",
        )
        .expect("authorize");

        assert_eq!(capability_rows(&env_workspace, tool_name), 0);
        assert_eq!(capability_rows(&target_workspace, tool_name), 1);
        let _ = std::fs::remove_dir_all(env_workspace);
        let _ = std::fs::remove_dir_all(target_workspace);
    }
}
