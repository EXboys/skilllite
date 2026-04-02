//! Path, template, size, and content gatekeepers for evolution writes.

use std::path::Path;

use crate::error::bail;
use crate::seed;
use crate::Result;

// ─── Gatekeeper (L1-L3) ───────────────────────────────────────────────────────

const ALLOWED_EVOLUTION_PATHS: &[&str] = &["prompts", "memory", "skills/_evolved"];

/// L1 path gatekeeper. When skills_root is Some, also allows target under skills_root/_evolved
/// (project-level skill evolution).
pub fn gatekeeper_l1_path(chat_root: &Path, target: &Path, skills_root: Option<&Path>) -> bool {
    for allowed in ALLOWED_EVOLUTION_PATHS {
        let allowed_dir = chat_root.join(allowed);
        if target.starts_with(&allowed_dir) {
            return true;
        }
    }
    if let Some(sr) = skills_root {
        let evolved = sr.join("_evolved");
        if target.starts_with(&evolved) {
            return true;
        }
    }
    false
}

pub fn gatekeeper_l1_template_integrity(filename: &str, new_content: &str) -> Result<()> {
    let missing = seed::validate_template(filename, new_content);
    if !missing.is_empty() {
        bail!(
            "Gatekeeper L1b: evolved template '{}' is missing required placeholders {:?}",
            filename,
            missing
        );
    }
    Ok(())
}

pub fn gatekeeper_l2_size(new_rules: usize, new_examples: usize, new_skills: usize) -> bool {
    new_rules <= 5 && new_examples <= 3 && new_skills <= 1
}

const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key",
    "api-key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "token",
    "bearer",
    "private_key",
    "private-key",
    "-----BEGIN",
    "-----END",
    "skip scan",
    "bypass",
    "disable security",
    "eval(",
    "exec(",
    "__import__",
];

pub fn gatekeeper_l3_content(content: &str) -> Result<()> {
    let lower = content.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            bail!(
                "Gatekeeper L3: evolution product contains sensitive pattern: '{}'",
                pattern
            );
        }
    }
    Ok(())
}
