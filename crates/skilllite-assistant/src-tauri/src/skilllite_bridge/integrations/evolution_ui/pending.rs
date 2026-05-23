//! Pending evolved skills: list, read, confirm, reject (L2 CLI first).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::integrations::shared::{
    existing_workspace_skills_root, resolve_workspace_skills_root,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSkillDto {
    pub name: String,
    pub needs_review: bool,
    pub preview: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EvolutionOpDto {
    #[allow(dead_code)]
    ok: bool,
    #[allow(dead_code)]
    message: Option<String>,
}

pub fn list_evolution_pending_skills(
    workspace: &str,
    skilllite_path: Option<&Path>,
) -> Vec<PendingSkillDto> {
    if let Some(path) = skilllite_path {
        if let Ok(rows) = spawn_skilllite_json::<Vec<PendingSkillDto>>(
            path,
            workspace,
            None,
            &["evolution", "pending", "--json", "--workspace", workspace],
        ) {
            return rows;
        }
    }
    list_evolution_pending_skills_inprocess(workspace)
}

fn list_evolution_pending_skills_inprocess(workspace: &str) -> Vec<PendingSkillDto> {
    let Some(skills_root) = existing_workspace_skills_root(workspace) else {
        return Vec::new();
    };
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
            PendingSkillDto {
                name,
                needs_review,
                preview,
            }
        })
        .collect()
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

pub fn read_evolution_pending_skill_md(
    workspace: &str,
    skill_name: &str,
) -> Result<String, String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    let path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name)
        .join("SKILL.md");
    if !path.is_file() {
        return Err(format!("未找到待审核技能: {}", skill_name));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn evolution_confirm_pending_skill(
    workspace: &str,
    skill_name: &str,
    skilllite_path: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = skilllite_path {
        if let Ok(_op) = spawn_skilllite_json::<EvolutionOpDto>(
            path,
            workspace,
            None,
            &[
                "evolution",
                "confirm",
                "--json",
                "--workspace",
                workspace,
                skill_name,
            ],
        ) {
            return Ok(());
        }
    }
    evolution_confirm_pending_skill_inprocess(workspace, skill_name)
}

fn evolution_confirm_pending_skill_inprocess(
    workspace: &str,
    skill_name: &str,
) -> Result<(), String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())?;
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

pub fn evolution_reject_pending_skill(
    workspace: &str,
    skill_name: &str,
    skilllite_path: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = skilllite_path {
        if let Ok(_op) = spawn_skilllite_json::<EvolutionOpDto>(
            path,
            workspace,
            None,
            &[
                "evolution",
                "reject",
                "--json",
                "--workspace",
                workspace,
                skill_name,
            ],
        ) {
            return Ok(());
        }
    }
    evolution_reject_pending_skill_inprocess(workspace, skill_name)
}

fn evolution_reject_pending_skill_inprocess(
    workspace: &str,
    skill_name: &str,
) -> Result<(), String> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())
}
