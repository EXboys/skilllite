//! Integration tests for the MCP (Model Context Protocol) server.
//!
//! The MCP server uses JSON-RPC 2.0 over stdio. These tests launch the
//! `skilllite mcp` process, feed JSON-RPC requests via stdin, and assert
//! on the JSON-RPC responses read from stdout.

mod common;

use common::{create_calculator_skill, create_prompt_only_skill, run_in_dir_with_stdin, stdout_str};
use serde_json::{json, Value};

/// Build a JSON-RPC 2.0 request string (terminated with newline).
fn jsonrpc_request(id: u64, method: &str, params: Value) -> String {
    let req = json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params
    });
    format!("{}\n", req)
}

/// Parse all JSON-RPC responses from stdout (one per line).
fn parse_responses(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Find a response with the given id.
fn find_response(responses: &[Value], id: u64) -> Option<&Value> {
    responses.iter().find(|r| r["id"] == json!(id))
}

// ═══════════════════════════════════════════════════════════════════════════════
// MCP Lifecycle
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_initialize_returns_capabilities() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let stdin = jsonrpc_request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response for id=1");
    assert!(
        resp.get("result").is_some(),
        "initialize should return result"
    );
    let result = &resp["result"];
    assert!(
        result.get("capabilities").is_some(),
        "result should have capabilities"
    );
    assert!(
        result.get("serverInfo").is_some(),
        "result should have serverInfo"
    );
}

#[test]
fn mcp_ping_returns_empty_object() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(1, "ping", json!({}));
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response for ping");
    assert!(resp.get("result").is_some());
}

// ═══════════════════════════════════════════════════════════════════════════════
// tools/list
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_tools_list_returns_tool_definitions() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(1, "tools/list", json!({}));
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response for tools/list");
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools should be an array");

    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(
        tool_names.contains(&"list_skills"),
        "should have list_skills tool, got: {:?}",
        tool_names
    );
    assert!(
        tool_names.contains(&"run_skill"),
        "should have run_skill tool"
    );
    assert!(
        tool_names.contains(&"scan_code"),
        "should have scan_code tool"
    );

    for tool in tools {
        assert!(
            tool.get("name").is_some(),
            "each tool should have a name"
        );
        assert!(
            tool.get("description").is_some(),
            "each tool should have a description"
        );
        assert!(
            tool.get("inputSchema").is_some(),
            "each tool should have inputSchema"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// tools/call — list_skills
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_list_skills_with_fixture() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());
    create_prompt_only_skill(tmp.path());

    let stdin = jsonrpc_request(
        1,
        "tools/call",
        json!({
            "name": "list_skills",
            "arguments": {}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response for list_skills");
    let result = &resp["result"];
    assert_eq!(result["isError"], false);

    let content_text = result["content"][0]["text"]
        .as_str()
        .expect("should have text content");
    assert!(
        content_text.contains("calculator"),
        "list_skills should include calculator"
    );
}

#[test]
fn mcp_list_skills_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(
        1,
        "tools/call",
        json!({
            "name": "list_skills",
            "arguments": {}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    assert_eq!(resp["result"]["isError"], false);
}

// ═══════════════════════════════════════════════════════════════════════════════
// tools/call — get_skill_info
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_get_skill_info_existing() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let stdin = jsonrpc_request(
        1,
        "tools/call",
        json!({
            "name": "get_skill_info",
            "arguments": {"skill_name": "calculator"}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    let result = &resp["result"];
    assert_eq!(result["isError"], false);
    let text = result["content"][0]["text"].as_str().unwrap_or("");
    assert!(
        text.contains("calculator"),
        "get_skill_info should include skill details"
    );
}

#[test]
fn mcp_get_skill_info_nonexistent() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(
        1,
        "tools/call",
        json!({
            "name": "get_skill_info",
            "arguments": {"skill_name": "does-not-exist"}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    let result = &resp["result"];
    assert_eq!(
        result["isError"], true,
        "nonexistent skill should be an error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Error handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_invalid_json_returns_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = "this is not valid json\n";
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    assert!(!responses.is_empty(), "should return an error response");
    let resp = &responses[0];
    assert!(
        resp.get("error").is_some(),
        "should have error field for invalid JSON"
    );
    let code = resp["error"]["code"].as_i64().unwrap_or(0);
    assert_eq!(code, -32700, "should be JSON parse error code");
}

#[test]
fn mcp_unknown_method_returns_method_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(1, "nonexistent/method", json!({}));
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    assert!(
        resp.get("error").is_some(),
        "unknown method should return error"
    );
    let code = resp["error"]["code"].as_i64().unwrap_or(0);
    assert_eq!(code, -32601, "should be method not found code");
}

#[test]
fn mcp_unknown_tool_returns_error() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(
        1,
        "tools/call",
        json!({
            "name": "nonexistent_tool",
            "arguments": {}
        }),
    );

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    let result = &resp["result"];
    assert_eq!(result["isError"], true);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Multi-request session
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_multi_request_session() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let mut stdin = String::new();
    // 1. initialize
    stdin.push_str(&jsonrpc_request(
        1,
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }),
    ));
    // 2. initialized notification (no id → no response expected)
    stdin.push_str(&format!(
        "{}\n",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    ));
    // 3. tools/list
    stdin.push_str(&jsonrpc_request(2, "tools/list", json!({})));
    // 4. list_skills
    stdin.push_str(&jsonrpc_request(
        3,
        "tools/call",
        json!({"name": "list_skills", "arguments": {}}),
    ));

    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    assert!(
        responses.len() >= 3,
        "should have at least 3 responses (init, tools/list, list_skills), got {}",
        responses.len()
    );

    assert!(find_response(&responses, 1).is_some(), "init response");
    assert!(
        find_response(&responses, 2).is_some(),
        "tools/list response"
    );
    assert!(
        find_response(&responses, 3).is_some(),
        "list_skills response"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// resources/list and prompts/list (stub endpoints)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn mcp_resources_list_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(1, "resources/list", json!({}));
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    let resources = resp["result"]["resources"]
        .as_array()
        .expect("resources should be an array");
    assert!(resources.is_empty());
}

#[test]
fn mcp_prompts_list_returns_empty() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let stdin = jsonrpc_request(1, "prompts/list", json!({}));
    let out = run_in_dir_with_stdin(&["mcp", "-s", ".skills"], tmp.path(), &stdin);
    assert!(out.status.success());

    let responses = parse_responses(&stdout_str(&out));
    let resp = find_response(&responses, 1).expect("should have response");
    let prompts = resp["result"]["prompts"]
        .as_array()
        .expect("prompts should be an array");
    assert!(prompts.is_empty());
}
