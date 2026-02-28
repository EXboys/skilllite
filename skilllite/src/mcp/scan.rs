//! Security scanning, code execution, and sandbox logic for the MCP server.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

use skilllite_sandbox::runner::{ResourceLimits, SandboxLevel};
use skilllite_sandbox::security::scanner::ScriptScanner;
use skilllite_sandbox::security::types::{
    ScanResult, SecurityIssue, SecurityIssueType, SecuritySeverity,
};

use super::state::{CachedScan, McpServer};

/// Handle the `scan_code` tool call.
pub(super) fn handle_scan_code(server: &mut McpServer, arguments: &Value) -> Result<String> {
    let language = arguments
        .get("language")
        .and_then(|v| v.as_str())
        .context("language is required")?;
    let code = arguments
        .get("code")
        .and_then(|v| v.as_str())
        .context("code is required")?;

    let (scan_result, scan_id, code_hash) = perform_scan(server, language, code)?;

    format_scan_response(&scan_result, &scan_id, &code_hash)
}

/// Build a fail-secure ScanResult when the scan process itself fails.
/// Returns High severity (requires_confirmation) so user can review and confirm.
pub(super) fn scan_error_result(err: &str) -> ScanResult {
    ScanResult {
        is_safe: false,
        issues: vec![SecurityIssue {
            rule_id: "scan-error".to_string(),
            severity: SecuritySeverity::High,
            issue_type: SecurityIssueType::ScanError,
            line_number: 0,
            description: format!("Security scan failed: {}. Manual review required.", err),
            code_snippet: String::new(),
        }],
    }
}

/// Perform a security scan and cache the result.
/// Fail-secure: on scan exception, returns a ScanResult with requires_confirmation
/// instead of propagating Err (aligned with Python SDK behavior).
pub(super) fn perform_scan(server: &mut McpServer, language: &str, code: &str) -> Result<(ScanResult, String, String)> {
    let code_hash = McpServer::generate_code_hash(language, code);
    let scan_id = McpServer::generate_scan_id(&code_hash);

    let scan_result = match do_scan(language, code) {
        Ok(r) => r,
        Err(e) => {
            // Fail-secure: return ScanResult requiring confirmation, not Err
            let err_result = scan_error_result(&e.to_string());
            server.scan_cache.insert(scan_id.clone(), CachedScan {
                scan_result: err_result.clone(),
                code_hash: code_hash.clone(),
                language: language.to_string(),
                code: code.to_string(),
                created_at: Instant::now(),
            });
            return Ok((err_result, scan_id, code_hash));
        }
    };

    // Cache the result
    server.scan_cache.insert(scan_id.clone(), CachedScan {
        scan_result: scan_result.clone(),
        code_hash: code_hash.clone(),
        language: language.to_string(),
        code: code.to_string(),
        created_at: Instant::now(),
    });

    Ok((scan_result, scan_id, code_hash))
}

/// Inner scan logic — may return Err on temp file or scanner failure.
fn do_scan(language: &str, code: &str) -> Result<ScanResult> {
    let ext = match language {
        "python" => ".py",
        "javascript" | "node" => ".js",
        "bash" | "shell" => ".sh",
        _ => ".txt",
    };
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().join(format!("scan{}", ext));
    std::fs::write(&temp_path, code)?;

    let scanner = ScriptScanner::new();
    scanner.scan_file(&temp_path)
}

/// Format a scan result as a human-readable response.
pub(super) fn format_scan_response(scan_result: &ScanResult, scan_id: &str, code_hash: &str) -> Result<String> {
    let has_high_severity = scan_result.issues.iter().any(|i| {
        matches!(i.severity, SecuritySeverity::High | SecuritySeverity::Critical)
    });
    let has_critical = scan_result.issues.iter().any(|i| {
        matches!(i.severity, SecuritySeverity::Critical)
    });

    let mut output = String::new();

    if scan_result.issues.is_empty() {
        output.push_str("✅ No security issues found. Code is safe to execute.\n\n");
    } else {
        output.push_str(&format!(
            "📋 Security Scan: {} issue(s) found\n\n",
            scan_result.issues.len()
        ));

        for (idx, issue) in scan_result.issues.iter().enumerate() {
            let severity_label = match issue.severity {
                SecuritySeverity::Low => "Low",
                SecuritySeverity::Medium => "Medium",
                SecuritySeverity::High => "High",
                SecuritySeverity::Critical => "Critical",
            };
            output.push_str(&format!(
                "  #{} [{}] {} - Line {}: {}\n    Code: {}\n\n",
                idx + 1,
                severity_label,
                issue.issue_type,
                issue.line_number,
                issue.description,
                issue.code_snippet,
            ));
        }

        if has_critical {
            output.push_str("🚫 BLOCKED: Critical security issues found. Execution is not permitted.\n");
        } else if has_high_severity {
            output.push_str("⚠️ High-severity issues found. User confirmation is required before execution.\n");
        }
    }

    // Always include scan details as JSON
    let details = json!({
        "scan_id": scan_id,
        "code_hash": code_hash,
        "is_safe": scan_result.is_safe,
        "issues_count": scan_result.issues.len(),
        "has_high_severity": has_high_severity,
        "has_critical": has_critical,
        "requires_confirmation": has_high_severity && !has_critical,
    });

    output.push_str(&format!("\n```json\n{}\n```", serde_json::to_string_pretty(&details)?));

    Ok(output)
}

