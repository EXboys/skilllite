//! SkillLite Evolution: self-evolving prompts, skills, and memory.
//!
//! EVO-1: Feedback collection + evaluation system + structured memory.
//! EVO-2: Prompt externalization + seed data mechanism.
//! EVO-3: Evolution engine core + evolution prompt design.
//! EVO-5: Polish + transparency (audit, degradation, CLI, time trends).
//!
//! Interacts with the agent through the [`EvolutionLlm`] trait for LLM completion.

pub mod audit;
pub mod changelog;
pub mod config;
pub mod error;
mod evolution_memory_rollup;
pub mod external_learner;
pub mod feedback;
pub mod gatekeeper;
pub mod growth_schedule;
pub mod lifecycle;
pub mod llm;
pub mod memory_learner;
pub mod prompt_learner;
pub mod rollback;
pub mod run;
pub mod run_state;
pub mod scope;
pub mod seed;
pub mod shallow_preflight;
pub mod skill_synth;
pub mod snapshots;

pub use error::{Error, Result};

pub use audit::{decision_ids_to_mark_after_run, log_evolution_event, mark_decisions_evolved};
pub use changelog::append_changelog;
pub use config::{EvolutionMode, EvolutionProfile, EvolutionThresholds, SkillAction};
pub use gatekeeper::{
    gatekeeper_l1_path, gatekeeper_l1_template_integrity, gatekeeper_l2_size, gatekeeper_l3_content,
};
pub use growth_schedule::{
    growth_due, inspect_growth_due, seconds_since_last_evolution_run, signal_burst_due,
    weighted_unprocessed_signal_sum, GrowthDueDiagnostics, GrowthDueOutcome, GrowthScheduleConfig,
};
pub use lifecycle::on_shutdown;
pub use llm::{sanitize_visible_llm_text, strip_think_blocks, EvolutionLlm, EvolutionMessage};
pub use rollback::check_auto_rollback;
pub use run::{format_evolution_changes, query_changes_by_txn, run_evolution};
pub use run_state::{finish_evolution, try_start_evolution, EvolutionRunResult};
pub use scope::{
    describe_empty_evolution_proposals, enqueue_user_capability_evolution,
    passive_schedule_diagnostics, should_evolve, should_evolve_with_mode,
    would_have_evolution_proposals, EvolutionProposal, EvolutionScope, PassiveScheduleDiagnostics,
    ProposalRiskLevel, ProposalSource,
};
pub use snapshots::{create_snapshot, restore_snapshot};

pub use skilllite_fs::atomic_write;

#[cfg(test)]
mod lib_tests {
    use super::*;
    use crate::scope::{
        auto_link_acceptance_status, build_proposal, compute_roi_score,
        coordinate_proposals_with_config, AcceptanceThresholds, CoordinatorDecision,
        EvolutionCoordinatorConfig, EvolutionRiskBudget,
    };
    use crate::snapshots::{create_extended_snapshot, restore_extended_snapshot};
    use rusqlite::Connection;
    use std::path::Path;
    use std::sync::Mutex;

    static EVO_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn strip_think_blocks_after_closing_tag() {
        let s = "<redacted_thinking>\nhidden\n</redacted_thinking>\nvisible reply";
        assert_eq!(strip_think_blocks(s), "visible reply");
    }

    #[test]
    fn strip_think_blocks_plain_text_unchanged() {
        let s = "no think tags here";
        assert_eq!(strip_think_blocks(s), s);
    }

    #[test]
    fn strip_think_blocks_reasoning_tag() {
        let s = "<reasoning>x</reasoning>\nhello";
        assert_eq!(strip_think_blocks(s), "hello");
    }

    #[test]
    fn evolution_message_constructors() {
        let u = EvolutionMessage::user("u");
        assert_eq!(u.role, "user");
        assert_eq!(u.content.as_deref(), Some("u"));
        let sy = EvolutionMessage::system("s");
        assert_eq!(sy.role, "system");
    }

    #[test]
    fn skill_action_maps_to_evolve_skills_generate_flag() {
        assert!(SkillAction::None.should_run_skill_generation_paths());
        assert!(SkillAction::Generate.should_run_skill_generation_paths());
        assert!(!SkillAction::Refine.should_run_skill_generation_paths());
    }

    #[test]
    fn evolution_mode_capability_flags() {
        assert!(EvolutionMode::All.prompts_enabled());
        assert!(EvolutionMode::All.memory_enabled());
        assert!(EvolutionMode::All.skills_enabled());
        assert!(EvolutionMode::PromptsOnly.prompts_enabled());
        assert!(!EvolutionMode::PromptsOnly.memory_enabled());
        assert!(!EvolutionMode::MemoryOnly.prompts_enabled());
        assert!(EvolutionMode::MemoryOnly.memory_enabled());
        assert!(EvolutionMode::Disabled.is_disabled());
    }

