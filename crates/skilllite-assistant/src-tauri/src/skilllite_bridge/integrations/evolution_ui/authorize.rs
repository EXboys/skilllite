//! User-authorized capability evolution: L2 enqueue + background `evolution run`.

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::local::engine_types::AuthorizeCapabilityResponse;
use crate::skilllite_bridge::local::env_keys::evolution as evo_keys;
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};

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
        cmd.arg("evolution")
            .arg("run")
            .arg("--json")
            .arg("--workspace")
            .arg(&workspace_owned)
            .current_dir(&root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in load_dotenv_for_child(&workspace_owned) {
            cmd.env(k, v);
        }
        cmd.env(evo_keys::SKILLLITE_EVO_FORCE_PROPOSAL_ID, &proposal_id_owned);
        let _ = cmd.output();
    });
    Ok(proposal_id)
}
