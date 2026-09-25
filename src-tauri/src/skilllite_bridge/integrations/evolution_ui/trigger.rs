//! Manual evolution trigger via `skilllite evolution run --json` (L2).

use crate::skilllite_bridge::local::engine_types::NodeResult;

use crate::skilllite_bridge::chat::ChatConfigOverrides;
use crate::skilllite_bridge::evolution_cli::spawn_skilllite_json;

pub fn trigger_evolution_run(
    workspace: &str,
    proposal_id: Option<&str>,
    skilllite_path: &std::path::Path,
    overrides: Option<ChatConfigOverrides>,
) -> Result<String, String> {
    let mut args: Vec<String> = vec![
        "evolution".into(),
        "run".into(),
        "--json".into(),
        "--log-manual-trigger".into(),
        "--workspace".into(),
        workspace.to_string(),
    ];
    if let Some(pid) = proposal_id.filter(|s| !s.trim().is_empty()) {
        args.push("--proposal-id".into());
        args.push(pid.to_string());
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let result: NodeResult = spawn_skilllite_json(skilllite_path, workspace, overrides.as_ref(), &arg_refs)?;
    Ok(result.response)
}
