//! User-authorized capability evolution: L2 enqueue + background `evolution run`.

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::local::engine_types::AuthorizeCapabilityResponse;
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};

fn authorized_evolution_run_args(proposal_id: &str) -> Vec<&str> {
    vec!["evolution", "run", "--json", "--proposal-id", proposal_id]
}

pub fn authorize_capability_evolution(
    workspace: &str,
    tool_name: &str,
    outcome: &str,
    summary: &str,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let snap: AuthorizeCapabilityResponse = spawn_skilllite_json(
        skilllite_path,
        workspace,
        None,
        &[
            "evolution",
            "authorize-capability",
            "--json",
            "--workspace",
            workspace,
            "--tool-name",
            tool_name,
            "--outcome",
            outcome,
            "--summary",
            summary,
        ],
    )?;
    let proposal_id = snap.proposal_id.clone();
    let workspace_owned = workspace.to_string();
    let proposal_id_owned = proposal_id.clone();
    let skilllite_path_owned = skilllite_path.to_path_buf();
    std::thread::spawn(move || {
        let root = find_project_root(&workspace_owned);
        let mut cmd = std::process::Command::new(&skilllite_path_owned);
        crate::windows_spawn::hide_child_console(&mut cmd);
        cmd.args(authorized_evolution_run_args(&proposal_id_owned))
            .current_dir(&root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in load_dotenv_for_child(&workspace_owned) {
            cmd.env(k, v);
        }
        let _ = cmd.output();
    });
    Ok(proposal_id)
}

#[cfg(test)]
mod tests {
    use super::authorized_evolution_run_args;

    #[test]
    fn authorize_background_run_args_force_proposal() {
        assert_eq!(
            authorized_evolution_run_args("proposal_20260604_110000.000"),
            vec![
                "evolution",
                "run",
                "--json",
                "--proposal-id",
                "proposal_20260604_110000.000"
            ]
        );
    }
}
