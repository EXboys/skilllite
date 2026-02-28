//! Script scanner for detecting security issues
//!
//! This module provides the `ScriptScanner` struct for scanning scripts
//! for potential security issues before execution.

#![allow(dead_code)]

use super::default_rules::get_default_rules;
use super::rules::{RulesConfig, SecurityRule};
use super::types::{ScanResult, SecurityIssue, SecurityIssueType, SecuritySeverity};
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Script scanner for detecting security issues
pub struct ScriptScanner {
    /// Whether to allow network operations
    allow_network: bool,
    /// Whether to allow file operations
    allow_file_ops: bool,
    /// Whether to allow process execution
    allow_process_exec: bool,
    /// Compiled rules for scanning
    rules: Vec<(SecurityRule, Regex)>,
    /// Disabled rule IDs
    disabled_rules: Vec<String>,
}

impl Default for ScriptScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptScanner {
    /// Create a new scanner with default rules
    pub fn new() -> Self {
        let default_rules = get_default_rules();
        let compiled_rules = Self::compile_rules(&default_rules);

        Self {
            allow_network: false,
            allow_file_ops: false,
            allow_process_exec: false,
            rules: compiled_rules,
            disabled_rules: Vec::new(),
        }
    }

    /// Create a scanner with custom rules configuration
    pub fn with_config(config: &RulesConfig) -> Self {
        let mut rules = if config.use_default_rules {
            get_default_rules()
        } else {
            Vec::new()
        };

        // Add custom rules
        rules.extend(config.rules.clone());

        let compiled_rules = Self::compile_rules(&rules);

        Self {
            allow_network: false,
            allow_file_ops: false,
            allow_process_exec: false,
            rules: compiled_rules,
            disabled_rules: config.disabled_rules.clone(),
        }
    }

