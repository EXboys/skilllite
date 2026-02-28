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

// ─── Cross-crate conversions ────────────────────────────────────────────────
// skilllite-core::SkillMetadata → skilllite-sandbox::MetadataHint
// Placed here because the binary crate depends on both, while the library
// crates are independent of each other.
// (Cannot use `impl From` due to Rust orphan rules.)

pub fn metadata_into_hint(
    m: skilllite_core::skill::metadata::SkillMetadata,
) -> skilllite_sandbox::security::dependency_audit::MetadataHint {
    skilllite_sandbox::security::dependency_audit::MetadataHint {
        compatibility: m.compatibility,
        resolved_packages: m.resolved_packages,
        description: m.description,
        language: m.language,
        entry_point: m.entry_point,
    }
}

pub mod skill;
pub mod ide;
pub mod env;
pub mod reindex;
pub mod init;
#[cfg(feature = "agent")]
pub mod evolution;
#[cfg(feature = "agent")]
pub mod planning_rules_gen;
#[cfg(feature = "agent")]
pub mod quickstart;
