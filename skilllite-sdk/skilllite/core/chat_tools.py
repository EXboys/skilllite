"""
Chat tools - memory_search and memory_write for LLM.

These tools allow the LLM to store and retrieve information from the
persistent memory (BM25 index). Requires skillbox built with --features chat.
"""

from typing import Any, Dict, List, Optional


def get_memory_tools() -> List[Dict[str, Any]]:
    """
    Get memory tool definitions in OpenAI-compatible format.

    Returns:
        List of tool definitions for memory_search and memory_write
    """
    return [
        {
            "type": "function",
            "function": {
                "name": "memory_search",
                "description": "Search the agent's memory for relevant information. Use when you need to recall past context, user preferences, or stored facts.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (keywords or natural language)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results (default: 10)",
                            "default": 10
                        }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "memory_write",
                "description": "Store information in the agent's memory for future retrieval. Use for: user preferences, important facts, decisions, or context to remember.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "rel_path": {
                            "type": "string",
                            "description": "Logical path/category (e.g. MEMORY.md, preferences/theme.md)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to store"
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append to existing content; otherwise overwrite",
                            "default": False
                        }
                    },
                    "required": ["rel_path", "content"]
                }
            }
        },
    ]


def create_memory_tool_executor(workspace_path: Optional[str] = None):
    """
    Create executor for memory tools. Calls skillbox IPC.

    Args:
        workspace_path: Workspace path for memory storage

    Returns:
        Executor function: (tool_input) -> str
    """
    def executor(tool_input: Dict[str, Any]) -> str:
        tool_name = tool_input.get("tool_name", "")
        if tool_name not in ("memory_search", "memory_write"):
            raise ValueError(f"Unknown memory tool: {tool_name}")

        from ..sandbox.skillbox import find_binary
        from ..sandbox.skillbox.ipc_client import SkillboxIPCClient

        binary = find_binary()
        if not binary:
            return "Error: skillbox binary not found"

        client = SkillboxIPCClient(binary_path=binary)
        try:
            if tool_name == "memory_search":
                query = tool_input.get("query", "")
                limit = tool_input.get("limit", 10)
                hits = client.memory_search(
                    query=query,
                    limit=limit,
                    workspace_path=workspace_path,
                )
                if not hits:
                    return "No relevant memory found."
                parts = [f"Found {len(hits)} result(s):\n"]
                for i, h in enumerate(hits, 1):
                    c = h.get("content", "")
                    score = h.get("score")
                    parts.append(f"{i}. {c[:400]}{'...' if len(c) > 400 else ''}")
                    if score is not None:
                        parts.append(f"   (score: {score})")
                return "\n".join(parts)
            else:  # memory_write
                rel_path = tool_input.get("rel_path", "MEMORY.md")
                content = tool_input.get("content", "")
                append = tool_input.get("append", False)
                client.memory_write(
                    rel_path=rel_path,
                    content=content,
                    append=append,
                    workspace_path=workspace_path,
                )
                return f"Successfully stored in {rel_path} ({len(content)} chars)"
        except Exception as e:
            return f"Error: {e}"
        finally:
            client.close()

    return executor
