//! Cheap preflight before snapshots and learner LLM calls.
//!
//! When A9 fires on **periodic** cadence with **no** decision backlog, running the full
//! evolution pipeline still pays for snapshots and learner setup. This module skips that
//! work when there is **no** weighted/unprocessed signal and no skill-tree / external work.
//!
//! Trade-off: when skipped, **prompt rule retirement** (which can run without new decisions)
//! is also skipped for that tick. Disable via `SKILLLITE_EVO_SHALLOW_PREFLIGHT=0`.

use std::path::Path;

use rusqlite::Connection;

use skilllite_core::config::env_keys::evolution as evo_keys;

use crate::external_learner;
use crate::feedback;
use crate::growth_schedule::GrowthScheduleConfig;
use crate::scope::{EvolutionProposal, EvolutionScope};
use crate::Result;

fn env_shallow_enabled() -> bool {
    match std::env::var(evo_keys::SKILLLITE_EVO_SHALLOW_PREFLIGHT)
        .ok()
        .as_deref()
        .map(str::trim)
    {
        None | Some("") | Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on") => {
            true
        }
        Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("off") => false,
        Some(_) => true,
    }
}

/// When `Some(reason)`, caller should log and return early **without** creating a txn snapshot.
pub fn shallow_skip_evolution_run(
    conn: &Connection,
    skills_root: Option<&Path>,
    scope: &EvolutionScope,
    proposal: &EvolutionProposal,
) -> Result<Option<&'static str>> {
    if !env_shallow_enabled() {
        return Ok(None);
    }
    if proposal.dedupe_key.starts_with("user_capability:") {
        return Ok(None);
    }
    if external_learner::should_run_external_learning(conn) {
        return Ok(None);
    }
    // Skill retire / refine / generation consult DB + disk under `.skills`.
    if scope.skills && skills_root.is_some_and(|p| p.is_dir()) {
        return Ok(None);
    }

    let cfg = GrowthScheduleConfig::from_env();
    let weighted =
        crate::growth_schedule::weighted_unprocessed_signal_sum(conn, cfg.signal_window)?;
    let raw = feedback::count_unprocessed_decisions(conn)?;
    if weighted > 0 || raw > 0 {
        return Ok(None);
    }

    Ok(Some(
        "ShallowSkip: no unprocessed decisions and zero weighted signals; skipped snapshot/learners (see SKILLLITE_EVO_SHALLOW_PREFLIGHT)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SkillAction;
    use crate::scope::ProposalSource;
    use crate::ProposalRiskLevel;
    use rusqlite::Connection;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        feedback::ensure_evolution_tables(&conn).unwrap();
        conn
    }

    fn sample_proposal() -> EvolutionProposal {
        EvolutionProposal {
            proposal_id: "p1".into(),
            source: ProposalSource::Passive,
            scope: EvolutionScope {
                prompts: true,
                memory: true,
                skills: false,
                skill_action: SkillAction::None,
                decision_ids: vec![],
            },
            risk_level: ProposalRiskLevel::Low,
            expected_gain: 0.5,
            effort: 1.0,
            roi_score: 0.5,
            dedupe_key: "passive:default".into(),
            acceptance_criteria: vec![],
        }
    }

    #[test]
    fn skips_when_idle_and_prompts_only_no_skills_root() {
        let _g = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("SKILLLITE_EXTERNAL_LEARNING");
        std::env::set_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT", "1");
        let conn = open_mem();
        let mut p = sample_proposal();
        p.scope.skills = false;
        let r = shallow_skip_evolution_run(&conn, None, &p.scope, &p).unwrap();
        assert!(r.is_some());
        std::env::remove_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT");
    }

    #[test]
    fn does_not_skip_when_unprocessed_exists() {
        let _g = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("SKILLLITE_EXTERNAL_LEARNING");
        std::env::set_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT", "1");
        let conn = open_mem();
        conn.execute(
            "INSERT INTO decisions (evolved, total_tools, failed_tools, feedback)
             VALUES (0, 1, 0, 'neutral')",
            [],
        )
        .unwrap();
        let p = sample_proposal();
        let r = shallow_skip_evolution_run(&conn, None, &p.scope, &p).unwrap();
        assert!(r.is_none());
        std::env::remove_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT");
    }

    #[test]
    fn disabled_by_env() {
        let _g = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("SKILLLITE_EXTERNAL_LEARNING");
        std::env::set_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT", "0");
        let conn = open_mem();
        let p = sample_proposal();
        let r = shallow_skip_evolution_run(&conn, None, &p.scope, &p).unwrap();
        assert!(r.is_none());
        std::env::remove_var("SKILLLITE_EVO_SHALLOW_PREFLIGHT");
    }
}
