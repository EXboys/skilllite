//! Pending evolved skills (L2 CLI only).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::integrations::shared::resolve_workspace_skills_root;

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
    skilllite_path: &Path,
) -> Result<Vec<PendingSkillDto>, String> {
    spawn_skilllite_json(
        skilllite_path,
        workspace,
        None,
        &["evolution", "pending", "--json", "--workspace", workspace],
    )
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
    skilllite_path: &Path,
) -> Result<(), String> {
    let _op: EvolutionOpDto = spawn_skilllite_json(
        skilllite_path,
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
    )?;
    Ok(())
}

pub fn evolution_reject_pending_skill(
    workspace: &str,
    skill_name: &str,
    skilllite_path: &Path,
) -> Result<(), String> {
    let _op: EvolutionOpDto = spawn_skilllite_json(
        skilllite_path,
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
    )?;
    Ok(())
}
