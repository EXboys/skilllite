//! MCP (Model Context Protocol) Server — Phase 3.5a
//!
//! Implements the standard MCP JSON-RPC 2.0 over stdio protocol.
//! Provides 5 tools: list_skills, get_skill_info, run_skill, scan_code, execute_code.
//!
//! This replaces the Python `mcp/server.py` implementation. Once this is
//! complete, `skilllite mcp` can be removed from the Python SDK.
//!
//! Protocol flow:
//!   1. Client sends `initialize` → Server returns capabilities
//!   2. Client sends `notifications/initialized`
//!   3. Client sends `tools/list` → Server returns 5 tool definitions
//!   4. Client sends `tools/call` → Server executes tool, returns result
//!
//! Security model (two-phase confirmation):
//!   - `scan_code` / auto-scan in `run_skill` / `execute_code` → returns scan_id
//!   - Caller re-calls with `confirmed=true` + `scan_id` → executes
//!   - Hard-blocked issues (Critical severity) cannot be overridden

use anyhow::{Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::sandbox::runner::{ResourceLimits, SandboxLevel};
use crate::sandbox::security::scanner::ScriptScanner;
use crate::sandbox::security::types::{
    ScanResult, SecurityIssue, SecurityIssueType, SecuritySeverity,
};
use crate::skill::metadata;

// ═══════════════════════════════════════════════════════════════════════════════
// MCP Server State
// ═══════════════════════════════════════════════════════════════════════════════

/// Cached scan result with TTL.
struct CachedScan {
    scan_result: ScanResult,
    code_hash: String,
    #[allow(dead_code)]
    language: String,
    #[allow(dead_code)]
    code: String,
    created_at: Instant,
}

/// Session-level confirmation cache: skill_name → code_hash.
/// Avoids re-scanning the same skill if its code hasn't changed.
struct ConfirmedSkill {
    code_hash: String,
}

/// MCP Server state maintained across requests.
struct McpServer {
    /// Skills directory path
    skills_dir: PathBuf,
    /// Scan result cache: scan_id → CachedScan (TTL: 300s)
    scan_cache: HashMap<String, CachedScan>,
    /// Session-level confirmation cache: skill_name → ConfirmedSkill
    confirmed_skills: HashMap<String, ConfirmedSkill>,
    /// Scan cache TTL
    cache_ttl: Duration,
}

impl McpServer {
    fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            scan_cache: HashMap::new(),
            confirmed_skills: HashMap::new(),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Remove expired scan cache entries.
    fn cleanup_expired_scans(&mut self) {
        let now = Instant::now();
        self.scan_cache.retain(|_, v| now.duration_since(v.created_at) < self.cache_ttl);
    }

    /// Generate a code hash: SHA256(language:code) full hexdigest.
    fn generate_code_hash(language: &str, code: &str) -> String {
        let content = format!("{}:{}", language, code);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate a scan_id: SHA256(code_hash:timestamp)[:16].
    fn generate_scan_id(code_hash: &str) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            .to_string();
        let content = format!("{}:{}", code_hash, timestamp);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }

    /// Compute a hash of a skill's entry point code for confirmation cache.
    fn compute_skill_hash(skill_dir: &Path, entry_point: &str) -> String {
        let mut hasher = Sha256::new();
        let entry_path = if !entry_point.is_empty() {
            skill_dir.join(entry_point)
        } else {
            skill_dir.join("SKILL.md")
        };
        if let Ok(content) = std::fs::read(&entry_path) {
            hasher.update(&content);
        }
        if let Ok(skill_md) = std::fs::read(skill_dir.join("SKILL.md")) {
            hasher.update(&skill_md);
        }
        hex::encode(hasher.finalize())[..16].to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MCP Protocol: Tool Definitions
// ═══════════════════════════════════════════════════════════════════════════════

/// Return the 5 MCP tool definitions.
fn get_mcp_tools() -> Vec<Value> {
    vec![
        json!({
            "name": "list_skills",
            "description": "List all available skills with their names, descriptions, and languages. Returns a formatted list of installed skills.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        json!({
            "name": "get_skill_info",
            "description": "Get detailed information about a specific skill, including its input schema, description, and usage. Returns the full SKILL.md content.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Name of the skill to get info for"
                    }
                },
                "required": ["skill_name"]
            }
        }),
        json!({
            "name": "run_skill",
            "description": "Execute a skill with the given input parameters. Use list_skills to see available skills and get_skill_info to understand required parameters. IMPORTANT: If the skill has high-severity security issues, you MUST show the security report to the user and ASK for their explicit confirmation before setting confirmed=true. Do NOT auto-confirm without user approval.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Name of the skill to execute"
                    },
                    "input": {
                        "type": "object",
                        "description": "Input parameters for the skill"
                    },
                    "confirmed": {
                        "type": "boolean",
                        "description": "Set to true ONLY after the user has explicitly approved execution. You must ask the user for confirmation first."
                    },
                    "scan_id": {
                        "type": "string",
                        "description": "Scan ID from security review (required when confirmed=true)"
                    }
                },
                "required": ["skill_name"]
            }
        }),
        json!({
            "name": "scan_code",
            "description": "Scan code for security issues before execution. Returns a security report with any potential risks found. Use this before execute_code to review security implications.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "Programming language of the code",
                        "enum": ["python", "javascript", "bash"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to scan for security issues"
                    }
                },
                "required": ["language", "code"]
            }
        }),
        json!({
            "name": "execute_code",
            "description": "Execute code in a secure sandbox environment. IMPORTANT: If security issues are found, you MUST show the security report to the user and ASK for their explicit confirmation before setting confirmed=true. Do NOT auto-confirm without user approval.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "description": "Programming language to execute",
                        "enum": ["python", "javascript", "bash"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to execute"
                    },
                    "confirmed": {
                        "type": "boolean",
                        "default": false,
                        "description": "Set to true ONLY after the user has explicitly approved execution. You must ask the user for confirmation first."
                    },
                    "scan_id": {
                        "type": "string",
                        "description": "The scan_id from a previous scan_code call. Required when confirmed=true to verify the code hasn't changed."
                    },
                    "sandbox_level": {
                        "type": "integer",
                        "default": 3,
                        "description": "Sandbox security level: 1=no sandbox, 2=sandbox only, 3=sandbox+security scan (default)",
                        "enum": [1, 2, 3]
                    }
                },
                "required": ["language", "code"]
            }
        }),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════════
