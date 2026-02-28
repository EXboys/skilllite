//! Canonical Sandbox Runtime Policy - Single Source of Truth
//!
//! This module defines the **runtime** security policy for sandbox isolation.
//! It applies to ALL sandbox implementations (macOS, Linux). Platform-specific
//! code translates these policies into native formats:
//!
//! - macOS: Seatbelt profile (generate_sandbox_profile_with_proxy)
//! - Linux: bwrap/firejail arguments
//!
//! Distinct from the parent `security` module's **static code scanning**:
//! - `security::scanner` = pre-execution static analysis of script source
//! - `security::policy` = runtime isolation rules (paths, processes, network)
//!
//! Policy categories:
//! - Mandatory deny paths (file writes)
//! - Move protection paths (block mv/rename)
//! - Sensitive file read paths
//! - Process execution denylist
//! - Network policy

// ============================================================================
// Mandatory Deny Paths (File Writes - ALWAYS blocked)
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
// Move Protection Paths (block mv/rename to prevent bypass)
// ============================================================================

/// Paths protected from move/rename operations.
/// Prevents bypass: mv ~/.ssh /tmp/ssh then access /tmp/ssh
pub fn get_move_protection_paths() -> Vec<String> {
    vec![
        "~/.ssh".to_string(),
        "~/.aws".to_string(),
        "~/.gnupg".to_string(),
        "~/.kube".to_string(),
        "~/.docker".to_string(),
        "~/.git/hooks".to_string(),
        "~/.bashrc".to_string(),
        "~/.zshrc".to_string(),
        "~/.profile".to_string(),
        "**/.git/hooks".to_string(),
        "**/.env".to_string(),
    ]
}

// ============================================================================
// Sensitive File Read Paths (block reads)
// ============================================================================

/// Unix-style home directory regex for path matching
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Linux variant used when target_os = "linux"
pub enum HomePathStyle {
    /// macOS: /Users/username/...
    MacOS,
    /// Linux: /home/username/... or /root/...
    Linux,
}

/// Sensitive paths that should never be read (system-wide)
pub fn get_sensitive_read_system_paths(platform: HomePathStyle) -> Vec<&'static str> {
    let mut paths = vec!["/etc", "/private/etc"];
    if matches!(platform, HomePathStyle::Linux) {
        paths = vec!["/etc"];
    }
    paths
}

/// Home-relative paths that should never be read
/// Platform translates ~/.ssh -> /Users/xxx/.ssh or /home/xxx/.ssh
pub fn get_sensitive_read_home_relative_paths() -> Vec<&'static str> {
    vec![
        ".ssh",
        ".aws",
        ".gnupg",
        ".kube",
        ".docker",
        ".config",
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".bash_history",
        ".zsh_history",
    ]
}

/// Project-level regex patterns to block when NOT in relaxed mode
/// Returns full regex pattern (e.g. for Seatbelt: (deny file-read* (regex #"...")))
pub fn get_sensitive_read_project_regex_patterns(relaxed: bool) -> Vec<&'static str> {
    if relaxed {
        return vec![];
    }
    vec![
        r"/\.git/",       // any path containing .git/
        r"/\.env$",       // path ending with .env
        r"/\.env\.[^/]+$", // path ending with .env.xxx
    ]
}

// ============================================================================
// Process Execution Policy
// ============================================================================
//
// Strategy: WHITELIST (as of hardening step 2)
//
// macOS Seatbelt profile now uses a whitelist approach:
//   (allow process-exec (literal "/resolved/interpreter"))
//   (deny process-exec)
//
// The resolved interpreter path comes from RuntimePaths at execution time.
// The legacy denylist constants below are retained for Linux (firejail) and
// as documentation of known-dangerous binaries.

/// Commands always blocked from execution (used by Linux firejail/bwrap)
pub const PROCESS_DENYLIST_ALWAYS: &[&str] = &[
    "/bin/bash",
    "/bin/zsh",
    "/bin/sh",
    "/usr/bin/env",
    "/usr/bin/curl",
    "/usr/bin/wget",
    "/usr/bin/ssh",
    "/usr/bin/scp",
    "/bin/rm",
    "/bin/chmod",
];

/// Commands blocked in strict mode only (allowed when relaxed/L2)
pub const PROCESS_DENYLIST_STRICT_ONLY: &[&str] = &["/usr/bin/git"];

/// macOS-specific: osascript (AppleScript execution)
pub const PROCESS_DENYLIST_MACOS: &[&str] = &["/usr/bin/osascript"];

/// Get full process denylist for a platform (used by Linux sandbox implementations)
pub fn get_process_exec_denylist(relaxed: bool, platform: HomePathStyle) -> Vec<&'static str> {
    let mut list: Vec<&'static str> = PROCESS_DENYLIST_ALWAYS.iter().copied().collect();
    if !relaxed {
        list.extend(PROCESS_DENYLIST_STRICT_ONLY);
    }
    if matches!(platform, HomePathStyle::MacOS) {
        list.extend(PROCESS_DENYLIST_MACOS);
    }
    list
}

// ============================================================================
// IPC / Kernel Operation Policy (macOS Seatbelt)
// ============================================================================
//
// High-risk Mach/IOKit operations that are always denied in the sandbox.
// These are never needed by skill scripts and can be exploited for sandbox escape.

