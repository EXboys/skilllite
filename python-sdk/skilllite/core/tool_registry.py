"""
ToolRegistry - Unified registration and lookup for builtin tools (file + memory).

Replaces hardcoded if/else routing in chat_session. New tools: register and done.
"""

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional


@dataclass
class ToolEntry:
    """Single tool: schema for LLM + handler for execution."""

    schema: Dict[str, Any]  # OpenAI format: {"type": "function", "function": {...}}
    handler: Callable[[Dict[str, Any]], str]


class ToolRegistry:
    """
    Registry for builtin tools. Populated per-session when context is available.

    Usage:
        registry = ToolRegistry()
        registry.register("read_file", schema, lambda t: builtin_executor(t))
        tools = registry.get_tool_definitions()
        result = registry.execute("read_file", tool_input)
    """

    def __init__(self) -> None:
        self._tools: Dict[str, ToolEntry] = {}

    def register(self, name: str, schema: Dict[str, Any], handler: Callable[[Dict[str, Any]], str]) -> None:
        """Register a tool: name, OpenAI-format schema, and execution handler."""
        self._tools[name] = ToolEntry(schema=schema, handler=handler)

    def get(self, name: str) -> Optional[ToolEntry]:
        """Get tool entry by name."""
        return self._tools.get(name)

    def has(self, name: str) -> bool:
        """Check if tool is registered."""
        return name in self._tools

    def get_tool_definitions(self) -> List[Dict[str, Any]]:
        """Get all tool schemas in OpenAI format for LLM."""
        return [entry.schema for entry in self._tools.values()]

    def execute(self, tool_name: str, tool_input: Dict[str, Any]) -> str:
        """Execute a tool by name. Returns result string or error message."""
        entry = self.get(tool_name)
        if not entry:
            return f"Error: No executor for tool: {tool_name}"
        try:
            return entry.handler(tool_input)
        except Exception as e:
            return f"Error: {e}"