// MCP Protocol: Main Server Loop
// ═══════════════════════════════════════════════════════════════════════════════

/// Run the MCP server over stdio (JSON-RPC 2.0).
///
/// This is the entry point for `skilllite mcp-serve`.
pub fn serve_mcp_stdio(skills_dir: &str) -> Result<()> {
    let skills_path = PathBuf::from(skills_dir);
    let mut server = McpServer::new(skills_path);

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line.context("Failed to read stdin")?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("Parse error: {}", e)}
                });
                writeln!(stdout, "{}", err_resp)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = request.get("id").cloned();
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        // Periodic cleanup of expired scan cache entries
        server.cleanup_expired_scans();

        match method {
            // ─── Lifecycle ──────────────────────────────────────────────
            "initialize" => {
                let result = handle_initialize(&params);
                send_response(&mut stdout, id, Ok(result))?;
            }
            "notifications/initialized" | "initialized" => {
                // Notification — no response required
            }
            "ping" => {
                send_response(&mut stdout, id, Ok(json!({})))?;
            }

            // ─── Tools ─────────────────────────────────────────────────
            "tools/list" => {
                let result = json!({ "tools": get_mcp_tools() });
                send_response(&mut stdout, id, Ok(result))?;
            }
            "tools/call" => {
                let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                let result = match tool_name {
                    "list_skills" => handle_list_skills(&server),
                    "get_skill_info" => handle_get_skill_info(&server, &arguments),
                    "run_skill" => handle_run_skill(&mut server, &arguments),
                    "scan_code" => handle_scan_code(&mut server, &arguments),
                    "execute_code" => handle_execute_code(&mut server, &arguments),
                    _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
                };

                match result {
                    Ok(content) => {
                        let resp = json!({
                            "content": [{"type": "text", "text": content}],
                            "isError": false
                        });
                        send_response(&mut stdout, id, Ok(resp))?;
                    }
                    Err(e) => {
                        let resp = json!({
                            "content": [{"type": "text", "text": format!("Error: {}", e)}],
                            "isError": true
                        });
                        send_response(&mut stdout, id, Ok(resp))?;
                    }
                }
            }

            // ─── Resources / Prompts (not implemented) ──────────────────
            "resources/list" => {
                send_response(&mut stdout, id, Ok(json!({"resources": []})))?;
            }
            "prompts/list" => {
                send_response(&mut stdout, id, Ok(json!({"prompts": []})))?;
            }

            // ─── Unknown ────────────────────────────────────────────────
            _ => {
                if id.is_some() {
                    let err_resp = json!({
                        "code": -32601,
                        "message": format!("Method not found: {}", method)
                    });
                    send_response(&mut stdout, id, Err(err_resp))?;
                }
                // Notifications (no id) are silently ignored per MCP spec
            }
        }
    }

    Ok(())
}

