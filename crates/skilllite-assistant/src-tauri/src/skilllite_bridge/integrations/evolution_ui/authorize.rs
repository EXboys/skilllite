//! User-authorized capability evolution: L2 enqueue + background `evolution run`.

use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;
use crate::skilllite_bridge::local::engine_types::AuthorizeCapabilityResponse;
use crate::skilllite_bridge::local::env_keys::evolution as evo_keys;
use crate::skilllite_bridge::paths::{find_project_root, load_dotenv_for_child};

use super::{arg_refs, with_workspace_arg};

fn authorized_evolution_run_args(workspace: &str) -> Vec<String> {
    with_workspace_arg(
        vec![
            "evolution".to_string(),
            "run".to_string(),
            "--json".to_string(),
        ],
        workspace,
    )
}

pub fn authorize_capability_evolution(
    workspace: &str,
    tool_name: &str,
    outcome: &str,
    summary: &str,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let mut args = with_workspace_arg(
        vec![
            "evolution".to_string(),
            "authorize-capability".to_string(),
            "--json".to_string(),
        ],
        workspace,
    );
    args.extend([
        "--tool-name".to_string(),
        tool_name.to_string(),
        "--outcome".to_string(),
        outcome.to_string(),
        "--summary".to_string(),
        summary.to_string(),
    ]);
    let arg_refs = arg_refs(&args);
    let snap: AuthorizeCapabilityResponse =
        spawn_skilllite_json(skilllite_path, workspace, None, &arg_refs)?;
    let proposal_id = snap.proposal_id.clone();
    let workspace_owned = workspace.to_string();
    let proposal_id_owned = proposal_id.clone();
    let skilllite_path_owned = skilllite_path.to_path_buf();
    std::thread::spawn(move || {
        let root = find_project_root(&workspace_owned);
        let args = authorized_evolution_run_args(&workspace_owned);
        let mut cmd = std::process::Command::new(&skilllite_path_owned);
        crate::windows_spawn::hide_child_console(&mut cmd);
        cmd.args(&args)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_run_args_include_canonical_workspace() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let tmp = std::env::temp_dir().join(format!(
            "skilllite_authorized_evolution_workspace_{}_{}",
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        let nested = tmp.join("apps").join("frontend");
        std::fs::create_dir_all(tmp.join("skills")).expect("skills root");
        std::fs::create_dir_all(&nested).expect("nested workspace");

        let args = authorized_evolution_run_args(nested.to_string_lossy().as_ref());

        assert_eq!(
            &args[..4],
            ["evolution", "run", "--json", "--workspace"]
        );
        assert_eq!(
            std::path::PathBuf::from(&args[4])
                .canonicalize()
                .expect("workspace"),
            tmp.canonicalize().expect("tmp")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
