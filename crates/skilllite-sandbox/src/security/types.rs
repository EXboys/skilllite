//! Security issue types and severity definitions
//!
//! This module contains the core type definitions for security scanning.

use serde::{Deserialize, Serialize};

/// Security issue found in script
#[derive(Debug, Clone)]
pub struct SecurityIssue {
    /// Rule ID that triggered this issue
    pub rule_id: String,
    /// Issue severity
    pub severity: SecuritySeverity,
    /// Issue type/category
    pub issue_type: SecurityIssueType,
    /// Line number where issue was found
    pub line_number: usize,
    /// Description of the issue
    pub description: String,
    /// The code snippet that triggered the issue
    pub code_snippet: String,
}

/// Severity levels for security issues
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecuritySeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Types of security issues
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityIssueType {
    FileOperation,
    NetworkRequest,
    CodeInjection,
    MemoryBomb,
    ProcessExecution,
    SystemAccess,
    DangerousModule,
    /// High-entropy line detected — likely obfuscated/encoded payload
    ObfuscatedCode,
    /// Scan process failed (timeout, IO error, etc.) — fail-secure
    ScanError,
}

impl std::fmt::Display for SecurityIssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityIssueType::FileOperation => write!(f, "File Operation"),
            SecurityIssueType::NetworkRequest => write!(f, "Network Request"),
            SecurityIssueType::CodeInjection => write!(f, "Code Injection"),
            SecurityIssueType::MemoryBomb => write!(f, "Memory Bomb"),
            SecurityIssueType::ProcessExecution => write!(f, "Process Execution"),
            SecurityIssueType::SystemAccess => write!(f, "System Access"),
            SecurityIssueType::DangerousModule => write!(f, "Dangerous Module"),
            SecurityIssueType::ObfuscatedCode => write!(f, "Obfuscated Code"),
            SecurityIssueType::ScanError => write!(f, "Scan Error"),
        }
    }
}

/// Result of scanning a script
#[derive(Debug, Clone)]
pub struct ScanResult {
    /// Whether the script is safe to execute
    pub is_safe: bool,
    /// List of security issues found
    pub issues: Vec<SecurityIssue>,
}
