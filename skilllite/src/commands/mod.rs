//! Phase 3 CLI commands — skill management, IDE integration, environment management.
//!
//! These modules implement pure management commands migrated from Python CLI.
//! They depend ONLY on the skill/env layer (Layer 1-2), NOT on the agent layer (Layer 3).
//!
//! Layer separation:
//!   commands/ → skill/, env/     ✅ (management layer)
//!   commands/ → agent/           ❌ (forbidden — use agent/rpc.rs instead)
//!
//! Phase 3.5c additions:
//!   init      — project initialization (binary check + .skills/ + deps + audit)
//!   quickstart — zero-config LLM setup + chat launch
//!
//! Core execution (refactored from main.rs):
//!   execute  — run_skill, exec_script, bash_command, validate_skill, show_skill_info
//!   scan     — scan_skill and script analysis
//!   security — security_scan_script, dependency_audit_skill

pub mod execute;
pub mod scan;
pub mod security;

pub mod skill;
pub mod ide;
pub mod env;
pub mod reindex;
pub mod init;
#[cfg(feature = "agent")]
pub mod planning_rules_gen;
#[cfg(feature = "agent")]
pub mod quickstart;