/// Handle the `execute_code` tool call.
pub(super) fn handle_execute_code(server: &mut McpServer, arguments: &Value) -> Result<String> {
    let language = arguments
        .get("language")
        .and_then(|v| v.as_str())
        .context("language is required")?;
    let code = arguments
        .get("code")
        .and_then(|v| v.as_str())
        .context("code is required")?;
    let confirmed = arguments
        .get("confirmed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let scan_id = arguments
        .get("scan_id")
        .and_then(|v| v.as_str());
    let sandbox_level_arg = arguments
        .get("sandbox_level")
        .and_then(|v| v.as_u64())
        .map(|v| v as u8);

    let sandbox_level = SandboxLevel::from_env_or_cli(sandbox_level_arg);

    // For Level 3: automatic security scan
    if sandbox_level == SandboxLevel::Level3 {
        if confirmed {
            // Verify scan_id
            let sid = scan_id.context(
                "scan_id is required when confirmed=true. Call scan_code first to get a scan_id."
            )?;

            // Extract needed data within a scoped borrow, then consume on success
            let (cached_code_hash, issues_count, has_critical) = {
                let cached = server.scan_cache.get(sid).context(
                    "Invalid or expired scan_id. The scan may have expired (TTL: 300s). Please call scan_code again."
                )?;
                (
                    cached.code_hash.clone(),
                    cached.scan_result.issues.len(),
                    cached.scan_result.issues.iter().any(|i| {
                        matches!(i.severity, SecuritySeverity::Critical)
                    }),
                )
            };

            // Verify code_hash matches
            let current_hash = McpServer::generate_code_hash(language, code);
            if cached_code_hash != current_hash {
                anyhow::bail!(
                    "Code has changed since the scan. Please call scan_code again with the new code."
                );
            }

            // Check for critical issues — cannot override
            if has_critical {
                skilllite_core::observability::security_scan_rejected(
                    "execute_code",
                    sid,
                    issues_count,
                );
                anyhow::bail!(
                    "Execution blocked: Critical security issues cannot be overridden even with confirmation."
                );
            }

            // One-time consumption: remove scan_id to prevent replay (F4)
            server.scan_cache.remove(sid);

            // Audit: execution approved
            skilllite_core::observability::audit_confirmation_response("execute_code", true, "user");
            skilllite_core::observability::security_scan_approved(
                "execute_code",
                sid,
                issues_count,
            );
        } else {
            // Auto-scan
            let (scan_result, new_scan_id, code_hash) = perform_scan(server, language, code)?;

            let has_high = scan_result.issues.iter().any(|i| {
                matches!(i.severity, SecuritySeverity::High | SecuritySeverity::Critical)
            });

            if has_high {
                // Return scan report, requiring confirmation
                return format_scan_response(&scan_result, &new_scan_id, &code_hash);
            }
            // No high-severity issues — proceed to execution
        }
    }

    // Execute the code
    execute_code_in_sandbox(language, code, sandbox_level)
}

/// Execute code in the sandbox.
pub(super) fn execute_code_in_sandbox(language: &str, code: &str, sandbox_level: SandboxLevel) -> Result<String> {
    let ext = match language {
        "python" => ".py",
        "javascript" | "node" => ".js",
        "bash" | "shell" => ".sh",
        _ => anyhow::bail!("Unsupported language: {}", language),
    };

    // Create a temporary skill-like directory
    let temp_dir = tempfile::tempdir()?;
    let script_name = format!("main{}", ext);
    let script_path = temp_dir.path().join(&script_name);
    std::fs::write(&script_path, code)?;

    // Create minimal metadata
    let lang_str = match language {
        "python" => "python",
        "javascript" | "node" => "node",
        "bash" | "shell" => "shell",
        _ => "python",
    };

    let config = skilllite_sandbox::runner::SandboxConfig {
        name: "execute_code".to_string(),
        entry_point: script_name,
        language: lang_str.to_string(),
        network_enabled: false,
        network_outbound: Vec::new(),
        uses_playwright: false,
    };

    let limits = ResourceLimits::from_env();
    let runtime = skilllite_sandbox::env::builder::build_runtime_paths(&PathBuf::new());

    let output = skilllite_sandbox::runner::run_in_sandbox_with_limits_and_level(
        temp_dir.path(),
        &runtime,
        &config,
        "{}",
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