/// Mach IPC operations that are always blocked.
/// - mach-register: prevents registering rogue Mach services (IPC injection)
/// - mach-priv-task-port: prevents debugging/injecting other processes
pub const MACH_DENY_ALWAYS: &[&str] = &["mach-register", "mach-priv-task-port"];

/// IOKit operations that are always blocked.
/// - iokit-open: prevents direct kernel driver access
pub const IOKIT_DENY_ALWAYS: &[&str] = &["iokit-open"];

// ============================================================================
// Network Policy
// ============================================================================

/// Resolved network policy from skill metadata
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedNetworkPolicy {
    /// Network disabled - block all
    BlockAll,
    /// Wildcard "*" in outbound - allow all without proxy
    AllowAll,
    /// Use proxy to filter by outbound domains
    ProxyFiltered { domains: Vec<String> },
}

/// Extract domain part from an outbound rule, stripping optional `:port` suffix.
/// e.g. "*:80" → "*", "*.github.com:443" → "*.github.com", "example.com" → "example.com"
fn strip_port_suffix(rule: &str) -> &str {
    if let Some(colon_pos) = rule.rfind(':') {
        let after_colon = &rule[colon_pos + 1..];
        if !after_colon.is_empty() && after_colon.chars().all(|c| c.is_ascii_digit()) {
            return &rule[..colon_pos];
        }
    }
    rule
}

/// Resolve network policy from metadata
pub fn resolve_network_policy(network_enabled: bool, outbound: &[String]) -> ResolvedNetworkPolicy {
    if !network_enabled {
        return ResolvedNetworkPolicy::BlockAll;
    }
    // Only exact "*" (without port) bypasses proxy entirely (AllowAll).
    // "*:80" / "*:443" still go through the proxy — the domain part "*" is
    // handled by ProxyConfig::domain_matches which allows all domains, but
    // traffic remains observable and controllable through the proxy layer.
    let has_wildcard = outbound.iter().any(|d| d.trim() == "*");
    if has_wildcard {
        return ResolvedNetworkPolicy::AllowAll;
    }
    if outbound.is_empty() {
        return ResolvedNetworkPolicy::BlockAll;
    }
    // Extract domain parts (strip :port if present) for proxy filtering.
    // e.g. "*:80" → "*", "*.github.com:443" → "*.github.com"
    ResolvedNetworkPolicy::ProxyFiltered {
        domains: outbound.iter().map(|s| strip_port_suffix(s.trim()).to_string()).collect(),
    }
}

/// Whether to use network proxy (when policy is ProxyFiltered)
pub fn should_use_proxy(policy: &ResolvedNetworkPolicy) -> bool {
    matches!(policy, ResolvedNetworkPolicy::ProxyFiltered { .. })
}

/// Whether network is allowed without proxy (AllowAll)
pub fn is_allow_all_network(policy: &ResolvedNetworkPolicy) -> bool {
    matches!(policy, ResolvedNetworkPolicy::AllowAll)
}

/// Whether all network is blocked
pub fn is_network_blocked(policy: &ResolvedNetworkPolicy) -> bool {
    matches!(policy, ResolvedNetworkPolicy::BlockAll)
}

// ============================================================================
// Relaxed Mode (L2)
// ============================================================================

/// Check if relaxed mode is enabled (SKILLLITE_SANDBOX_LEVEL=2 or legacy SKILLBOX_SANDBOX_LEVEL=2)
pub fn is_relaxed_mode() -> bool {
    crate::common::env_compat("SKILLLITE_SANDBOX_LEVEL", "SKILLBOX_SANDBOX_LEVEL")
        .map(|s| s.trim() == "2")
        .unwrap_or(false)
}

/// Check if Playwright is explicitly allowed (SKILLLITE_ALLOW_PLAYWRIGHT=1 or legacy SKILLBOX_ALLOW_PLAYWRIGHT=1)
pub fn is_playwright_allowed() -> bool {
    crate::common::env_compat("SKILLLITE_ALLOW_PLAYWRIGHT", "SKILLBOX_ALLOW_PLAYWRIGHT")
        .map(|s| s.trim() == "1" || s.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Whether to allow Playwright (relaxed mode OR explicit flag)
pub fn should_allow_playwright() -> bool {
    is_relaxed_mode() || is_playwright_allowed()
}

// ============================================================================
// Mandatory Deny Rule Structure (for platform translators)
// ============================================================================

/// Represents a mandatory deny rule for file writes
#[derive(Debug, Clone)]
pub struct MandatoryDenyRule {
    /// The pattern to match (file path or directory)
    pub pattern: String,
    /// Whether this is a directory pattern
    pub is_directory: bool,
}

/// Get all mandatory deny rules (used by seatbelt + firejail translators)
pub fn get_mandatory_deny_rules() -> Vec<MandatoryDenyRule> {
    let mut rules = Vec::new();

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

    for pattern in MANDATORY_DENY_DIRECTORIES {
        rules.push(MandatoryDenyRule {
            pattern: pattern.to_string(),
            is_directory: true,
        });
    }

    rules
}