    /// Compile a list of rules into regex patterns
    fn compile_rules(rules: &[SecurityRule]) -> Vec<(SecurityRule, Regex)> {
        rules
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|rule| match rule.compile() {
                Ok(regex) => Some((rule.clone(), regex)),
                Err(e) => {
                    tracing::warn!("Failed to compile rule '{}': {}", rule.id, e);
                    None
                }
            })
            .collect()
    }

    /// Set whether network operations are allowed
    pub fn allow_network(mut self, allowed: bool) -> Self {
        self.allow_network = allowed;
        self
    }

    /// Set whether file operations are allowed
    pub fn allow_file_ops(mut self, allowed: bool) -> Self {
        self.allow_file_ops = allowed;
        self
    }

    /// Set whether process execution is allowed
    pub fn allow_process_exec(mut self, allowed: bool) -> Self {
        self.allow_process_exec = allowed;
        self
    }

    /// Disable specific rules by ID
    pub fn disable_rules(mut self, rule_ids: &[&str]) -> Self {
        self.disabled_rules
            .extend(rule_ids.iter().map(|s| s.to_string()));
        self
    }

    /// Scan a script file for security issues
    pub fn scan_file(&self, script_path: &Path) -> Result<ScanResult> {
        let content = fs::read_to_string(script_path)
            .with_context(|| format!("Failed to read script file: {}", script_path.display()))?;

        self.scan_content(&content, script_path)
    }

    /// Scan script content for security issues
    pub fn scan_content(&self, content: &str, script_path: &Path) -> Result<ScanResult> {
        let language = detect_language(script_path);
        let mut issues = Vec::new();

        self.scan_with_rules(content, &language, &mut issues);

        let is_safe = issues
            .iter()
            .all(|issue| matches!(issue.severity, SecuritySeverity::Low));

        Ok(ScanResult { is_safe, issues })
    }

    /// Scan content using the configured rules
    fn scan_with_rules(&self, content: &str, language: &str, issues: &mut Vec<SecurityIssue>) {
        let lines: Vec<&str> = content.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            // Skip comment lines to reduce false positives
            let trimmed = line.trim();
            if Self::is_comment_line(trimmed, language) {
                continue;
            }

            for (rule, regex) in &self.rules {
                // Skip disabled rules
                if self.disabled_rules.contains(&rule.id) {
                    continue;
                }

                // Check if rule applies to this language
                if !rule.languages.is_empty() && !rule.languages.contains(&language.to_string()) {
                    continue;
                }

                if regex.is_match(line) {
                    // Check if this should be allowed based on scanner settings
                    let should_report = match rule.issue_type {
                        SecurityIssueType::NetworkRequest => !self.allow_network,
                        SecurityIssueType::FileOperation => !self.allow_file_ops,
                        SecurityIssueType::ProcessExecution => !self.allow_process_exec,
                        _ => true,
                    };

                    if should_report {
                        issues.push(SecurityIssue {
                            rule_id: rule.id.clone(),
                            severity: rule.severity.clone(),
                            issue_type: rule.issue_type.clone(),
                            line_number: line_idx + 1,
                            description: rule.description.clone(),
                            code_snippet: trimmed.to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Check if a line is a comment
    fn is_comment_line(line: &str, language: &str) -> bool {
        match language {
            "python" => line.starts_with('#'),
            "javascript" | "node" => {
                line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
            }
            _ => false,
        }
    }
}

/// Detect programming language from file extension
fn detect_language(script_path: &Path) -> String {
    script_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| match ext.to_lowercase().as_str() {
            "py" => "python",
            "js" => "javascript",
            "ts" => "javascript",
            _ => "unknown",
        })
        .unwrap_or("unknown")
        .to_string()
}

/// Format scan result for display.
/// When `compact` is true and issues > 5, groups by rule and shows summary.
pub fn format_scan_result(result: &ScanResult) -> String {
    format_scan_result_impl(result, false)
}

/// Compact format for chat/CLI when there are many issues.
pub fn format_scan_result_compact(result: &ScanResult) -> String {
    format_scan_result_impl(result, true)
}

fn format_scan_result_impl(result: &ScanResult, compact: bool) -> String {
    if result.issues.is_empty() {
        return "✅ No security issues found. Script is safe to execute.".to_string();
    }

    let use_compact = compact && result.issues.len() > 5;

    if use_compact {
        // Group by (rule_id, severity) and count
        use std::collections::HashMap;
        let mut groups: HashMap<(String, String), usize> = HashMap::new();
        for issue in &result.issues {
            let severity_str = match issue.severity {
                SecuritySeverity::Low => "Low",
                SecuritySeverity::Medium => "Medium",
                SecuritySeverity::High => "High",
                SecuritySeverity::Critical => "Critical",
            };
            *groups
                .entry((issue.rule_id.clone(), severity_str.to_string()))
                .or_insert(0) += 1;
        }

        let mut output = format!(
            "📋 Security Scan: {} item(s) flagged for review\n\n",
            result.issues.len()
        );
        for ((rule_id, severity_str), count) in groups {
            let icon = match severity_str.as_str() {
                "Low" => "🟢",
                "Medium" => "🟡",
                "High" => "🟠",
                "Critical" => "🔴",
                _ => "⚪",
            };
            output.push_str(&format!("  {} {}× {} [{}]\n", icon, count, rule_id, severity_str));
        }
        if result.is_safe {
            output.push_str("\n✅ All clear - only informational items found.");
        } else {
            output.push_str("\n📝 Review complete. Awaiting your approval to proceed.");
        }
        return output;
    }

    let mut output = format!(
        "📋 Security Scan: {} item(s) flagged for review\n\n",
        result.issues.len()
    );

    for (idx, issue) in result.issues.iter().enumerate() {
        let severity_icon = match issue.severity {
            SecuritySeverity::Low => "🟢",
            SecuritySeverity::Medium => "🟡",
            SecuritySeverity::High => "🟠",
            SecuritySeverity::Critical => "🔴",
        };
        let severity_label = match issue.severity {
            SecuritySeverity::Low => "Low",
            SecuritySeverity::Medium => "Medium",
            SecuritySeverity::High => "High",
            SecuritySeverity::Critical => "Critical",
        };

        output.push_str(&format!(
            "  {} #{} [{}] {}\n",
            severity_icon, idx + 1, severity_label, issue.issue_type
        ));
        output.push_str(&format!("     ├─ Rule: {}\n", issue.rule_id));
        output.push_str(&format!("     ├─ Line {}: {}\n", issue.line_number, issue.description));
        output.push_str(&format!("     └─ Code: {}\n\n", issue.code_snippet));
    }

    if result.is_safe {
        output.push_str("✅ All clear - only informational items found.");
    } else {
        output.push_str("📝 Review complete. Awaiting your approval to proceed.");
    }

    output
}

/// Format scan result as structured JSON for machine parsing
pub fn format_scan_result_json(result: &ScanResult) -> String {
    let severity_str = |s: &SecuritySeverity| -> &str {
        match s {
            SecuritySeverity::Low => "Low",
            SecuritySeverity::Medium => "Medium",
            SecuritySeverity::High => "High",
            SecuritySeverity::Critical => "Critical",
        }
    };

    let issues_json: Vec<serde_json::Value> = result
        .issues
        .iter()
        .map(|issue| {
            serde_json::json!({
                "rule_id": issue.rule_id,
                "severity": severity_str(&issue.severity),
                "issue_type": issue.issue_type.to_string(),
                "line_number": issue.line_number,
                "description": issue.description,
                "code_snippet": issue.code_snippet,
            })
        })
        .collect();

    let high_count = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, SecuritySeverity::High | SecuritySeverity::Critical))
        .count();
    let medium_count = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, SecuritySeverity::Medium))
        .count();
    let low_count = result
        .issues
        .iter()
        .filter(|i| matches!(i.severity, SecuritySeverity::Low))
        .count();

    let output = serde_json::json!({
        "is_safe": result.is_safe,
        "issues": issues_json,
        "high_severity_count": high_count,
        "medium_severity_count": medium_count,
        "low_severity_count": low_count,
    });

    serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
}
