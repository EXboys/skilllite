"""
LangChain adapter for SkillLite.

Provides SkillLiteTool and SkillLiteToolkit for integrating SkillLite
skills into LangChain agents.

Usage:
    from skilllite import SkillManager
    from skilllite.core.adapters.langchain import SkillLiteToolkit

    manager = SkillManager(skills_dir="./skills")
    tools = SkillLiteToolkit.from_manager(manager)

    # Use with LangChain agent
    from langchain.agents import create_openai_tools_agent, AgentExecutor
    agent = create_openai_tools_agent(llm, tools, prompt)
    executor = AgentExecutor.from_agent_and_tools(agent=agent, tools=tools)

Security Confirmation:
    For sandbox level 3, the adapter supports security confirmation callbacks:

    def my_confirmation_callback(security_report: str, scan_id: str) -> bool:
        print(security_report)
        return input("Continue? [y/N]: ").lower() == 'y'

    tools = SkillLiteToolkit.from_manager(
        manager,
        sandbox_level=3,
        confirmation_callback=my_confirmation_callback
    )

Requirements:
    pip install skilllite[langchain]
"""

from typing import Any, Dict, List, Optional, Type, TYPE_CHECKING
import asyncio

try:
    from langchain_core.tools import BaseTool
    from langchain_core.callbacks import CallbackManagerForToolRun, AsyncCallbackManagerForToolRun
    from pydantic import BaseModel, Field, ConfigDict
except ImportError as e:
    raise ImportError(
        "LangChain adapter requires langchain-core. "
        "Install with: pip install skilllite[langchain]"
    ) from e

# Import unified types from protocols layer - Single Source of Truth
from ..protocols import (
    SecurityScanResult,
    ConfirmationCallback,
    AsyncConfirmationCallback,
)
from .base import BaseAdapter

if TYPE_CHECKING:
    from ..manager import SkillManager


