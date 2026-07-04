//! Evolution UI bridge — all data via `skilllite … --json` (L2).

mod authorize;
mod backlog;
mod growth;
mod pending;
mod status;
mod trigger;

fn with_workspace_arg(mut args: Vec<String>, workspace: &str) -> Vec<String> {
    args.push("--workspace".to_string());
    args.push(crate::skilllite_bridge::canonical_workspace_arg(workspace));
    args
}

fn arg_refs(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

pub use authorize::authorize_capability_evolution;
pub use backlog::{
    get_evolution_proposal_status, load_evolution_backlog, EvolutionBacklogRowDto,
    EvolutionProposalStatusDto,
};
pub use growth::evolution_growth_due;
pub use pending::{
    evolution_confirm_pending_skill, evolution_reject_pending_skill, list_evolution_pending_skills,
    read_evolution_pending_skill_md, PendingSkillDto,
};
pub use status::{load_evolution_status, EvolutionStatusPayload};
pub use trigger::trigger_evolution_run;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn with_workspace_arg_canonicalizes_nested_workspace() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!(
            "skilllite_evolution_ui_workspace_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let nested = tmp.join("apps").join("frontend");
        std::fs::create_dir_all(tmp.join("skills")).expect("skills root");
        std::fs::create_dir_all(&nested).expect("nested workspace");

        let args = with_workspace_arg(
            vec!["evolution".to_string(), "status".to_string()],
            nested.to_string_lossy().as_ref(),
        );

        assert_eq!(args[2], "--workspace");
        assert_eq!(
            PathBuf::from(&args[3]).canonicalize().expect("workspace"),
            tmp.canonicalize().expect("tmp")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
