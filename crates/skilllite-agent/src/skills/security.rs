//! Security scanning and lock file management for skills.

use anyhow::Result;
use std::path::Path;

use skilllite_sandbox::security::scanner::ScriptScanner;
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

    if let Ok(content) = std::fs::read(&entry_path) {
        hasher.update(&content);
    }
    // Also include SKILL.md content
    if let Ok(skill_md) = std::fs::read(skill_dir.join("SKILL.md")) {
        hasher.update(&skill_md);
    }

    hex::encode(hasher.finalize())[..16].to_string()
}

/// Run security scan on a skill's entry point and SKILL.md.
/// Returns formatted report string if any issues found, or None if scan is clean.
pub(super) fn run_security_scan(skill_dir: &Path, metadata: &SkillMetadata) -> Option<String> {
    let mut report_parts = Vec::new();

    // 1. Scan SKILL.md for supply chain / agent-driven social engineering patterns
    let skill_md_path = skill_dir.join("SKILL.md");
    if skill_md_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&skill_md_path) {
            let alerts = skilllite_core::skill::skill_md_security::scan_skill_md_suspicious_patterns(&content);
            if !alerts.is_empty() {
                report_parts.push("SKILL.md security alerts (supply chain / agent-driven social engineering):".to_string());
                for a in &alerts {
                    report_parts.push(format!("  [{}] {}: {}", a.severity.to_uppercase(), a.pattern, a.message));
                }
                report_parts.push(String::new());
            }
        }
    }

    // 2. Scan entry point script
    let entry_path = if !metadata.entry_point.is_empty() {
        skill_dir.join(&metadata.entry_point)
    } else {
        let defaults = ["scripts/main.py", "main.py"];
        match defaults.iter().map(|d| skill_dir.join(d)).find(|p| p.exists()) {
            Some(p) => p,
            None => {
                return if report_parts.is_empty() {
                    None
                } else {
                    Some(report_parts.join("\n"))
                };
            }
        }
    };

    if entry_path.exists() {
        let scanner = ScriptScanner::new();
        match scanner.scan_file(&entry_path) {
            Ok(result) => {
                if !result.is_safe {
                    report_parts.push(skilllite_sandbox::security::scanner::format_scan_result_compact(&result));
                }
            }
            Err(e) => {
                tracing::warn!("Security scan failed for {}: {}", entry_path.display(), e);
                report_parts.push(format!("Script security scan failed: {}. Manual review required.", e));
            }
        }
    }

    if report_parts.is_empty() {
        None
    } else {
        Some(report_parts.join("\n"))
    }
}

// ─── Phase 2.5: .skilllite.lock dependency resolution ───────────────────────
// Kept for future init_deps integration; metadata uses its own read_lock_file_packages.

#[allow(dead_code)]
/// Lock file structure for cached dependency resolution.
#[derive(Debug, serde::Deserialize)]
pub struct LockFile {
    pub compatibility_hash: String,
    pub language: String,
    pub resolved_packages: Vec<String>,
    pub resolved_at: String,
    pub resolver: String,
}

#[allow(dead_code)]
/// Read and validate a `.skilllite.lock` file for a skill.
/// Returns the resolved packages if the lock is fresh, None if stale or missing.
pub fn read_lock_file(skill_dir: &Path, compatibility: Option<&str>) -> Option<Vec<String>> {
    let lock_path = skill_dir.join(".skilllite.lock");
    if !lock_path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&lock_path).ok()?;
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
        tracing::debug!(
            "Lock file stale for {}: hash mismatch",
            skill_dir.display()
        );
        return None;
    }

    Some(lock.resolved_packages)
}

#[allow(dead_code)]
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
    std::fs::write(&lock_path, serde_json::to_string_pretty(&lock)? + "\n")?;

    Ok(())
}
