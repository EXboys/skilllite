//! Life Pulse: whether to spawn `skilllite evolution run` (L2 status JSON only).

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::skilllite_bridge::chat::ChatConfigOverrides;

use super::status::load_evolution_status;

/// Sync Life Pulse periodic-anchor mutex to match [`skilllite_evolution::growth_schedule::growth_due`].
///
/// - `None` → seed to `now_unix` (first-tick elapsed stays 0).
/// - After a due tick that includes the periodic arm → advance to `now_unix`.
///
/// Returns the anchor that must be passed into status inspection for this heartbeat.
pub(crate) fn prepare_periodic_anchor(
    last_periodic_spawn_unix: &mut Option<i64>,
    now_unix: i64,
) -> Option<i64> {
    if last_periodic_spawn_unix.is_none() {
        *last_periodic_spawn_unix = Some(now_unix);
    }
    *last_periodic_spawn_unix
}

/// Advance the periodic arm after a due tick, matching mutating `growth_due`.
pub(crate) fn advance_periodic_anchor_if_armed(
    last_periodic_spawn_unix: &mut Option<i64>,
    now_unix: i64,
    arm_periodic: bool,
) {
    if arm_periodic {
        *last_periodic_spawn_unix = Some(now_unix);
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
    skilllite_path: &Path,
) -> bool {
    let now = now_unix();
    let anchor = {
        let mut guard = match last_periodic_spawn_unix.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        prepare_periodic_anchor(&mut guard, now)
    };

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

    // Match agent-rpc `growth_due`: advance when the periodic arm contributed, even if
    // the empty-proposal preflight below skips the actual spawn.
    {
        let mut guard = match last_periodic_spawn_unix.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        advance_periodic_anchor_if_armed(&mut guard, now, a9.arm_periodic);
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
    fn prepare_seeds_none_anchor_once() {
        let mut last = None;
        let t0 = 1_000_000i64;
        assert_eq!(prepare_periodic_anchor(&mut last, t0), Some(t0));
        assert_eq!(last, Some(t0));
        // Subsequent prepare with later now must keep the seeded anchor.
        assert_eq!(prepare_periodic_anchor(&mut last, t0 + 30), Some(t0));
        assert_eq!(last, Some(t0));
    }

    #[test]
    fn advance_only_when_periodic_arm_fires() {
        let mut last = Some(1_000_000i64);
        advance_periodic_anchor_if_armed(&mut last, 1_000_070, false);
        assert_eq!(last, Some(1_000_000));
        advance_periodic_anchor_if_armed(&mut last, 1_000_070, true);
        assert_eq!(last, Some(1_000_070));
    }

    #[test]
    fn first_tick_then_interval_matches_growth_due_semantics() {
        let mut last = None;
        let t0 = 1_000_000i64;
        let interval = 60i64;

        let anchor0 = prepare_periodic_anchor(&mut last, t0).expect("seeded");
        assert_eq!(anchor0, t0);
        // inspect: elapsed 0 → arm_periodic false
        let elapsed0 = t0.saturating_sub(anchor0);
        assert!(elapsed0 < interval);

        let t1 = t0 + 30;
        let anchor1 = prepare_periodic_anchor(&mut last, t1).expect("kept");
        assert_eq!(anchor1, t0);
        assert!(t1.saturating_sub(anchor1) < interval);

        let t2 = t0 + 70;
        let anchor2 = prepare_periodic_anchor(&mut last, t2).expect("kept");
        assert!(t2.saturating_sub(anchor2) >= interval);
        // Due tick with arm_periodic → advance
        advance_periodic_anchor_if_armed(&mut last, t2, true);
        assert_eq!(last, Some(t2));

        // Next heartbeats before another full interval must not be periodic-due.
        let t3 = t2 + 30;
        let anchor3 = prepare_periodic_anchor(&mut last, t3).expect("kept");
        assert!(t3.saturating_sub(anchor3) < interval);
    }
}
