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

mod state;
mod tools;
mod scan;
mod handlers;

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;

use state::McpServer;
use tools::get_mcp_tools;
use handlers::{handle_initialize, handle_list_skills, handle_get_skill_info, handle_run_skill};
use scan::{handle_scan_code, handle_execute_code};

/// Maximum JSON-RPC request size (10 MB) to prevent OOM DoS.
const MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;

// ═══════════════════════════════════════════════════════════════════════════════
// Size-Limited Stdin Reader (F3: OOM DoS prevention)
// ═══════════════════════════════════════════════════════════════════════════════

/// Read a single line from `reader`, enforcing [`MAX_REQUEST_SIZE`].
/// Returns `Ok(None)` on EOF, `Ok(Some(line))` on success.
/// Oversized lines are skipped (bytes discarded) and an error is returned.
fn read_line_limited(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    loop {
        let available = match reader.fill_buf() {
            Ok(b) => b,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return if buf.is_empty() {
                Ok(None)
            } else {
                if buf.last() == Some(&b'\r') { buf.pop(); }
                String::from_utf8(buf)
                    .map(Some)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))
            };
        }
        match available.iter().position(|&b| b == b'\n') {
            Some(pos) => {
                if buf.len() + pos > MAX_REQUEST_SIZE {
                    reader.consume(pos + 1);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Request exceeds 10MB size limit",
                    ));
                }
                buf.extend_from_slice(&available[..pos]);
                reader.consume(pos + 1);
                if buf.last() == Some(&b'\r') { buf.pop(); }
                return String::from_utf8(buf)
                    .map(Some)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"));
            }
            None => {
                let len = available.len();
                if buf.len() + len > MAX_REQUEST_SIZE {
                    reader.consume(len);
                    skip_until_newline(reader);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Request exceeds 10MB size limit",
                    ));
                }
                buf.extend_from_slice(available);
                reader.consume(len);
            }
        }
    }
}

/// Discard bytes from `reader` until a newline or EOF, using only the
/// internal buffer (no heap allocation for the discarded data).
fn skip_until_newline(reader: &mut impl BufRead) {
    loop {
        match reader.fill_buf() {
            Ok(b) if b.is_empty() => break,
            Ok(b) => {
                if let Some(pos) = b.iter().position(|&c| c == b'\n') {
                    reader.consume(pos + 1);
                    break;
                }
                let len = b.len();
                reader.consume(len);
            }
            Err(_) => break,
        }
    }
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
    let mut reader = BufReader::new(stdin.lock());

    loop {
        let line = match read_line_limited(&mut reader) {
            Ok(None) => break,       // EOF
            Ok(Some(l)) => l,
            Err(e) => {
                let err_resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32600, "message": format!("Request size error: {}", e)}
                });
                writeln!(stdout, "{}", err_resp)?;
                stdout.flush()?;
                continue;
            }
        };
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