    #[test]
    fn evolution_run_result_txn_id() {
        assert_eq!(
            EvolutionRunResult::Completed(Some("t1".into())).txn_id(),
            Some("t1")
        );
        assert_eq!(EvolutionRunResult::SkippedBusy.txn_id(), None);
    }

    #[test]
    fn gatekeeper_l2_size_bounds() {
        assert!(gatekeeper_l2_size(5, 3, 1));
        assert!(!gatekeeper_l2_size(6, 0, 0));
        assert!(!gatekeeper_l2_size(0, 4, 0));
        assert!(!gatekeeper_l2_size(0, 0, 2));
    }

    #[test]
    fn gatekeeper_l3_rejects_secret_pattern() {
        assert!(gatekeeper_l3_content("safe text").is_ok());
        assert!(gatekeeper_l3_content("has api_key in body").is_err());
    }

    #[test]
    fn gatekeeper_l1_path_allows_prompts_under_chat_root() {
        let root = Path::new("/home/u/.skilllite/chat");
        let target = root.join("prompts/rules.json");
        assert!(gatekeeper_l1_path(root, &target, None));
        let bad = Path::new("/etc/passwd");
        assert!(!gatekeeper_l1_path(root, bad, None));
    }

    #[test]
    fn try_start_evolution_is_exclusive() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        finish_evolution();
        assert!(try_start_evolution());
        assert!(!try_start_evolution());
        finish_evolution();
    }

    #[test]
    fn evolution_thresholds_default_nonzero_cooldown() {
        let t = EvolutionThresholds::default();
        assert!(t.cooldown_hours > 0.0);
        assert!(t.recent_days > 0);
    }

    #[test]
    fn roi_score_penalizes_risk() {
        let low = compute_roi_score(1.0, 1.0, ProposalRiskLevel::Low);
        let high = compute_roi_score(1.0, 1.0, ProposalRiskLevel::High);
        assert!(low > high);
    }

    #[test]
    fn coordinator_executes_when_policy_runtime_disabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Medium,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: false,
                auto_execute_low_risk: false,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Execute(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_auto_executes_low_risk_when_enabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Low,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Execute(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_queues_when_low_risk_budget_exhausted() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Low,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );

        conn.execute(
            "INSERT INTO evolution_backlog
             (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, note)
             VALUES (?1, 'active', ?2, '{}', 'low', 0.1, 0.1, 1.0, '[]', 'executed', 'seed')",
            rusqlite::params![
                "seed_proposal",
                format!("seed_{}", uuid::Uuid::new_v4()),
            ],
        )
        .expect("insert seed");

        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 1,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Queued(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_denies_critical_when_policy_runtime_disabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            skills: true,
            skill_action: SkillAction::Generate,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Passive,
            scope,
            ProposalRiskLevel::Critical,
            0.9,
            3.0,
            vec!["no regressions".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: false,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 1,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Denied(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_denies_critical_when_policy_enabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            skills: true,
            skill_action: SkillAction::Generate,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Passive,
            scope,
            ProposalRiskLevel::Critical,
            0.9,
            3.0,
            vec!["no regressions".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 1,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Denied(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn enqueue_user_capability_evolution_inserts_backlog_row() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let proposal_id = enqueue_user_capability_evolution(
            &conn,
            "weather",
            "failure",
            "Tool cannot provide tomorrow forecast",
        )
        .expect("enqueue");
        let (status, risk, dedupe): (String, String, String) = conn
            .query_row(
                "SELECT status, risk_level, dedupe_key FROM evolution_backlog WHERE proposal_id = ?1",
                rusqlite::params![proposal_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read row");
        assert_eq!(status, "queued");
        assert_eq!(risk, "medium");
        assert_eq!(dedupe, "user_capability:weather:failure");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn enqueue_user_capability_evolution_returns_existing_id_when_deduped() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let first_id = enqueue_user_capability_evolution(
            &conn,
            "task_completion",
            "partial_success",
            "first auth",
        )
        .expect("first enqueue");
        let second_id = enqueue_user_capability_evolution(
            &conn,
            "task_completion",
            "partial_success",
            "second auth",
        )
        .expect("second enqueue");

        let backlog_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evolution_backlog WHERE dedupe_key = 'user_capability:task_completion:partial_success' AND status != 'executed'",
                [],
                |row| row.get(0),
            )
            .expect("count backlog rows");
        assert_eq!(backlog_rows, 1);
        assert_eq!(second_id, first_id);

        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evolution_backlog WHERE proposal_id = ?1",
                [second_id],
                |row| row.get(0),
            )
            .expect("verify returned proposal id exists");
        assert_eq!(exists, 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn seed_backlog_row(conn: &Connection, proposal_id: &str, updated_at: &str) {
        conn.execute(
            "INSERT INTO evolution_backlog
             (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, acceptance_status, note, updated_at)
             VALUES (?1, 'active', ?2, '{}', 'low', 0.5, 0.5, 1.0, '[]', 'executed', 'pending_validation', 'seed', ?3)",
            rusqlite::params![proposal_id, format!("dedupe_{}", proposal_id), updated_at],
        )
        .expect("insert backlog row");
    }

    #[test]
    fn auto_link_acceptance_stays_pending_without_full_window() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_pending", "2026-04-01 00:00:00");

        conn.execute(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES ('2026-04-01', 0.90, 0.1, 1.0, 0.05, 0.0)",
            [],
        )
        .expect("insert metric");
        conn.execute(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version)
             VALUES ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1')",
            [],
        )
        .expect("insert run");

        auto_link_acceptance_status(&conn, "p_pending").expect("auto link");
        let (status, note): (String, String) = conn
            .query_row(
                "SELECT acceptance_status, note FROM evolution_backlog WHERE proposal_id = 'p_pending'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read backlog");
        assert_eq!(status, "pending_validation");
        assert!(note.contains("Awaiting acceptance window"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_link_acceptance_marks_met_on_healthy_window() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_met", "2026-04-01 00:00:00");

        conn.execute_batch(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES
             ('2026-04-01', 0.82, 0.1, 1.0, 0.08, 0.0),
             ('2026-04-02', 0.85, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-03', 0.80, 0.1, 1.0, 0.12, 0.0);
             INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES
             ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1'),
             ('2026-04-02T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-2'),
             ('2026-04-03T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-3');",
        )
        .expect("seed metrics and runs");

        auto_link_acceptance_status(&conn, "p_met").expect("auto link");
        let status: String = conn
            .query_row(
                "SELECT acceptance_status FROM evolution_backlog WHERE proposal_id = 'p_met'",
                [],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "met");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_link_acceptance_marks_not_met_when_rollback_rate_high() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_not_met", "2026-04-01 00:00:00");

        conn.execute_batch(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES
             ('2026-04-01', 0.85, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-02', 0.88, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-03', 0.87, 0.1, 1.0, 0.10, 0.0);
             INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES
             ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1'),
             ('2026-04-02T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-2'),
             ('2026-04-03T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-3'),
             ('2026-04-02T09:00:00Z', 'auto_rollback', 'txn-2', 'decline', 'rollback_txn-2');",
        )
        .expect("seed metrics and rollback");

        auto_link_acceptance_status(&conn, "p_not_met").expect("auto link");
        let status: String = conn
            .query_row(
                "SELECT acceptance_status FROM evolution_backlog WHERE proposal_id = 'p_not_met'",
                [],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "not_met");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn acceptance_thresholds_read_from_env_and_clamped() {
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS", "0");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE", "1.2");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE", "-0.1");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE", "0.35");
        let t = AcceptanceThresholds::from_env();
        assert_eq!(t.window_days, 1);
        assert!((t.min_success_rate - 1.0).abs() < 1e-9);
        assert!((t.max_correction_rate - 0.0).abs() < 1e-9);
        assert!((t.max_rollback_rate - 0.35).abs() < 1e-9);
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE");
    }

    #[test]
    fn extended_snapshot_restores_memory_and_skills() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let skills_root = root.join("skills_project");
        let prompts_dir = root.join("prompts");
        let memory_dir = root.join("memory").join("evolution");
        let evolved_dir = skills_root.join("_evolved").join("s1");
        std::fs::create_dir_all(&prompts_dir).expect("prompts");
        std::fs::create_dir_all(&memory_dir).expect("memory");
        let entities_dir = memory_dir.join("entities");
        std::fs::create_dir_all(&entities_dir).expect("entities");
        std::fs::create_dir_all(&evolved_dir).expect("skills");
        std::fs::write(prompts_dir.join("rules.json"), b"before_rules").expect("rules");
        std::fs::write(entities_dir.join("2026-04.md"), b"before_memory").expect("memory shard");
        std::fs::write(evolved_dir.join("SKILL.md"), b"before_skill").expect("skill");

        let snap = create_extended_snapshot(&root, Some(&skills_root), "txn_x", true, true, true)
            .expect("snapshot");
        assert!(snap.iter().any(|f| f == "memory/evolution"));
        assert!(snap.iter().any(|f| f == "skills/_evolved"));

        std::fs::write(prompts_dir.join("rules.json"), b"after_rules").expect("rules mutate");
        std::fs::write(entities_dir.join("2026-04.md"), b"after_memory").expect("memory mutate");
        std::fs::write(evolved_dir.join("SKILL.md"), b"after_skill").expect("skill mutate");

        restore_extended_snapshot(&root, Some(&skills_root), "txn_x").expect("restore");
        let rules = std::fs::read_to_string(prompts_dir.join("rules.json")).expect("rules read");
        let memory = std::fs::read_to_string(entities_dir.join("2026-04.md")).expect("memory read");
        let skill = std::fs::read_to_string(evolved_dir.join("SKILL.md")).expect("skill read");
        assert_eq!(rules, "before_rules");
        assert_eq!(memory, "before_memory");
        assert_eq!(skill, "before_skill");
        let _ = std::fs::remove_dir_all(&root);
    }
}