/// Send a JSON-RPC 2.0 response.
fn send_response(stdout: &mut io::Stdout, id: Option<Value>, result: Result<Value, Value>) -> Result<()> {
    let id = id.unwrap_or(Value::Null);
    let resp = match result {
        Ok(res) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": res
        }),
        Err(err) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": err
        }),
    };
    writeln!(stdout, "{}", resp)?;
    stdout.flush()?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: initialize
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_initialize(_params: &Value) -> Value {
    json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "skilllite-mcp-server",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: list_skills
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_list_skills(server: &McpServer) -> Result<String> {
    let skills_dir = &server.skills_dir;
    if !skills_dir.exists() {
        return Ok("No skills directory found. Use `skilllite add` to install skills.".to_string());
    }

    let mut skills = Vec::new();

    // Scan subdirectories for skills
    let entries = std::fs::read_dir(skills_dir)
        .with_context(|| format!("Failed to read skills directory: {}", skills_dir.display()))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }

        match metadata::parse_skill_metadata(&path) {
            Ok(meta) => {
                let lang = meta.language.as_deref().unwrap_or("auto");
                let desc = meta.description.as_deref().unwrap_or("No description");
                skills.push(json!({
                    "name": meta.name,
                    "description": desc,
                    "language": lang
                }));
            }
            Err(e) => {
                tracing::warn!("Failed to parse skill at {}: {}", path.display(), e);
            }
        }
    }

    if skills.is_empty() {
        return Ok("No skills installed. Use `skilllite add` to install skills.".to_string());
    }

    let result = json!({
        "skills": skills,
        "count": skills.len()
    });

    Ok(serde_json::to_string_pretty(&result)?)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: get_skill_info
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_get_skill_info(server: &McpServer, arguments: &Value) -> Result<String> {
    let skill_name = arguments
        .get("skill_name")
        .and_then(|v| v.as_str())
        .context("skill_name is required")?;

    let skill_dir = server.skills_dir.join(skill_name);
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        anyhow::bail!("Skill '{}' not found in {}", skill_name, server.skills_dir.display());
    }

    let skill_content = std::fs::read_to_string(&skill_md_path)
        .with_context(|| format!("Failed to read SKILL.md for '{}'", skill_name))?;

    // Also parse metadata for structured info
    let meta = metadata::parse_skill_metadata(&skill_dir)?;

    // Check for multi-script tools
    let scripts_dir = skill_dir.join("scripts");
    let mut scripts = Vec::new();
    if scripts_dir.exists() && scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".py") || fname.ends_with(".js") || fname.ends_with(".ts") || fname.ends_with(".sh") {
                    if !fname.starts_with("test_") && !fname.ends_with("_test.py") && !fname.starts_with('.') && fname != "__init__.py" {
                        scripts.push(fname);
                    }
                }
            }
        }
    }

    let mut output = format!("# Skill: {}\n\n", skill_name);
    output.push_str(&skill_content);

    if !scripts.is_empty() {
        output.push_str("\n\n## Available Scripts\n\n");
        for script in &scripts {
            output.push_str(&format!("- `scripts/{}`\n", script));
        }
    }

    // Include input schema if available
    if !meta.entry_point.is_empty() {
        let entry_path = skill_dir.join(&meta.entry_point);
        if entry_path.extension().and_then(|e| e.to_str()) == Some("py") {
            if let Some(schema) = parse_argparse_schema_from_path(&entry_path) {
                output.push_str(&format!(
                    "\n\n## Input Schema\n\n```json\n{}\n```\n",
                    serde_json::to_string_pretty(&schema)?
                ));
            }
        }
    }

    Ok(output)
}

