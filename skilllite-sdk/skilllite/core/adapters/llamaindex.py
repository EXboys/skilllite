"""
LlamaIndex adapter for SkillLite.

Provides SkillLiteToolSpec for integrating SkillLite skills into LlamaIndex agents.

This adapter inherits from BaseAdapter to share common logic with other adapters.
It NO LONGER depends on the LangChain adapter - all shared types come from the
protocols layer.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.llamaindex import SkillLiteToolSpec

    manager = SkillManager(skills_dir="./skills")
    tool_spec = SkillLiteToolSpec.from_manager(manager)
    tools = tool_spec.to_tool_list()

    # Use with LlamaIndex agent
    from llama_index.core.agent import ReActAgent
    agent = ReActAgent.from_tools(tools, llm=llm, verbose=True)

Security Confirmation:
    For sandbox level 3, the adapter supports security confirmation callbacks:

    def my_confirmation_callback(security_report: str, scan_id: str) -> bool:
        print(security_report)
        return input("Continue? [y/N]: ").lower() == 'y'

    tool_spec = SkillLiteToolSpec.from_manager(
        manager,
        sandbox_level=3,
        confirmation_callback=my_confirmation_callback
    )

Requirements:
    pip install skilllite[llamaindex]
"""

from typing import Any, List, Optional, TYPE_CHECKING

try:
    from llama_index.core.tools import FunctionTool, ToolMetadata
    from llama_index.core.tools.types import BaseTool as LlamaBaseTool
except ImportError as e:
    raise ImportError(
        "LlamaIndex adapter requires llama-index. "
        "Install with: pip install skilllite[llamaindex]"
    ) from e

# Import unified types from protocols layer - Single Source of Truth
# No longer depends on LangChain adapter
from ..protocols import (
    SecurityScanResult,
    ConfirmationCallback,
)
from .base import BaseAdapter

if TYPE_CHECKING:
    from ..manager import SkillManager


class SkillLiteToolSpec(BaseAdapter):
    """
    LlamaIndex ToolSpec for SkillLite - inherits from BaseAdapter.

    Provides a way to create LlamaIndex tools from SkillLite skills.

    This class inherits common functionality from BaseAdapter:
    - Unified execution through UnifiedExecutionService
    - Shared security scanning logic
    - Common confirmation flow

    Usage:
        manager = SkillManager(skills_dir="./skills")
        tool_spec = SkillLiteToolSpec.from_manager(manager)
        tools = tool_spec.to_tool_list()

        # Use with ReActAgent
        agent = ReActAgent.from_tools(tools, llm=llm)
        response = agent.chat("Your query")

    Security Confirmation (sandbox_level=3):
        def confirm(report: str, scan_id: str) -> bool:
            print(report)
            return input("Continue? [y/N]: ").lower() == 'y'

        tool_spec = SkillLiteToolSpec.from_manager(
            manager, sandbox_level=3, confirmation_callback=confirm
        )
    """

    @classmethod
    def from_manager(
        cls,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        sandbox_level: int = 3,
        confirmation_callback: Optional[ConfirmationCallback] = None
    ) -> "SkillLiteToolSpec":
        """
        Create a SkillLiteToolSpec from a SkillManager.

        Args:
            manager: SkillManager instance
            skill_names: Optional list of skill names to include
            allow_network: Whether to allow network access
            timeout: Execution timeout in seconds
            sandbox_level: Sandbox security level (1/2/3, default: 3)
            confirmation_callback: Callback for security confirmation

        Returns:
            SkillLiteToolSpec instance
        """
        return cls(
            manager=manager,
            sandbox_level=sandbox_level,
            allow_network=allow_network,
            timeout=timeout,
            confirmation_callback=confirmation_callback,
            skill_names=skill_names,
        )

    def _create_skill_function(self, skill_name: str):
        """
        Create a callable function for a skill.

        Uses BaseAdapter.execute_skill() which delegates to UnifiedExecutionService.
        """
        def skill_fn(**kwargs) -> str:
            try:
                # Use BaseAdapter's execute_skill method
                result = self.execute_skill(skill_name, kwargs)

                if result.success:
                    return result.output or "Execution completed successfully"
                else:
                    return f"Error: {result.error}"
            except Exception as e:
                return f"Execution failed: {str(e)}"

        return skill_fn

    def to_tools(self) -> List[LlamaBaseTool]:
        """
        Convert skills to LlamaIndex tools.

        Implements the abstract method from BaseAdapter.

        Returns:
            List of FunctionTool instances
        """
        tools = []

        for skill in self.get_executable_skills():
            # Create function for this skill
            fn = self._create_skill_function(skill.name)

            # Skill Usage Protocol - Phase 2 (Usage Phase):
            # Use full SKILL.md content as description so LLM can infer
            # correct parameters from usage examples.
            full_content = skill.get_full_content()
            tool_description = full_content or skill.description or f"Execute the {skill.name} skill"

            # Create FunctionTool
            tool = FunctionTool.from_defaults(
                fn=fn,
                name=skill.name,
                description=tool_description
            )
            tools.append(tool)

        return tools

    def to_tool_list(self) -> List[LlamaBaseTool]:
        """
        Convert all skills to a list of LlamaIndex tools.

        This method maintains backward compatibility.
        Delegates to to_tools() method.

        Returns:
            List of FunctionTool instances
        """
        return self.to_tools()


__all__ = ["SkillLiteToolSpec", "SecurityScanResult", "ConfirmationCallback"]

