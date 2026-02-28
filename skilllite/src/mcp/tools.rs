//! MCP tool definitions — the 5 tools exposed by the MCP server.

use serde_json::{json, Value};

/// Return the 5 MCP tool definitions.
pub(super) fn get_mcp_tools() -> Vec<Value> {
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

