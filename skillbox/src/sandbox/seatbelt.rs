//! macOS Seatbelt Profile Generation
//!
//! Translates canonical security policy (from security_policy) into macOS sandbox-exec
//! Seatbelt profile format.

use crate::sandbox::security::policy as security_policy;

// Re-export for Linux (firejail/bwrap)
#[cfg(target_os = "linux")]
pub use security_policy::{get_mandatory_deny_rules, MandatoryDenyRule, MANDATORY_DENY_DIRECTORIES};

use security_policy::HomePathStyle;

// ============================================================================
// Seatbelt-Specific Formatting
// ============================================================================

/// Escape special regex characters for Seatbelt profile
fn seatbelt_regex_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result
}

/// Generate Seatbelt deny patterns for macOS sandbox-exec
/// Uses canonical policy from security_policy module
pub fn generate_seatbelt_mandatory_deny_patterns() -> Vec<String> {
    let mut patterns = Vec::new();

    for rule in security_policy::get_mandatory_deny_rules() {
        let escaped = seatbelt_regex_escape(&rule.pattern);

        if rule.is_directory {
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}\"))",
                escaped
            ));
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}/.+\"))",
                escaped
            ));
        } else if rule.pattern.contains('/') {
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}\"))",
                escaped
            ));
        } else {
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}$\"))",
                escaped
            ));
        }
    }

    patterns
}

/// Generate Seatbelt file-read deny rules for sensitive paths (macOS only)
pub fn generate_seatbelt_sensitive_read_deny_rules(relaxed: bool) -> Vec<String> {
    let mut rules = Vec::new();

    for path in security_policy::get_sensitive_read_system_paths(HomePathStyle::MacOS) {
        rules.push(format!("(deny file-read* (subpath \"{}\"))", path));
    }

    for path in security_policy::get_sensitive_read_home_relative_paths() {
        let escaped = path.replace('.', "\\.");
        rules.push(format!(
            "(deny file-read* (regex #\"^/Users/[^/]+/{}\"))",
            escaped
        ));
    }

    rules.push("(deny file-read* (regex #\"^/Users/[^/]+/Library/Keychains\"))".to_string());

    for pattern in security_policy::get_sensitive_read_project_regex_patterns(relaxed) {
        rules.push(format!("(deny file-read* (regex #\"{}\"))", pattern));
    }

    rules
}

/// Generate blacklist arguments for firejail (Linux)
#[cfg(target_os = "linux")]
pub fn generate_firejail_blacklist_args() -> Vec<String> {
    let mut args = Vec::new();

    for rule in security_policy::get_mandatory_deny_rules() {
        let path = if rule.pattern.starts_with('/') {
            rule.pattern.clone()
        } else {
            format!("~/{}", rule.pattern)
        };

        args.push(format!("--blacklist={}", path));
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandatory_deny_rules() {
        let rules = security_policy::get_mandatory_deny_rules();
        assert!(!rules.is_empty());

        let has_file_rules = rules.iter().any(|r| !r.is_directory);
        let has_dir_rules = rules.iter().any(|r| r.is_directory);
        assert!(has_file_rules);
        assert!(has_dir_rules);
    }

    #[test]
    fn test_seatbelt_patterns() {
        let patterns = generate_seatbelt_mandatory_deny_patterns();
        assert!(!patterns.is_empty());

        for pattern in &patterns {
            assert!(pattern.starts_with("(deny file-write*"));
            assert!(pattern.ends_with(")"));
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_firejail_blacklist_args() {
        let args = generate_firejail_blacklist_args();
        assert!(!args.is_empty());

        for arg in &args {
            assert!(arg.starts_with("--blacklist="));
        }
    }
}
