//! Life Pulse: whether to spawn `skilllite evolution run` (L2 status JSON only).

use std::path::Path;
use std::sync::Mutex;

use crate::skilllite_bridge::chat::ChatConfigOverrides;

use super::status::load_evolution_status;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn next_periodic_anchor(
    current: Option<i64>,
    now: i64,
    growth_tick_would_be_due: bool,
    arm_periodic: bool,
) -> Option<i64> {
    if current.is_none() || (growth_tick_would_be_due && arm_periodic) {
        Some(now)
    } else {
        current
    }
}

pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
    skilllite_path: &Path,
) -> bool {
    let now = now_unix();
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
    let next_anchor = next_periodic_anchor(
        anchor,
        now,
        a9.growth_tick_would_be_due,
        a9.arm_periodic,
    );
    if next_anchor != anchor {
        if let Ok(mut guard) = last_periodic_spawn_unix.lock() {
            *guard = next_anchor;
        }
    }
    if !a9.growth_tick_would_be_due {
        return false;
    }
    if a9.periodic_only && !status.would_have_evolution_proposals {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_anchor_initializes_on_first_successful_check() {
        assert_eq!(next_periodic_anchor(None, 100, false, false), Some(100));
    }

    #[test]
    fn periodic_anchor_advances_when_periodic_arm_fires() {
        assert_eq!(
            next_periodic_anchor(Some(10), 100, true, true),
            Some(100)
        );
    }

    #[test]
    fn periodic_anchor_does_not_advance_for_signal_only_due() {
        assert_eq!(
            next_periodic_anchor(Some(10), 100, true, false),
            Some(10)
        );
    }
}
