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
        self.scan_entropy(content, &language, &mut issues);
        self.scan_base64(content, &language, &mut issues);
        self.scan_multistage_payload(content, &language, &mut issues);

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

    // ─── B2: Base64 payload detection ────────────────────────────────────────

    /// Detect long base64 literals and explicit base64-decode calls (B2).
    ///
    /// Severity rules:
    /// - Quoted base64 literal ≥ 50 chars + decode call on same line + dangerous decoded
    ///   content → **Critical**
    /// - Quoted base64 literal ≥ 50 chars + decode call on same line → **High**
    /// - Explicit decode call (b64decode / atob / Buffer.from base64) without visible
    ///   literal, or long literal alone → **Medium**
    fn scan_base64(&self, content: &str, language: &str, issues: &mut Vec<SecurityIssue>) {
        // Regex: quoted base64 string of ≥ 50 base64-alphabet chars (with optional padding)
        let b64_literal_re =
            match Regex::new(r#"['"]([A-Za-z0-9+/]{50,}={0,2})['"]"#) {
                Ok(r) => r,
                Err(_) => return,
            };

        // Language-specific decode-call patterns
        let decode_re_py =
            Regex::new(r"base64\s*\.\s*(?:b64decode|decodebytes|decode)\s*\(|codecs\s*\.\s*decode\s*\(")
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap());
        let decode_re_js =
            Regex::new(r#"atob\s*\(|Buffer\s*\.\s*from\s*\([^)]*['"]base64['"]"#)
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap());

        let decode_re: &Regex = match language {
            "python" => &decode_re_py,
            "javascript" | "node" => &decode_re_js,
            _ => return, // unknown language — skip
        };

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || Self::is_comment_line(trimmed, language) {
                continue;
            }

            let has_decode_call = decode_re.is_match(line);
            let b64_cap = b64_literal_re.captures(line);

            match (has_decode_call, b64_cap) {
                // decode call + visible base64 literal on the same line
                (true, Some(cap)) => {
                    let b64_str = &cap[1];
                    let (severity, detail) =
                        if let Some(danger) = analyze_decoded_base64(b64_str) {
                            (
                                SecuritySeverity::Critical,
                                format!(
                                    "Base64 decode call with literal that decodes to dangerous content: {}",
                                    danger
                                ),
                            )
                        } else {
                            (
                                SecuritySeverity::High,
                                format!(
                                    "Base64 decode call with embedded literal ({} chars) — \
                                     possible encoded payload",
                                    b64_str.len()
                                ),
                            )
                        };
                    issues.push(SecurityIssue {
                        rule_id: "base64-encoded-payload".to_string(),
                        severity,
                        issue_type: SecurityIssueType::EncodedPayload,
                        line_number: line_idx + 1,
                        description: detail,
                        code_snippet: trimmed.chars().take(120).collect(),
                    });
                }
                // decode call without visible literal
                (true, None) => {
                    issues.push(SecurityIssue {
                        rule_id: "base64-decode-call".to_string(),
                        severity: SecuritySeverity::Medium,
                        issue_type: SecurityIssueType::EncodedPayload,
                        line_number: line_idx + 1,
                        description:
                            "Base64/codec decode call detected — verify the decoded content is safe"
                                .to_string(),
                        code_snippet: trimmed.chars().take(120).collect(),
                    });
                }
                // long base64 literal without an explicit decode call on this line
                (false, Some(cap)) => {
                    let b64_str = &cap[1];
                    issues.push(SecurityIssue {
                        rule_id: "base64-literal".to_string(),
                        severity: SecuritySeverity::Medium,
                        issue_type: SecurityIssueType::EncodedPayload,
                        line_number: line_idx + 1,
                        description: format!(
                            "Long base64-encoded string literal ({} chars) — possible encoded payload",
                            b64_str.len()
                        ),
                        code_snippet: trimmed.chars().take(120).collect(),
                    });
                }
                (false, None) => {}
            }
        }
    }

    // ─── B3: Multi-stage payload detection ───────────────────────────────────

    /// Detect "download → decode → execute" chain patterns across a file (B3).
    ///
    /// Three families are matched over all lines:
    /// - **Download**: urllib/requests/fetch/curl/wget…
    /// - **Decode**: base64.b64decode/codecs.decode/bytes.fromhex/atob…
    /// - **Execute**: exec/eval/subprocess/os.system/child_process/spawn…
    ///
    /// Severity:
    /// - 2 out of 3 families → **High** (suspicious combination)
    /// - All 3 families → **Critical** (classic staged payload chain)
    fn scan_multistage_payload(
        &self,
        content: &str,
        language: &str,
        issues: &mut Vec<SecurityIssue>,
    ) {
        let (dl_re, dec_re, exec_re) = match language {
            "python" => (
                Regex::new(
                    r"urllib\.request|requests\s*\.\s*(?:get|post|Session)|httplib|http\.client\
                      |wget\.download|urlopen\s*\(",
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
                Regex::new(
                    r"base64\s*\.\s*(?:b64decode|decodebytes|decode)|codecs\s*\.\s*decode\
                      |bytes\.fromhex\s*\(",
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
                Regex::new(
                    r"(?:^|[^.\w])exec\s*\(|eval\s*\(|subprocess\s*\.\s*(?:run|call|Popen)\
                      |os\s*\.\s*system\s*\(",
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            ),
            "javascript" | "node" => (
                Regex::new(
                    r#"fetch\s*\(|axios\s*\.\s*(?:get|post)|http\s*\.\s*(?:get|request)\s*\(|https\s*\.\s*(?:get|request)\s*\(|require\s*\(\s*['"]node-fetch['"]"#,
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
                Regex::new(
                    r#"atob\s*\(|Buffer\s*\.\s*from\s*\([^)]*['"]base64['"]|\.toString\s*\(\s*['"]base64['"]"#,
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
                Regex::new(
                    r#"eval\s*\(|new\s+Function\s*\(|child_process\s*\.\s*(?:exec|spawn|execSync)|require\s*\(\s*['"]vm['"]"#,
                )
                .unwrap_or_else(|_| Regex::new(r"$^").unwrap()),
            ),
            _ => return,
        };

        let mut dl_line: Option<usize> = None;
        let mut dec_line: Option<usize> = None;
        let mut exec_line: Option<usize> = None;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || Self::is_comment_line(trimmed, language) {
                continue;
            }
            if dl_line.is_none() && dl_re.is_match(line) {
                dl_line = Some(line_idx + 1);
            }
            if dec_line.is_none() && dec_re.is_match(line) {
                dec_line = Some(line_idx + 1);
            }
            if exec_line.is_none() && exec_re.is_match(line) {
                exec_line = Some(line_idx + 1);
            }
        }

        let matched: Vec<(&str, usize)> = [
            ("download", dl_line),
            ("decode", dec_line),
            ("execute", exec_line),
        ]
        .iter()
        .filter_map(|(name, opt)| opt.map(|ln| (*name, ln)))
        .collect();

        if matched.len() >= 2 {
            let severity = if matched.len() == 3 {
                SecuritySeverity::Critical
            } else {
                SecuritySeverity::High
            };
            let stages: Vec<String> = matched
                .iter()
                .map(|(name, ln)| format!("{}(line {})", name, ln))
                .collect();
            let description = format!(
                "Multi-stage payload chain detected: {} — \
                 {} out of 3 stages (download/decode/execute) found in this file",
                stages.join(" → "),
                matched.len()
            );
            // Report at the first matched line
            let first_line = matched.iter().map(|(_, ln)| *ln).min().unwrap_or(1);
            issues.push(SecurityIssue {
                rule_id: "multistage-payload".to_string(),
                severity,
                issue_type: SecurityIssueType::MultiStagePayload,
                line_number: first_line,
                description,
                code_snippet: format!(
                    "stages: {}",
                    matched
                        .iter()
                        .map(|(n, _)| *n)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
    }

    /// Scan for high-entropy lines that indicate obfuscated / encoded payloads.
    ///
    /// Lines shorter than `MIN_LEN` chars are skipped (too short to be meaningful).
    /// Lines whose Shannon entropy exceeds `THRESHOLD` bits/char are flagged as
    /// `SecuritySeverity::Medium` with issue type `ObfuscatedCode`.
    fn scan_entropy(&self, content: &str, language: &str, issues: &mut Vec<SecurityIssue>) {
        /// Minimum printable characters required before entropy is computed.
        const MIN_LEN: usize = 20;
        /// Entropy threshold in bits per character (base-2).
        const THRESHOLD: f64 = 4.5;

        for (line_idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // Skip blank lines and comment lines
            if trimmed.len() < MIN_LEN || Self::is_comment_line(trimmed, language) {
                continue;
            }

            if shannon_entropy(trimmed) > THRESHOLD {
                issues.push(SecurityIssue {
                    rule_id: "entropy-obfuscation".to_string(),
                    severity: SecuritySeverity::Medium,
                    issue_type: SecurityIssueType::ObfuscatedCode,
                    line_number: line_idx + 1,
                    description: format!(
                        "High-entropy line ({:.2} bits/char > {:.1} threshold) — possible obfuscated or encoded payload",
                        shannon_entropy(trimmed),
                        THRESHOLD,
                    ),
                    code_snippet: trimmed.chars().take(120).collect(),
                });
            }
        }
    }
}

/// Compute Shannon entropy (bits per character) of a string.
///
/// H = -∑ p_i · log₂(p_i)  where p_i = count(byte_i) / total_bytes
///
/// Returns 0.0 for empty strings.
fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut freq = [0u32; 256];
    for &b in s.as_bytes() {
        freq[b as usize] += 1;
    }
    let total = s.len() as f64;
    freq.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / total;
            -p * p.log2()
        })
        .sum()
}

// ─── Base64 helpers (no external crate) ──────────────────────────────────────

/// Decode a standard base64 string. Returns `None` on invalid input.
///
/// Pure Rust, ~25 lines — avoids adding a `base64` crate dependency.
fn base64_decode_safe(input: &str) -> Option<Vec<u8>> {
    const TABLE: [u8; 128] = {
        let mut t = [255u8; 128];
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0usize;
        while i < chars.len() {
            t[chars[i] as usize] = i as u8;
            i += 1;
        }
        t
    };
    let input = input.trim_end_matches('=');
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 1);
    let bytes = input.as_bytes();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &b in bytes {
        if b as usize >= 128 {
            return None;
        }
        let val = TABLE[b as usize];
        if val == 255 {
            return None;
        }
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Try to decode a base64 string and check if the decoded bytes contain
/// known dangerous patterns. Returns a short reason string if dangerous.
fn analyze_decoded_base64(b64: &str) -> Option<&'static str> {
    let decoded = base64_decode_safe(b64)?;
    // Work with both raw bytes and lossy UTF-8
    let text = String::from_utf8_lossy(&decoded);
    let lower = text.to_lowercase();

    // Shell execution
    if lower.contains("/bin/sh") || lower.contains("/bin/bash") || lower.contains("cmd.exe") {
        return Some("decoded content contains shell reference (/bin/sh, bash, cmd.exe)");
    }
    // Download tools
    if lower.contains("wget ") || lower.contains("curl ") || lower.contains("powershell") {
        return Some("decoded content contains download tool (wget/curl/powershell)");
    }
    // Privilege escalation
    if lower.contains("chmod +x") || lower.contains("chmod 777") || lower.contains("sudo ") {
        return Some("decoded content contains privilege escalation (chmod/sudo)");
    }
    // Code execution functions
    if lower.contains("exec(") || lower.contains("eval(") || lower.contains("import socket") {
        return Some("decoded content contains code execution (exec/eval/socket)");
    }
    // Subprocess / os
    if lower.contains("subprocess") || lower.contains("os.system") {
        return Some("decoded content contains subprocess/os.system call");
    }
    // Network reverse shell indicators
    if lower.contains("connect(") && (lower.contains("socket") || lower.contains("127.0.0")) {
        return Some("decoded content contains socket connect — possible reverse shell");
    }
    None
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
