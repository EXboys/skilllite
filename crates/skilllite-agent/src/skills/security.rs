//! Security scanning and lock file management for skills.

use crate::Result;
use std::path::Path;

use skilllite_core::skill::metadata::SkillMetadata;

/// Compute a hash of a skill's code for cache invalidation.
pub(super) fn compute_skill_hash(skill_dir: &Path, metadata: &SkillMetadata) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();

    // Hash the entry point script content
    let entry_path = if !metadata.entry_point.is_empty() {
        skill_dir.join(&metadata.entry_point)
    } else {
        // Try common defaults
        let defaults = ["scripts/main.py", "main.py"];
        defaults
            .iter()
            .map(|d| skill_dir.join(d))
            .find(|p| p.exists())
            .unwrap_or_else(|| skill_dir.join("SKILL.md"))
    };

    if let Ok(content) = skilllite_fs::read_bytes(&entry_path) {
        hasher.update(&content);
    }
    // Also include SKILL.md content
    if let Ok(skill_md) = skilllite_fs::read_bytes(&skill_dir.join("SKILL.md")) {
        hasher.update(&skill_md);
    }

    hex::encode(hasher.finalize())[..16].to_string()
}

/// Unified pre-gate scan: SKILL.md supply-chain patterns + entry script scan using the **same**
/// [`ScriptScanner`] policy as [`skilllite_sandbox::runner`] (network flag from skill metadata;
/// file/process exec disallowed for rule purposes).
///
/// Call this once before `run_in_sandbox_with_limits_and_level_opt` with `skip_skill_precheck: true`.
pub(super) fn run_security_scan(
    skill_dir: &Path,
    metadata: &SkillMetadata,
    network_enabled: bool,
) -> skilllite_sandbox::security::SkillPrecheckSummary {
    skilllite_sandbox::security::run_skill_precheck_for_metadata(
        skill_dir,
        metadata,
        network_enabled,
    )
}

// ─── Phase 2.5: .skilllite.lock dependency resolution ───────────────────────
// Kept for future init_deps integration; metadata uses its own read_lock_file_packages.

/// Lock file structure for cached dependency resolution.
#[derive(Debug, serde::Deserialize)]
pub struct LockFile {
    pub compatibility_hash: String,
    pub language: String,
    pub resolved_packages: Vec<String>,
    pub resolved_at: String,
    pub resolver: String,
}

/// Read and validate a `.skilllite.lock` file for a skill.
/// Returns the resolved packages if the lock is fresh, None if stale or missing.
pub fn read_lock_file(skill_dir: &Path, compatibility: Option<&str>) -> Option<Vec<String>> {
    let lock_path = skill_dir.join(".skilllite.lock");
    if !lock_path.exists() {
        return None;
    }

    let content = skilllite_fs::read_file(&lock_path).ok()?;
    let lock: LockFile = serde_json::from_str(&content).ok()?;

    // Check staleness via compatibility hash
    let compat_str = compatibility.unwrap_or("");
    let current_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(compat_str.as_bytes());
        hex::encode(hasher.finalize())
    };

    if lock.compatibility_hash != current_hash {
        tracing::debug!("Lock file stale for {}: hash mismatch", skill_dir.display());
        return None;
    }

    Some(lock.resolved_packages)
}

/// Write a `.skilllite.lock` file for a skill.
pub fn write_lock_file(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    packages: &[String],
    resolver: &str,
) -> Result<()> {
    let compat_str = compatibility.unwrap_or("");
    let compat_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(compat_str.as_bytes());
        hex::encode(hasher.finalize())
    };

    let mut sorted_packages = packages.to_vec();
    sorted_packages.sort();

    let lock = serde_json::json!({
        "compatibility_hash": compat_hash,
        "language": language,
        "resolved_packages": sorted_packages,
        "resolved_at": chrono::Utc::now().to_rfc3339(),
        "resolver": resolver,
    });

    let lock_path = skill_dir.join(".skilllite.lock");
    skilllite_fs::write_file(&lock_path, &(serde_json::to_string_pretty(&lock)? + "\n"))?;

    Ok(())
}
