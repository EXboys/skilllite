"""
SkillLite Core - Core execution engine and data structures.

This module contains the essential components that should never be modified
by external tools or AI assistants without explicit user permission.

Core Components:
- SkillManager: Main interface for managing skills (facade)
- SkillRegistry: Skill registration and discovery
- ToolBuilder: Tool definition generation and schema inference
- PromptBuilder: System prompt context generation
- ToolCallHandler: LLM response handling and tool execution
- SkillExecutor: Executes skills using the skillbox binary
- AgenticLoop: Handles LLM-tool interaction loops
- SkillInfo: Information container for skills
- ToolDefinition/ToolResult: Tool protocol adapters
- ExecutionResult: Result of skill execution

These components are protected and should only be modified by:
1. The original maintainer
2. Explicit user requests
3. Critical bug fixes

All other functionality (CLI, quick start, utilities, etc.) should be
implemented in modules outside this core directory.
"""

from .manager import SkillManager
from .executor import SkillExecutor, ExecutionResult
from .agent_proxy import AgenticLoop, AgenticLoopClaudeNative, ApiFormat
from .skill_info import SkillInfo
from .tools import ToolDefinition, ToolUseRequest, ToolResult, ToolFormat
from .metadata import SkillMetadata, NetworkPolicy, parse_skill_metadata
from .registry import SkillRegistry
from .tool_builder import ToolBuilder
from .prompt_builder import PromptBuilder
from .handler import ToolCallHandler
from .tool_registry import ToolRegistry, ToolEntry

__all__ = [
    # Core Management (Facade)
    "SkillManager",
    # Modular Components
    "SkillRegistry",
    "ToolBuilder",
    "PromptBuilder",
    "ToolCallHandler",
    # Skill Info
    "SkillInfo",
    "SkillMetadata",
    "NetworkPolicy",
    # Execution
    "SkillExecutor",
    "ExecutionResult",
    # Loops (proxy to skillbox agent-rpc)
    "AgenticLoop",
    "AgenticLoopClaudeNative",
    "ApiFormat",
    # Tools
    "ToolDefinition",
    "ToolUseRequest",
    "ToolResult",
    "ToolFormat",
    "ToolRegistry",
    "ToolEntry",
    # Utilities
    "parse_skill_metadata",
    # Adapters (lazy loaded)
    "adapters",
]
