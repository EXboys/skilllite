//! Pending evolved skills (L2 CLI only).

use std::path::{Component, Path};

use serde::{Deserialize, Serialize};

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::integrations::shared::resolve_workspace_skills_root;

/// Pending skill directory names must be a single normal path segment.
/// Mirrors `skilllite_evolution::skill_synth::validate_pending_skill_name` (PR #89)
/// for the Tauri direct-read path that does not go through that crate.
fn validate_pending_skill_name(skill_name: &str) -> Result<&str, String> {
    if skill_name.trim().is_empty() || skill_name.chars().any(|c| matches!(c, '/' | '\\' | '\0')) {
        return Err(format!("invalid pending skill name: {}", skill_name));
    }
    let mut components = Path::new(skill_name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(skill_name),
        _ => Err(format!("invalid pending skill name: {}", skill_name)),
    }
}

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
    let skill_name = validate_pending_skill_name(skill_name)?;
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

#[cfg(test)]
mod pending_name_tests {
    use super::validate_pending_skill_name;

    #[test]
    fn accepts_single_segment_names() {
        assert_eq!(
            validate_pending_skill_name("safe-skill").unwrap(),
            "safe-skill"
        );
        assert_eq!(validate_pending_skill_name("报告技能").unwrap(), "报告技能");
    }

    #[test]
    fn rejects_absolute_and_traversal_names() {
        assert!(validate_pending_skill_name("/tmp/outside_pending").is_err());
        assert!(validate_pending_skill_name("../../../../outside_pending").is_err());
        assert!(validate_pending_skill_name("a/b").is_err());
        assert!(validate_pending_skill_name(r"a\b").is_err());
        assert!(validate_pending_skill_name("").is_err());
        assert!(validate_pending_skill_name(".").is_err());
        assert!(validate_pending_skill_name("..").is_err());
    }
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
