//! Security policies and mandatory deny paths for sandbox isolation
//! 
//! This module provides centralized security configuration that applies to all
//! sandbox implementations (macOS, Linux). It defines:
//! - Mandatory deny paths (files that are ALWAYS blocked from writes)
//! - Network filtering policies
//! - Unix socket restrictions
//! - Violation monitoring utilities


// ============================================================================
// Mandatory Deny Paths (Auto-Protected Files)
// These files are ALWAYS blocked from writes, even within allowed paths.
// This provides defense-in-depth against sandbox escapes and config tampering.
// ============================================================================

/// Shell configuration files - blocked to prevent shell injection attacks
pub const MANDATORY_DENY_SHELL_CONFIGS: &[&str] = &[
    ".bashrc",
    ".bash_profile",
    ".bash_login",
    ".bash_logout",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".zlogin",
    ".zlogout",
    ".profile",
    ".login",
    ".cshrc",
    ".tcshrc",
    ".kshrc",
    ".config/fish/config.fish",
];

/// Git configuration files - blocked to prevent git hook injection
pub const MANDATORY_DENY_GIT_CONFIGS: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".git/config",
    ".git/hooks/pre-commit",
    ".git/hooks/post-commit",
    ".git/hooks/pre-push",
    ".git/hooks/post-checkout",
    ".git/hooks/pre-receive",
    ".git/hooks/post-receive",
    ".git/hooks/prepare-commit-msg",
    ".git/hooks/commit-msg",
    ".git/hooks/pre-rebase",
    ".git/hooks/post-rewrite",
    ".git/hooks/post-merge",
];

/// IDE and editor configuration - blocked to prevent malicious workspace settings
pub const MANDATORY_DENY_IDE_CONFIGS: &[&str] = &[
    ".vscode/settings.json",
    ".vscode/tasks.json",
    ".vscode/launch.json",
    ".vscode/extensions.json",
    ".idea/workspace.xml",
    ".idea/tasks.xml",
    ".idea/runConfigurations",
    ".sublime-project",
    ".sublime-workspace",
    ".atom/config.cson",
    ".emacs",
    ".vimrc",
    ".nvimrc",
    ".config/nvim/init.vim",
    ".config/nvim/init.lua",
];

/// Package manager and tool configurations - blocked to prevent supply chain attacks
pub const MANDATORY_DENY_PACKAGE_CONFIGS: &[&str] = &[
    ".npmrc",
    ".yarnrc",
    ".yarnrc.yml",
    ".pnpmrc",
    ".pypirc",
    ".pip/pip.conf",
    ".cargo/config",
    ".cargo/config.toml",
    ".cargo/credentials",
    ".cargo/credentials.toml",
    ".gemrc",
    ".bundle/config",
    ".m2/settings.xml",
    ".gradle/gradle.properties",
    ".nuget/NuGet.Config",
];

/// Security-sensitive files - blocked to prevent credential theft
pub const MANDATORY_DENY_SECURITY_FILES: &[&str] = &[
    ".ssh/authorized_keys",
    ".ssh/known_hosts",
    ".ssh/config",
    ".ssh/id_rsa",
    ".ssh/id_rsa.pub",
    ".ssh/id_ed25519",
    ".ssh/id_ed25519.pub",
    ".gnupg/gpg.conf",
    ".gnupg/pubring.kbx",
    ".gnupg/trustdb.gpg",
    ".aws/credentials",
    ".aws/config",
    ".kube/config",
    ".docker/config.json",
    ".netrc",
    ".ripgreprc",
];

/// AI/Agent configuration files - blocked to prevent agent manipulation
pub const MANDATORY_DENY_AGENT_CONFIGS: &[&str] = &[
    ".mcp.json",
    ".claude/settings.json",
    ".claude/commands",
    ".claude/agents",
    ".cursor/settings.json",
    ".continue/config.json",
    ".aider.conf.yml",
    ".copilot/config.json",
    ".codeium/config.json",
];

/// Directories that should be completely blocked from writes
pub const MANDATORY_DENY_DIRECTORIES: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".kube",
    ".docker",
    ".git/hooks",
    ".vscode",
    ".idea",
    ".claude",
    ".cursor",
];

// ============================================================================
// Security Policy Structures
// ============================================================================

/// Represents a mandatory deny rule
#[derive(Debug, Clone)]
pub struct MandatoryDenyRule {
    /// The pattern to match (file path or directory)
    pub pattern: String,
    /// Whether this is a directory pattern
    pub is_directory: bool,
}

/// Get all mandatory deny rules
pub fn get_mandatory_deny_rules() -> Vec<MandatoryDenyRule> {
    let mut rules = Vec::new();

    // Add file rules
    for pattern in MANDATORY_DENY_SHELL_CONFIGS {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    for pattern in MANDATORY_DENY_GIT_CONFIGS {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    for pattern in MANDATORY_DENY_IDE_CONFIGS {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    for pattern in MANDATORY_DENY_PACKAGE_CONFIGS {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    for pattern in MANDATORY_DENY_SECURITY_FILES {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    for pattern in MANDATORY_DENY_AGENT_CONFIGS {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: false,
        });
    }

    // Add directory rules
    for pattern in MANDATORY_DENY_DIRECTORIES {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: true,
        });
    }

    rules
}

// ============================================================================
// macOS Seatbelt Profile Generation
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
pub fn generate_seatbelt_mandatory_deny_patterns() -> Vec<String> {
    let mut patterns = Vec::new();
    
    for rule in get_mandatory_deny_rules() {
        let escaped = seatbelt_regex_escape(&rule.pattern);
        
        if rule.is_directory {
            // Block the directory itself and all contents
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}\"))",
                escaped
            ));
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}/.+\"))",
                escaped
            ));
        } else if rule.pattern.contains('/') {
            // Path with subdirectory - match exactly
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}\"))",
                escaped
            ));
        } else {
            // Simple filename - match in any directory
            patterns.push(format!(
                "(deny file-write* (regex #\"(^|/){}$\"))",
                escaped
            ));
        }
    }
    
    patterns
}

// ============================================================================
// Tests
// ============================================================================

/// Generate blacklist arguments for firejail (used in tests)
#[cfg(test)]
fn generate_firejail_blacklist_args() -> Vec<String> {
    let mut args = Vec::new();
    
    for rule in get_mandatory_deny_rules() {
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
        let rules = get_mandatory_deny_rules();
        assert!(!rules.is_empty());
        
        // Check that we have both file and directory rules
        let has_file_rules = rules.iter().any(|r| !r.is_directory);
        let has_dir_rules = rules.iter().any(|r| r.is_directory);
        assert!(has_file_rules);
        assert!(has_dir_rules);
    }

    #[test]
    fn test_seatbelt_patterns() {
        let patterns = generate_seatbelt_mandatory_deny_patterns();
        assert!(!patterns.is_empty());
        
        // Check that patterns are properly formatted
        for pattern in &patterns {
            assert!(pattern.starts_with("(deny file-write*"));
            assert!(pattern.ends_with(")"));
        }
    }

    #[test]
    fn test_firejail_blacklist_args() {
        let args = generate_firejail_blacklist_args();
        assert!(!args.is_empty());
        
        // Check that args are properly formatted
        for arg in &args {
            assert!(arg.starts_with("--blacklist="));
        }
    }
}
