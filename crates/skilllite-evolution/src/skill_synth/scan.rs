//! L4 安全扫描、技能使用追踪

use std::path::Path;

use anyhow::Result;

use skilllite_sandbox::security::scanner::ScriptScanner;

use super::SkillMeta;

/// Detect if skill_md_content declares network requirement.
pub(super) fn skill_md_needs_network(skill_md: &str) -> bool {
    let lower = skill_md.to_lowercase();
    lower.contains("network access")
        || lower.contains("network")
        || lower.contains("需网络")
        || lower.contains("需网络权限")
        || lower.contains("internet")
        || lower.contains("api access")
}

pub(super) fn run_l4_scan(
    script_content: &str,
    script_path: &Path,
    allow_network: bool,
) -> Result<bool> {
    let scanner = ScriptScanner::new().allow_network(allow_network);
    let result = scanner.scan_content(script_content, script_path)?;
    if !result.is_safe {
        tracing::warn!(
            "L4 security scan found issues in {}",
            script_path.display()
        );
    }
    Ok(result.is_safe)
}

/// Update .meta.json after a skill execution (called from agent_loop).
pub fn track_skill_usage(evolved_dir: &Path, skill_name: &str, success: bool) {
    let meta_path = evolved_dir.join(skill_name).join(".meta.json");
    if !meta_path.exists() {
        return;
    }
    let mut meta: SkillMeta = match skilllite_fs::read_file(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(m) => m,
        None => return,
    };

    meta.call_count += 1;
    if success {
        meta.success_count += 1;
    } else {
        meta.failure_count += 1;
    }
    meta.last_used = Some(chrono::Utc::now().to_rfc3339());

    let _ = skilllite_fs::write_file(
        &meta_path,
        &serde_json::to_string_pretty(&meta).unwrap_or_default(),
    );
}