/// Parse argparse schema from a Python file path (reuses agent/skills.rs logic).
fn parse_argparse_schema_from_path(script_path: &Path) -> Option<Value> {
    let content = std::fs::read_to_string(script_path).ok()?;

    let arg_re = regex::Regex::new(
        r#"\.add_argument\s*\(\s*['"]([^'"]+)['"](?:\s*,\s*['"]([^'"]+)['"])?([^)]*)\)"#,
    ).ok()?;

    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for caps in arg_re.captures_iter(&content) {
        let arg_name = caps.get(1)?.as_str();
        let second_arg = caps.get(2).map(|m| m.as_str());
        let kwargs_str = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let (param_name, is_positional) = if arg_name.starts_with("--") {
            (arg_name[2..].replace('-', "_"), false)
        } else if arg_name.starts_with('-') {
            if let Some(s) = second_arg {
                if s.starts_with("--") {
                    (s[2..].replace('-', "_"), false)
                } else {
                    (arg_name[1..].to_string(), false)
                }
            } else {
                (arg_name[1..].to_string(), false)
            }
        } else {
            (arg_name.replace('-', "_"), true)
        };

        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), json!("string"));

        if let Some(help_cap) = regex::Regex::new(r#"help\s*=\s*['"]([^'"]+)['"]"#)
            .ok().and_then(|re| re.captures(kwargs_str))
        {
            prop.insert("description".to_string(), json!(help_cap.get(1).unwrap().as_str()));
        }

        if let Some(type_cap) = regex::Regex::new(r"type\s*=\s*(\w+)")
            .ok().and_then(|re| re.captures(kwargs_str))
        {
            match type_cap.get(1).unwrap().as_str() {
                "int" => { prop.insert("type".to_string(), json!("integer")); }
                "float" => { prop.insert("type".to_string(), json!("number")); }
                "bool" => { prop.insert("type".to_string(), json!("boolean")); }
                _ => {}
            }
        }

        if kwargs_str.contains("store_true") || kwargs_str.contains("store_false") {
            prop.insert("type".to_string(), json!("boolean"));
        }

        if is_positional || kwargs_str.contains("required=True") {
            required.push(param_name.clone());
        }

        properties.insert(param_name, Value::Object(prop));
    }

    if properties.is_empty() {
        return None;
    }

    Some(json!({
        "type": "object",
        "properties": properties,
        "required": required
    }))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: scan_code
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_scan_code(server: &mut McpServer, arguments: &Value) -> Result<String> {
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
fn scan_error_result(err: &str) -> ScanResult {
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
fn perform_scan(server: &mut McpServer, language: &str, code: &str) -> Result<(ScanResult, String, String)> {
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
fn format_scan_response(scan_result: &ScanResult, scan_id: &str, code_hash: &str) -> Result<String> {
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

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: execute_code
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_execute_code(server: &mut McpServer, arguments: &Value) -> Result<String> {
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

            let cached = server.scan_cache.get(sid).context(
                "Invalid or expired scan_id. The scan may have expired (TTL: 300s). Please call scan_code again."
            )?;

            // Verify code_hash matches
            let current_hash = McpServer::generate_code_hash(language, code);
            if cached.code_hash != current_hash {
                anyhow::bail!(
                    "Code has changed since the scan. Please call scan_code again with the new code."
                );
            }

            // Check for critical issues — cannot override
            let has_critical = cached.scan_result.issues.iter().any(|i| {
                matches!(i.severity, SecuritySeverity::Critical)
            });
            if has_critical {
                crate::observability::security_scan_rejected(
                    "execute_code",
                    sid,
                    cached.scan_result.issues.len(),
                );
                anyhow::bail!(
                    "Execution blocked: Critical security issues cannot be overridden even with confirmation."
                );
            }

            // Audit: execution approved
            crate::observability::audit_confirmation_response("execute_code", true, "user");
            crate::observability::security_scan_approved(
                "execute_code",
                sid,
                cached.scan_result.issues.len(),
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
fn execute_code_in_sandbox(language: &str, code: &str, sandbox_level: SandboxLevel) -> Result<String> {
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

    let config = crate::sandbox::runner::SandboxConfig {
        name: "execute_code".to_string(),
        entry_point: script_name,
        language: lang_str.to_string(),
        network_enabled: false,
        network_outbound: Vec::new(),
        uses_playwright: false,
    };

    let limits = ResourceLimits::from_env();
    let runtime = crate::env::builder::build_runtime_paths(&PathBuf::new());

    let output = crate::sandbox::runner::run_in_sandbox_with_limits_and_level(
        temp_dir.path(),
        &runtime,
        &config,
        "{}",
        limits,
        sandbox_level,
    )?;

    Ok(output)
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler: run_skill
// ═══════════════════════════════════════════════════════════════════════════════

fn handle_run_skill(server: &mut McpServer, arguments: &Value) -> Result<String> {
    let skill_name = arguments
        .get("skill_name")
        .and_then(|v| v.as_str())
        .context("skill_name is required")?;
    let input = arguments
        .get("input")
        .cloned()
        .unwrap_or(json!({}));
    let confirmed = arguments
        .get("confirmed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let scan_id = arguments
        .get("scan_id")
        .and_then(|v| v.as_str());

    // Find the skill
    let skill_dir = server.skills_dir.join(skill_name);
    if !skill_dir.exists() || !skill_dir.join("SKILL.md").exists() {
        anyhow::bail!("Skill '{}' not found in {}", skill_name, server.skills_dir.display());
    }

    let meta = metadata::parse_skill_metadata(&skill_dir)?;
    let sandbox_level = SandboxLevel::from_env_or_cli(None);

    // Security check for Level 3
    if sandbox_level == SandboxLevel::Level3 {
        let code_hash = McpServer::compute_skill_hash(&skill_dir, &meta.entry_point);

        // Check session-level confirmation cache
        let already_confirmed = server.confirmed_skills.get(skill_name)
            .map_or(false, |c| c.code_hash == code_hash);

        if !already_confirmed {
            if confirmed {
                // Verify scan_id
                let sid = scan_id.context(
                    "scan_id is required when confirmed=true. The skill security scan must be reviewed first."
                )?;

                if !server.scan_cache.contains_key(sid) {
                    anyhow::bail!(
                        "Invalid or expired scan_id. Please review the security report and try again."
                    );
                }

                // Check for critical issues
                if let Some(cached) = server.scan_cache.get(sid) {
                    let has_critical = cached.scan_result.issues.iter().any(|i| {
                        matches!(i.severity, SecuritySeverity::Critical)
                    });
                    if has_critical {
                        anyhow::bail!(
                            "Execution blocked: Critical security issues cannot be overridden."
                        );
                    }
                }

                // Audit: scan approved
                crate::observability::audit_confirmation_response(skill_name, true, "user");
                crate::observability::security_scan_approved(
                    skill_name,
                    sid,
                    server.scan_cache.get(sid).map_or(0, |c| c.scan_result.issues.len()),
                );

                // Cache confirmation
                server.confirmed_skills.insert(
                    skill_name.to_string(),
                    ConfirmedSkill { code_hash },
                );
            } else {
                // Perform security scan on entry point
                let entry_path = if !meta.entry_point.is_empty() {
                    skill_dir.join(&meta.entry_point)
                } else {
                    // Multi-script or no entry point — scan SKILL.md content as proxy
                    skill_dir.join("SKILL.md")
                };

                if entry_path.exists() {
                    let code = std::fs::read_to_string(&entry_path).unwrap_or_default();
                    let language = if entry_path.extension().and_then(|e| e.to_str()) == Some("py") {
                        "python"
                    } else if entry_path.extension().and_then(|e| e.to_str()) == Some("js") {
                        "javascript"
                    } else {
                        "bash"
                    };

                    let (scan_result, new_scan_id, new_code_hash) = perform_scan(server, language, &code)?;

                    let has_high = scan_result.issues.iter().any(|i| {
                        matches!(i.severity, SecuritySeverity::High | SecuritySeverity::Critical)
                    });

                    if has_high {
                        // Audit: confirmation requested
                        crate::observability::audit_confirmation_requested(
                            skill_name,
                            &new_code_hash,
                            scan_result.issues.len(),
                            "High",
                        );
                        crate::observability::security_scan_high(
                            skill_name,
                            "High",
                            &serde_json::json!(scan_result.issues.iter().map(|i| {
                                serde_json::json!({
                                    "rule": i.rule_id,
                                    "severity": format!("{:?}", i.severity),
                                    "description": i.description,
                                })
                            }).collect::<Vec<_>>()),
                        );
                        return format_scan_response(&scan_result, &new_scan_id, &new_code_hash);
                    }
                }

                // No high-severity issues — cache and proceed
                server.confirmed_skills.insert(
                    skill_name.to_string(),
                    ConfirmedSkill { code_hash },
                );
            }
        }
    }

    // Execute the skill
    let input_json = serde_json::to_string(&input)?;

    if meta.entry_point.is_empty() {
        // Prompt-only skill or multi-script skill without entry_point
        return Ok(format!(
            "Skill '{}' has no entry point. It is a prompt-only skill or uses multi-script tools.\n\
             Use get_skill_info to see available scripts.",
            skill_name
        ));
    }

    // Setup environment
    let cache_dir = crate::config::CacheConfig::cache_dir();
    let env_path = crate::env::builder::ensure_environment(
        &skill_dir,
        &meta,
        cache_dir.as_deref(),
    )?;

    let limits = ResourceLimits::from_env();

    let runtime = crate::env::builder::build_runtime_paths(&env_path);
    let config = crate::sandbox::runner::SandboxConfig {
        name: meta.name.clone(),
        entry_point: meta.entry_point.clone(),
        language: metadata::detect_language(&skill_dir, &meta),
        network_enabled: meta.network.enabled,
        network_outbound: meta.network.outbound.clone(),
        uses_playwright: meta.uses_playwright(),
    };
    let output = crate::sandbox::runner::run_in_sandbox_with_limits_and_level(
        &skill_dir,
        &runtime,
        &config,
        &input_json,
        limits,
        sandbox_level,
    )?;

    Ok(output)
}
