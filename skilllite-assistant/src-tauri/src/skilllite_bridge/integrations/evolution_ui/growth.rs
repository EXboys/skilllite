//! Life Pulse: whether to spawn `skilllite evolution run` (L2 status JSON only).

use std::path::Path;
use std::sync::Mutex;

use crate::skilllite_bridge::chat::ChatConfigOverrides;

use super::status::load_evolution_status;

pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
    skilllite_path: &Path,
) -> bool {
    let anchor = last_periodic_spawn_unix
        .lock()
        .ok()
        .and_then(|g| *g);
    let status = match load_evolution_status(workspace, cfg.cloned(), anchor, skilllite_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[life-pulse] evolution status: {}", e);
            return false;
        }
    };
    if status.mode_key == "disabled" {
        return false;
    }
    let Some(a9) = status.a9 else {
        return false;
    };
    if !a9.growth_tick_would_be_due {
        return false;
    }
    if a9.periodic_only && !status.would_have_evolution_proposals {
        return false;
    }
    true
}
