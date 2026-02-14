"""MCP tool definitions for SkillLite server."""

from typing import List

try:
    from mcp.types import Tool
    MCP_AVAILABLE = True
except ImportError:
    MCP_AVAILABLE = False
    Tool = None  # type: ignore


def get_mcp_tools() -> List["Tool"]:
    """Return the list of MCP tools for SkillLite server."""
    if not MCP_AVAILABLE:
        return []

    return [
        # Skills management tools
        Tool(
            name="list_skills",
            description=(
                "List all available skills in the skills directory. "
                "Returns skill names, descriptions, and languages."
            ),
            inputSchema={
                "type": "object",
                "properties": {},
                "required": []
            }
        ),
        Tool(
            name="get_skill_info",
            description=(
                "Get detailed information about a specific skill, "
                "including its input schema, description, and usage."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "skill_name": {
                        "type": "string",
                        "description": "Name of the skill to get info for"
                    }
                },
                "required": ["skill_name"]
            }
        ),
        Tool(
            name="run_skill",
            description=(
                "Execute a skill with the given input parameters. "
                "Use list_skills to see available skills and "
                "get_skill_info to understand required parameters. "
                "IMPORTANT: If the skill has high-severity security issues, "
                "you MUST show the security report to the user and ASK for their explicit confirmation "
                "before setting confirmed=true. Do NOT auto-confirm without user approval."
            ),
            inputSchema={
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
                        "description": (
                            "Set to true ONLY after the user has explicitly approved execution. "
                            "You must ask the user for confirmation first."
                        )
                    },
                    "scan_id": {
                        "type": "string",
                        "description": "Scan ID from security review (required when confirmed=true)"
                    }
                },
                "required": ["skill_name"]
            }
        ),
        # Code execution tools
        Tool(
            name="scan_code",
            description=(
                "Scan code for security issues before execution. "
                "Returns a security report with any potential risks found. "
                "Use this before execute_code to review security implications."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "javascript", "bash"],
                        "description": "Programming language of the code"
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to scan for security issues"
                    }
                },
                "required": ["language", "code"]
            }
        ),
        Tool(
            name="execute_code",
            description=(
                "Execute code in a secure sandbox environment. "
                "IMPORTANT: If security issues are found, you MUST show the security report "
                "to the user and ASK for their explicit confirmation before setting confirmed=true. "
                "Do NOT auto-confirm without user approval."
            ),
            inputSchema={
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "javascript", "bash"],
                        "description": "Programming language to execute"
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to execute"
                    },
                    "confirmed": {
                        "type": "boolean",
                        "description": (
                            "Set to true ONLY after the user has explicitly approved execution. "
                            "You must ask the user for confirmation first."
                        ),
                        "default": False
                    },
                    "scan_id": {
                        "type": "string",
                        "description": (
                            "The scan_id from a previous scan_code call. "
                            "Required when confirmed=true to verify the code hasn't changed."
                        )
                    },
                    "sandbox_level": {
                        "type": "integer",
                        "enum": [1, 2, 3],
                        "description": (
                            "Sandbox security level: "
                            "1=no sandbox, 2=sandbox only, 3=sandbox+security scan (default)"
                        ),
                        "default": 3
                    }
                },
                "required": ["language", "code"]
            }
        )
    ]