class SkillLiteTool(BaseTool):
    """
    LangChain BaseTool adapter for a single SkillLite skill.

    This wraps a SkillLite skill as a LangChain tool, enabling it to be
    used with LangChain agents.

    Attributes:
        name: Tool name (same as skill name)
        description: Tool description
        manager: SkillManager instance
        skill_name: Name of the skill to execute
        allow_network: Whether to allow network access
        timeout: Execution timeout in seconds
        sandbox_level: Sandbox security level (1/2/3, default: 3)
        confirmation_callback: Callback for security confirmation (sync)
        async_confirmation_callback: Callback for security confirmation (async)
    """

    name: str = Field(description="Tool name")
    description: str = Field(description="Tool description")
    args_schema: Optional[Type[BaseModel]] = Field(default=None, description="Pydantic schema for arguments")

    # SkillLite specific fields
    manager: Any = Field(exclude=True)  # SkillManager instance
    skill_name: str = Field(description="SkillLite skill name")
    allow_network: bool = Field(default=False, description="Allow network access")
    timeout: Optional[int] = Field(default=None, description="Execution timeout in seconds")

    # Security confirmation fields
    sandbox_level: int = Field(default=3, description="Sandbox security level (1/2/3)")
    confirmation_callback: Optional[Any] = Field(
        default=None,
        exclude=True,
        description="Sync callback for security confirmation: (report: str, scan_id: str) -> bool"
    )
    async_confirmation_callback: Optional[Any] = Field(
        default=None,
        exclude=True,
        description="Async callback for security confirmation: (report: str, scan_id: str) -> Future[bool]"
    )

    model_config = ConfigDict(arbitrary_types_allowed=True)

    def _run(
        self,
        run_manager: Optional[CallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """
        Execute the skill synchronously using ipc_executor.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning and confirmation
        3. Properly downgrades sandbox level after confirmation
        """
        try:
            if 'kwargs' in kwargs and isinstance(kwargs['kwargs'], dict) and len(kwargs) == 1:
                input_data = kwargs['kwargs']
            else:
                input_data = kwargs

            # Resolve skill_info (handle multi-script tools)
            tool_info = self.manager._registry.get_multi_script_tool_info(self.skill_name)
            if tool_info:
                skill_info = self.manager._registry.get_skill(tool_info["skill_name"])
                entry_point = tool_info.get("script_path")
            else:
                skill_info = self.manager._registry.get_skill(self.skill_name)
                entry_point = None

            if not skill_info:
                return f"Error: Skill '{self.skill_name}' not found"

            # Bash-tool: extract command
            if skill_info.is_bash_tool_skill:
                from ...sandbox.ipc_executor import execute_bash_via_ipc
                cmd = input_data.get("command", "")
                if not cmd:
                    return "Error: Bash tool requires 'command' parameter"
                result = execute_bash_via_ipc(skill_info, cmd, timeout=self.timeout)
            else:
                from ...sandbox.ipc_executor import execute_via_ipc
                result = execute_via_ipc(
                    skill_info=skill_info,
                    input_data=input_data,
                    entry_point=entry_point,
                    confirmation_callback=self.confirmation_callback,
                    allow_network=self.allow_network,
                    timeout=self.timeout,
                )

            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"

    async def _arun(
        self,
        run_manager: Optional[AsyncCallbackManagerForToolRun] = None,
        **kwargs: Any
    ) -> str:
        """
        Execute the skill asynchronously using ipc_executor.

        This method uses the unified execution layer which:
        1. Reads sandbox level at runtime
        2. Handles security scanning and confirmation
        3. Properly downgrades sandbox level after confirmation
        """
        try:
            if 'kwargs' in kwargs and isinstance(kwargs['kwargs'], dict) and len(kwargs) == 1:
                input_data = kwargs['kwargs']
            else:
                input_data = kwargs

            tool_info = self.manager._registry.get_multi_script_tool_info(self.skill_name)
            if tool_info:
                skill_info = self.manager._registry.get_skill(tool_info["skill_name"])
                entry_point = tool_info.get("script_path")
            else:
                skill_info = self.manager._registry.get_skill(self.skill_name)
                entry_point = None

            if not skill_info:
                return f"Error: Skill '{self.skill_name}' not found"

            def execute_sync():
                if skill_info.is_bash_tool_skill:
                    from ...sandbox.ipc_executor import execute_bash_via_ipc
                    from ...sandbox.base import ExecutionResult
                    cmd = input_data.get("command", "")
                    if not cmd:
                        return ExecutionResult(success=False, error="Bash tool requires 'command'", exit_code=1)
                    return execute_bash_via_ipc(skill_info, cmd, timeout=self.timeout)
                from ...sandbox.ipc_executor import execute_via_ipc
                return execute_via_ipc(
                    skill_info=skill_info,
                    input_data=input_data,
                    entry_point=entry_point,
                    confirmation_callback=self.confirmation_callback,
                    allow_network=self.allow_network,
                    timeout=self.timeout,
                )

            result = await asyncio.to_thread(execute_sync)

            if result.success:
                return result.output or "Execution completed successfully"
            else:
                return f"Error: {result.error}"
        except Exception as e:
            return f"Execution failed: {str(e)}"


class SkillLiteToolkit(BaseAdapter):
    """
    LangChain Toolkit for SkillLite - inherits from BaseAdapter.

    Provides a convenient way to create LangChain tools from all skills
    registered in a SkillManager.

    This class inherits common functionality from BaseAdapter:
    - Unified execution through ipc_executor
    - Shared security scanning logic
    - Common confirmation flow

    Usage:
        manager = SkillManager(skills_dir="./skills")
        tools = SkillLiteToolkit.from_manager(manager)

        # Or with options
        tools = SkillLiteToolkit.from_manager(
            manager,
            skill_names=["calculator", "web_search"],  # Only specific skills
            allow_network=True,
            timeout=60
        )

        # With security confirmation callback (for sandbox level 3)
        def confirm_execution(report: str, scan_id: str) -> bool:
            print(report)
            return input("Continue? [y/N]: ").lower() == 'y'

        tools = SkillLiteToolkit.from_manager(
            manager,
            sandbox_level=3,
            confirmation_callback=confirm_execution
        )
    """

    def to_tools(self) -> List[SkillLiteTool]:
        """
        Convert skills to LangChain tools.

        Implements the abstract method from BaseAdapter.

        Returns:
            List of SkillLiteTool instances
        """
        tools = []
        for skill in self.get_executable_skills():
            # Skill Usage Protocol - Phase 2 (Usage Phase):
            # Use full SKILL.md content as description so LLM can infer
            # correct parameters from usage examples.
            full_content = skill.get_full_content()
            tool_description = full_content or skill.description or f"Execute the {skill.name} skill"

            tool = SkillLiteTool(
                name=skill.name,
                description=tool_description,
                manager=self.manager,
                skill_name=skill.name,
                allow_network=self.allow_network,
                timeout=self.timeout,
                sandbox_level=self.sandbox_level,
                confirmation_callback=self.confirmation_callback,
                async_confirmation_callback=self.async_confirmation_callback,
            )
            tools.append(tool)
        return tools

    @classmethod
    def from_manager(
        cls,
        manager: "SkillManager",
        skill_names: Optional[List[str]] = None,
        allow_network: bool = False,
        timeout: Optional[int] = None,
        sandbox_level: int = 3,
        confirmation_callback: Optional[ConfirmationCallback] = None,
        async_confirmation_callback: Optional[AsyncConfirmationCallback] = None,
    ) -> List[SkillLiteTool]:
        """
        Create LangChain tools from a SkillManager.

        This is a convenience factory method that maintains backward compatibility.

        Args:
            manager: SkillManager instance with registered skills
            skill_names: Optional list of skill names to include (default: all)
            allow_network: Whether to allow network access for all tools
            timeout: Execution timeout in seconds for all tools
            sandbox_level: Sandbox security level (1/2/3, default: 3)
                - Level 1: No sandbox - direct execution
                - Level 2: Sandbox isolation only
                - Level 3: Sandbox isolation + security scanning (requires confirmation for high-severity issues)
            confirmation_callback: Sync callback for security confirmation.
                Signature: (security_report: str, scan_id: str) -> bool
                Return True to proceed, False to cancel.
            async_confirmation_callback: Async callback for security confirmation.
                Signature: (security_report: str, scan_id: str) -> Future[bool]
                Return True to proceed, False to cancel.

        Returns:
            List of SkillLiteTool instances

        Example with confirmation callback:
            def my_callback(report: str, scan_id: str) -> bool:
                print(f"Security Report:\\n{report}")
                response = input("Proceed with execution? [y/N]: ")
                return response.lower() == 'y'

            tools = SkillLiteToolkit.from_manager(
                manager,
                sandbox_level=3,
                confirmation_callback=my_callback
            )
        """
        toolkit = cls(
            manager=manager,
            sandbox_level=sandbox_level,
            allow_network=allow_network,
            timeout=timeout,
            confirmation_callback=confirmation_callback,
            async_confirmation_callback=async_confirmation_callback,
            skill_names=skill_names,
        )
        return toolkit.to_tools()


__all__ = [
    "SkillLiteTool",
    "SkillLiteToolkit",
    "SecurityScanResult",
    "ConfirmationCallback",
    "AsyncConfirmationCallback",
]

