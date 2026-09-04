//! Life Pulse: whether to spawn `skilllite evolution run` (L2 status JSON only).

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::skilllite_bridge::chat::ChatConfigOverrides;

use super::status::load_evolution_status;

pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
    skilllite_path: &Path,
) -> bool {
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let anchor = last_periodic_spawn_unix
        .lock()
        .map(|g| *g)
        .unwrap_or_else(|e| *e.into_inner());
    let status = match load_evolution_status(workspace, cfg.cloned(), anchor, skilllite_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[life-pulse] evolution status: {}", e);
            return false;
        }
    };
    let arm_periodic = status.a9.as_ref().is_some_and(|a9| a9.arm_periodic);
    {
        let mut guard = last_periodic_spawn_unix
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *guard = next_periodic_anchor(*guard, now_unix, arm_periodic);
    }
    let Some(a9) = status.a9.as_ref() else {
        return false;
    };
    should_spawn_growth(
        &status.mode_key,
        a9.growth_tick_would_be_due,
        a9.periodic_only,
        status.would_have_evolution_proposals,
    )
}

/// Mirror `growth_due`'s mutex rules: seed a missing anchor to `now`, and
/// refresh it when the periodic arm fires (including skipped no-proposal ticks).
fn next_periodic_anchor(current: Option<i64>, now_unix: i64, arm_periodic: bool) -> Option<i64> {
    if current.is_none() || arm_periodic {
        Some(now_unix)
    } else {
        current
    }
}

fn should_spawn_growth(
    mode_key: &str,
    growth_tick_would_be_due: bool,
    periodic_only: bool,
    would_have_evolution_proposals: bool,
) -> bool {
    if mode_key == "disabled" {
        return false;
    }
    if !growth_tick_would_be_due {
        return false;
    }
    if periodic_only && !would_have_evolution_proposals {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_periodic_anchor_seeds_missing_anchor_so_elapsed_can_accumulate() {
        let t0 = 1_700_000_000;
        assert_eq!(next_periodic_anchor(None, t0, false), Some(t0));
        assert_eq!(next_periodic_anchor(Some(t0), t0 + 30, false), Some(t0));
    }

    #[test]
    fn next_periodic_anchor_refreshes_when_periodic_arm_fires() {
        let t0 = 1_700_000_000;
        let t1 = t0 + 600;
        assert_eq!(next_periodic_anchor(Some(t0), t1, true), Some(t1));
    }

    #[test]
    fn next_periodic_anchor_keeps_existing_on_signal_or_sweep_only() {
        let t0 = 1_700_000_000;
        assert_eq!(next_periodic_anchor(Some(t0), t0 + 120, false), Some(t0));
    }

    #[test]
    fn should_spawn_growth_skips_disabled_and_not_due() {
        assert!(!should_spawn_growth("disabled", true, false, true));
        assert!(!should_spawn_growth("passive", false, false, true));
    }

    #[test]
    fn should_spawn_growth_skips_periodic_only_without_proposals() {
        assert!(!should_spawn_growth("passive", true, true, false));
        assert!(should_spawn_growth("passive", true, true, true));
    }

    #[test]
    fn should_spawn_growth_allows_signal_or_sweep_without_proposals() {
        assert!(should_spawn_growth("passive", true, false, false));
    }
}
