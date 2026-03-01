//! Skill execution stdio RPC: JSON-RPC 2.0 over stdio.
//!
//! **Entry**: `skilllite serve --stdio`
//!
//! **Scope**: Skill execution (run/exec/bash), executor RPC (session/transcript/memory/plan),
//! and agent helpers (build_skills_context, list_tools). One request → one response.
//!
//! **Not this module**: For Agent Chat streaming (JSON-Lines events, one request → many events),
//! see [`skilllite_agent::rpc`]. That uses `skilllite agent-rpc` as a separate process.
//!
//! Protocol:
//!
//! Request: `{"jsonrpc":"2.0","id":1,"method":"run"|"exec"|...","params":{...}}`
//! Response: `{"jsonrpc":"2.0","id":1,"result":{...}}` or `{"jsonrpc":"2.0","id":1,"error":{...}}`

use anyhow::{Context, Result};
use crate::commands::execute;
use skilllite_core::path_validation;
use skilllite_sandbox::runner::{ResourceLimits, SandboxLevel};
use serde_json::{json, Value};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::mpsc;
use std::thread;

/// Maximum JSON-RPC request size (10 MB) to prevent OOM DoS.
const MAX_REQUEST_SIZE: usize = 10 * 1024 * 1024;

/// Run the skill execution stdio RPC daemon.
///
/// Reads JSON-RPC requests from stdin (one per line), writes responses to stdout.
/// Uses rayon thread pool for concurrent request handling.
pub fn serve_stdio() -> Result<()> {
    skilllite_core::config::init_daemon_env();

    let (tx, rx) = mpsc::channel::<(Value, std::result::Result<Value, String>)>();

    // Writer thread: receives results and writes to stdout (stdout is not Sync)
    let writer_handle = thread::spawn(move || -> Result<()> {
        let mut stdout = io::stdout();
        for (id, result) in rx {
            match result {
                Ok(res) => {
                    let resp = json!({"jsonrpc": "2.0", "id": id, "result": res});
                    writeln!(stdout, "{}", resp)?;
                }
                Err(msg) => {
                    let err_resp = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {"code": -32603, "message": msg}
                    });
                    writeln!(stdout, "{}", err_resp)?;
                }
            }
            stdout.flush()?;
        }
        Ok(())
    });

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let (done_tx, done_rx) = mpsc::channel::<()>();
    let mut pending = 0usize;

    loop {
        let line = match read_line_limited(&mut reader) {
            Ok(None) => break,       // EOF
            Ok(Some(l)) => l,
            Err(e) => {
                let _ = tx.send((Value::Null, Err(format!("Request size error: {}", e))));
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
                let _ = tx.send((Value::Null, Err(format!("Parse error: {}", e))));
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let params = request
            .get("params")
            .cloned()
            .unwrap_or(Value::Object(serde_json::Map::new()));

        pending += 1;
        let tx = tx.clone();
        let done_tx = done_tx.clone();
        rayon::spawn(move || {
            let result = dispatch_request(&method, &params);
            let _ = tx.send((id, result.map_err(|e| e.to_string())));
            let _ = done_tx.send(());
        });
    }

    for _ in 0..pending {
        let _ = done_rx.recv();
    }
    drop(tx);
    writer_handle.join().map_err(|_| anyhow::anyhow!("Writer thread panicked"))??;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Size-Limited Stdin Reader (F3: OOM DoS prevention)
// ═══════════════════════════════════════════════════════════════════════════════

/// Read a single line from `reader`, enforcing [`MAX_REQUEST_SIZE`].
/// Returns `Ok(None)` on EOF, `Ok(Some(line))` on success.
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

/// Dispatch JSON-RPC request to the appropriate handler.
fn dispatch_request(method: &str, params: &Value) -> Result<Value> {
    match method {
        "run" => handle_run(params),
        "exec" => handle_exec(params),
        "bash" => handle_bash(params),
        #[cfg(feature = "executor")]
        "session_create" => skilllite_executor::rpc::handle_session_create(params),
        #[cfg(feature = "executor")]
        "session_get" => skilllite_executor::rpc::handle_session_get(params),
        #[cfg(feature = "executor")]
        "session_update" => skilllite_executor::rpc::handle_session_update(params),
        #[cfg(feature = "executor")]
        "transcript_append" => skilllite_executor::rpc::handle_transcript_append(params),
        #[cfg(feature = "executor")]
        "transcript_read" => skilllite_executor::rpc::handle_transcript_read(params),
        #[cfg(feature = "executor")]
        "transcript_ensure" => skilllite_executor::rpc::handle_transcript_ensure(params),
        #[cfg(feature = "executor")]
        "memory_write" => skilllite_executor::rpc::handle_memory_write(params),
        #[cfg(feature = "executor")]
        "memory_search" => skilllite_executor::rpc::handle_memory_search(params),
        #[cfg(feature = "executor")]
        "token_count" => skilllite_executor::rpc::handle_token_count(params),
        #[cfg(feature = "executor")]
        "plan_textify" => skilllite_executor::rpc::handle_plan_textify(params),
        #[cfg(feature = "executor")]
        "plan_write" => skilllite_executor::rpc::handle_plan_write(params),
        #[cfg(feature = "executor")]
        "plan_read" => skilllite_executor::rpc::handle_plan_read(params),
        #[cfg(feature = "agent")]
        "build_skills_context" => handle_build_skills_context(params),
        #[cfg(feature = "agent")]
        "list_tools" => handle_list_tools(params),
        _ => anyhow::bail!("Method not found: {}", method),
    }
}

fn handle_run(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = execute::run_skill(skill_dir, input_json, allow_network, cache_dir_ref, limits, sandbox_level)?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_exec(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let script_path = p.get("script_path").and_then(|v| v.as_str()).context("script_path required")?;
    let input_json = p.get("input_json").and_then(|v| v.as_str()).context("input_json required")?;
    let args = p.get("args").and_then(|v| v.as_str()).map(|s| s.to_string());
    let allow_network = p.get("allow_network").and_then(|v| v.as_bool()).unwrap_or(false);
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let cache_dir_ref = cache_dir.as_ref();
    let max_memory = p.get("max_memory").and_then(|v| v.as_u64());
    let timeout = p.get("timeout").and_then(|v| v.as_u64());
    let sandbox_level = p.get("sandbox_level").and_then(|v| v.as_u64()).map(|u| u as u8);

    let sandbox_level = SandboxLevel::from_env_or_cli(sandbox_level);
    let limits = ResourceLimits::from_env()
        .with_cli_overrides(max_memory, timeout);

    let output = execute::exec_script(
        skill_dir,
        script_path,
        input_json,
        args.as_ref(),
        allow_network,
        cache_dir_ref,
        limits,
        sandbox_level,
    )?;
    Ok(json!({
        "output": output,
        "exit_code": 0
    }))
}

fn handle_bash(params: &Value) -> Result<Value> {
    let p = params.as_object().context("params must be object")?;
    let skill_dir = p.get("skill_dir").and_then(|v| v.as_str()).context("skill_dir required")?;
    let command = p.get("command").and_then(|v| v.as_str()).context("command required")?;
    let cache_dir = p.get("cache_dir").and_then(|v| v.as_str()).map(|s| s.to_string());
    let timeout = p.get("timeout").and_then(|v| v.as_u64()).unwrap_or(120);
    let cwd = p.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());

    let output = execute::bash_command(skill_dir, command, cache_dir.as_ref(), timeout, cwd.as_ref())?;
    Ok(serde_json::from_str(&output).unwrap_or_else(|_| json!({
        "output": output,
        "exit_code": 0
    })))
}

#[cfg(feature = "agent")]
fn handle_build_skills_context(params: &Value) -> Result<Value> {
    use skilllite_agent::prompt::{build_skills_context, PromptMode};
    use skilllite_agent::skills;

    let p = params.as_object().context("params must be object")?;
    let skills_dir = p.get("skills_dir").and_then(|v| v.as_str()).context("skills_dir required")?;
    let mode_str = p.get("mode").and_then(|v| v.as_str()).unwrap_or("progressive");
    let skills_filter: Option<Vec<String>> = p
        .get("skills")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    let skills_path = path_validation::validate_path_under_root(skills_dir, "skills_dir")?;
    let skills_path_str = skills_path.to_string_lossy().to_string();

    let loaded = skills::load_skills(&[skills_path_str]);
    let loaded: Vec<_> = if let Some(ref filter) = skills_filter {
        loaded
            .into_iter()
            .filter(|s| filter.contains(&s.name))
            .collect()
    } else {
        loaded
    };

    let mode = match mode_str {
        "summary" => PromptMode::Summary,
        "standard" => PromptMode::Standard,
        "full" => PromptMode::Full,
        _ => PromptMode::Progressive,
    };

    let context = build_skills_context(&loaded, mode);
    Ok(json!({ "context": context }))
}

#[cfg(feature = "agent")]
pub fn handle_list_tools(params: &Value) -> Result<Value> {
    use skilllite_agent::skills;

    let p = params.as_object().context("params must be object")?;
    let skills_dir = p.get("skills_dir").and_then(|v| v.as_str()).context("skills_dir required")?;
    let skills_filter: Option<Vec<String>> = p
        .get("skills")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
    let format_str = p.get("format").and_then(|v| v.as_str()).unwrap_or("openai");

    let skills_path = path_validation::validate_path_under_root(skills_dir, "skills_dir")?;
    let skills_path_str = skills_path.to_string_lossy().to_string();

    let loaded = skills::load_skills(&[skills_path_str]);
    let loaded: Vec<_> = if let Some(ref filter) = skills_filter {
        loaded
            .into_iter()
            .filter(|s| filter.contains(&s.name))
            .collect()
    } else {
        loaded
    };

    let mut tools: Vec<Value> = Vec::new();
    let mut tool_meta: serde_json::Map<String, Value> = serde_json::Map::new();
    for skill in &loaded {
        let skill_dir_str = skill.skill_dir.to_string_lossy().to_string();
        for td in &skill.tool_definitions {
            let tool_name = &td.function.name;
            let formatted = match format_str {
                "claude" => td.to_claude_format(),
                _ => serde_json::to_value(td).unwrap_or_default(),
            };
            tools.push(formatted);
            let script_path = skill.multi_script_entries.get(tool_name).cloned();
            let entry_point = if script_path.is_none() && !skill.metadata.entry_point.is_empty() {
                Some(skill.metadata.entry_point.clone())
            } else {
                script_path.clone()
            };
            let is_bash = skill.metadata.is_bash_tool_skill();
            tool_meta.insert(
                tool_name.clone(),
                json!({
                    "skill_dir": skill_dir_str,
                    "script_path": script_path,
                    "entry_point": entry_point,
                    "is_bash": is_bash,
                    "capabilities": skill.metadata.capabilities
                }),
            );
        }
    }

    Ok(json!({ "tools": tools, "tool_meta": tool_meta }))
}